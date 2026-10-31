//! Concrete command-host composition for the Rust core services.
//!
//! The command wire owns decoding and DTO conversion. This adapter owns only
//! the composition boundary: it validates the caller-owned project session,
//! assembles the existing MC facades, and preserves unavailable host facts as
//! typed command failures.
mod mapping_coverage;

use super::{
    completion::completion_from_core,
    host::{
        BuildIdentity, CommandFailure, CommandHost, RuntimeCompletion, SemanticSnapshot,
        SourceFamily,
    },
};
use mcdev_core::{
    awat::{AwAtBufferSupport, AwAtFacadeRequest, AwAtServiceFacade},
    bytecode::{AtTargetCandidate, BytecodeIndex},
    expression_engine::{Cancellation, CancellationChecker},
    host_project::{
        HostBytecodeAdapter, HostCheckedClassIndex, HostProjectQuery, HostProjectSession,
    },
    host_source::{
        candidate_cache_debug, HostFact, HostProjectKey, HostSemanticModelProvider,
        HostSemanticRequest, HostSourceError, SemanticModelCache,
    },
    mixin_actions::MixinCodeActionService,
    mixin_completion::AtValueCompletionService,
    mixin_facade::{
        HostFacadeError, HostMixinFacade, HostMixinServiceFacade, HostMixinSourceProvider,
        MixinFacadeRequest,
    },
    mixin_navigation::{DefinitionBackend, DefinitionResolution, DefinitionResolutionService},
    mixin_source::{AnnotationContextExtractor, AnnotationSlot, MixinAnnotation},
    mixinextras_expression::ExpressionMemberCompletionCache,
    mixinextras_handler::HandlerReferenceTypeResolver,
    model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry},
    project::{normalize_path, uri_to_path, CachedProjectSession, ProjectContext},
    McDefinitionTarget, McFix, SourceScanEntry,
};
use mcdev_protocol::{
    convert::ResolvedDefinition, McdevCompletionOptions, McdevCompletionRequest,
    McdevCompletionTrigger, McdevDefinitionResolution, McdevError, McdevErrorCode, McdevRange,
    McdevRequestContext,
};
use std::{
    any::Any,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// The minimum concrete host required by the command dispatcher.
///
/// Identity-bearing inputs are owned by the caller and retained for this
/// host's lifetime. This type does not discover a project, walk source roots,
/// or manufacture SDK/classpath facts.
pub struct CoreCommandHost {
    authority: SessionAuthority,
    checked_class_index: Arc<dyn ClassIndex>,
    checked_bytecode_index: Arc<dyn BytecodeIndex>,
    reference_types: Arc<dyn HandlerReferenceTypeResolver>,
    definitions: Arc<dyn DefinitionBackend>,
    cached_session: CachedProjectSession,
    semantic_provider: Arc<dyn HostSemanticModelProvider>,
    semantic_cache: Arc<SemanticModelCache>,
    expression_cache: Arc<ExpressionMemberCompletionCache>,
    source_provider: Arc<dyn HostMixinSourceProvider>,
    source_inventory: HostFact<Vec<SourceScanEntry>>,
    build_identity: BuildIdentity,
    host_supports_kotlin_semantics: bool,
}

type ActiveCheck = dyn Fn() -> Result<(), HostSourceError> + Send + Sync;
type SpecifierProbe = dyn Fn() -> Result<bool, HostSourceError> + Send + Sync;
type AdditionalAtValues = dyn Fn() -> Result<Vec<String>, HostSourceError> + Send + Sync;

/// Owned inputs valid only for one host evaluation. Infallible compatibility
/// traits preserve unavailable facts as typed `CommandFailure` panic payloads.
pub struct EvaluationCommandInputs {
    pub workspace_root_uri: String,
    pub cached_session: CachedProjectSession,
    pub class_index: Arc<dyn ClassIndex>,
    pub bytecode_index: Arc<dyn BytecodeIndex>,
    pub reference_types: Arc<dyn HandlerReferenceTypeResolver>,
    pub definitions: Arc<dyn DefinitionBackend>,
    pub check_active: Arc<ActiveCheck>,
    pub specifier_supported: Arc<SpecifierProbe>,
    pub additional_at_values: Arc<AdditionalAtValues>,
    /// Host-issued context.refreshGeneration, present only after host refresh.
    pub refresh_generation: Option<String>,
    /// All config candidates and their source-set owners were captured.
    pub config_selection_complete: bool,
    /// The legacy root/cache mapping domain and its platform inputs were captured.
    pub mapping_discovery_complete: bool,
}

enum SessionAuthority {
    Legacy {
        session: Arc<HostProjectSession>,
        project_query: Arc<dyn HostProjectQuery>,
    },
    Evaluation {
        workspace_root_uri: String,
        check_active: Arc<ActiveCheck>,
        specifier_supported: Arc<SpecifierProbe>,
        additional_at_values: Arc<AdditionalAtValues>,
        refresh_generation: Option<String>,
        config_selection_complete: bool,
        mapping_discovery_complete: bool,
    },
}

/// The core traits predate the host fact protocol and intentionally expose
/// compatibility fallbacks.  The command boundary uses these private checked
/// adapters so an unavailable fact becomes a panic payload that the single
/// command catch boundary converts into a typed failure, never an empty core
/// result.
struct CheckedClassIndex {
    index: Arc<HostCheckedClassIndex>,
}

fn checked_fact<T: Default>(result: Result<HostFact<T>, HostSourceError>) -> T {
    match result {
        Ok(HostFact::Known(value)) => value,
        Ok(HostFact::Missing) => T::default(),
        Ok(HostFact::Unavailable { reason }) => {
            panic_checked_failure(HostSourceError::Failure(reason))
        }
        Err(error) => panic_checked_failure(error),
    }
}

impl ClassIndex for CheckedClassIndex {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        checked_fact(self.index.find_classes_result(prefix, limit))
    }

    fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry> {
        checked_fact(self.index.find_class_result(internal_name))
    }

    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        checked_fact(self.index.find_class_by_fqn_result(fqn))
    }

    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        checked_fact(self.index.get_methods_result(owner))
    }

    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        checked_fact(self.index.get_fields_result(owner))
    }
}

struct CheckedBytecodeIndex {
    index: Arc<HostBytecodeAdapter>,
}

struct EvaluationReferenceTypes {
    resolver: Arc<dyn HandlerReferenceTypeResolver>,
}

impl HandlerReferenceTypeResolver for EvaluationReferenceTypes {
    fn is_known_reference(&self, internal: &str) -> Option<bool> {
        Some(
            self.resolver
                .is_known_reference(internal)
                .unwrap_or_else(|| {
                    std::panic::panic_any(HostSourceError::Failure(
                        "reference type facts are unavailable".into(),
                    ))
                }),
        )
    }

    fn is_assignable(&self, from: &str, expected: &str) -> Option<bool> {
        Some(
            self.resolver
                .is_assignable(from, expected)
                .unwrap_or_else(|| {
                    std::panic::panic_any(HostSourceError::Failure(
                        "reference assignability is unavailable".into(),
                    ))
                }),
        )
    }
}

/// The compatibility `DefinitionBackend` cannot represent an unavailable
/// host answer. Keep that answer fallible at this command boundary instead of
/// turning it into the same `None` used for a checked missing location.
struct CheckedDefinitionBackend {
    session: Arc<HostProjectSession>,
}

impl DefinitionBackend for CheckedDefinitionBackend {
    fn resolve(
        &self,
        target: &McDefinitionTarget,
        _context: &ProjectContext,
        _workspace_root_uri: &str,
    ) -> Option<mcdev_core::mixin_navigation::ResolvedDefinition> {
        match self.session.definition_result(target) {
            Ok(HostFact::Known(value)) => Some(value),
            Ok(HostFact::Missing) => None,
            Ok(HostFact::Unavailable { reason }) => {
                std::panic::panic_any(HostSourceError::Failure(reason))
            }
            Err(error) => std::panic::panic_any(error),
        }
    }
}

fn checked_bytecode<T>(result: Result<T, HostSourceError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic_checked_failure(error),
    }
}

impl BytecodeIndex for CheckedBytecodeIndex {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
        at_value: &str,
    ) -> Vec<AtTargetCandidate> {
        checked_bytecode(
            self.index
                .candidates_result(owner, method, descriptor, at_value),
        )
    }

    fn get_return_ordinal_count(
        &self,
        owner: &str,
        method: &str,
        descriptor: Option<&str>,
    ) -> usize {
        checked_bytecode(self.index.return_count_result(owner, method, descriptor))
    }

    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        checked_bytecode(self.index.get_class_bytes_arc(owner)).map(|bytes| bytes.to_vec())
    }

    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        checked_bytecode(self.index.common_super_result(left, right))
    }
}

impl CoreCommandHost {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: Arc<HostProjectSession>,
        cached_session: CachedProjectSession,
        project_query: Arc<dyn HostProjectQuery>,
        semantic_provider: Arc<dyn HostSemanticModelProvider>,
        source_provider: Arc<dyn HostMixinSourceProvider>,
        source_inventory: HostFact<Vec<SourceScanEntry>>,
        build_identity: BuildIdentity,
        host_supports_kotlin_semantics: bool,
    ) -> Result<Self, CommandFailure> {
        let checked_class_index = Arc::new(CheckedClassIndex {
            index: session.checked_class_index.clone(),
        });
        let checked_bytecode = Arc::new(HostBytecodeAdapter::new(
            session.class_bytes_provider.clone(),
            checked_class_index.clone(),
        ));
        let reference_types = Arc::new(session.checked_reference_types());
        let definitions = Arc::new(CheckedDefinitionBackend {
            session: session.clone(),
        });
        let workspace_root_uri = session.identity.workspace_root_uri.clone();
        let host = Self {
            authority: SessionAuthority::Legacy {
                session,
                project_query,
            },
            checked_class_index,
            checked_bytecode_index: Arc::new(CheckedBytecodeIndex {
                index: checked_bytecode,
            }),
            reference_types,
            definitions,
            cached_session,
            semantic_provider,
            semantic_cache: Arc::new(SemanticModelCache::default()),
            expression_cache: Arc::new(ExpressionMemberCompletionCache::default()),
            source_provider,
            source_inventory,
            build_identity,
            host_supports_kotlin_semantics,
        };
        host.validate_cached_identity()?;
        host.validate_workspace(&workspace_root_uri)?;
        Ok(host)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_evaluation(
        inputs: EvaluationCommandInputs,
        semantic_provider: Arc<dyn HostSemanticModelProvider>,
        source_provider: Arc<dyn HostMixinSourceProvider>,
        source_inventory: HostFact<Vec<SourceScanEntry>>,
        build_identity: BuildIdentity,
        host_supports_kotlin_semantics: bool,
    ) -> Result<Self, CommandFailure> {
        if !inputs.cached_session.context.root.is_absolute() {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "owning project root is not absolute",
            ));
        }
        if inputs.cached_session.cache_hit {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "an evaluation cannot reuse a project session from another evaluation",
            ));
        }
        if inputs.refresh_generation.as_ref().is_some_and(|value| {
            value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
                || value.parse::<u64>().is_err()
        }) {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "invalid host refresh generation",
            ));
        }
        let root = inputs.workspace_root_uri.clone();
        let host = Self {
            authority: SessionAuthority::Evaluation {
                workspace_root_uri: inputs.workspace_root_uri,
                check_active: inputs.check_active,
                specifier_supported: inputs.specifier_supported,
                additional_at_values: inputs.additional_at_values,
                refresh_generation: inputs.refresh_generation,
                config_selection_complete: inputs.config_selection_complete,
                mapping_discovery_complete: inputs.mapping_discovery_complete,
            },
            checked_class_index: inputs.class_index,
            checked_bytecode_index: inputs.bytecode_index,
            reference_types: Arc::new(EvaluationReferenceTypes {
                resolver: inputs.reference_types,
            }),
            definitions: inputs.definitions,
            cached_session: inputs.cached_session,
            semantic_provider,
            semantic_cache: Arc::new(SemanticModelCache::default()),
            expression_cache: Arc::new(ExpressionMemberCompletionCache::default()),
            source_provider,
            source_inventory,
            build_identity,
            host_supports_kotlin_semantics,
        };
        host.validate_workspace(&root)?;
        Ok(host)
    }

    fn workspace_root_uri(&self) -> &str {
        match &self.authority {
            SessionAuthority::Legacy { session, .. } => &session.identity.workspace_root_uri,
            SessionAuthority::Evaluation {
                workspace_root_uri, ..
            } => workspace_root_uri,
        }
    }

    fn validate_cached_identity(&self) -> Result<(), CommandFailure> {
        let SessionAuthority::Legacy { session, .. } = &self.authority else {
            return Ok(());
        };
        let identity = &session.identity;
        if self.cached_session.version != identity.project_session_version
            || self.cached_session.context.project_id != session.context.project_id
            || normalize_path(&self.cached_session.context.root)
                != normalize_path(&session.context.root)
        {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "cached project session does not match the supplied host session",
            ));
        }
        Ok(())
    }

    fn validate_workspace(&self, workspace_root: &str) -> Result<(), CommandFailure> {
        let requested = normalize_path(&uri_to_path(workspace_root).map_err(|error| {
            unavailable(
                McdevErrorCode::IncompleteProjectContext,
                format!("invalid workspace root: {error}"),
            )
        })?);
        let session_root =
            normalize_path(&uri_to_path(self.workspace_root_uri()).map_err(|error| {
                unavailable(
                    McdevErrorCode::IncompleteProjectContext,
                    format!("invalid session workspace root: {error}"),
                )
            })?);
        if requested != session_root {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "workspace root does not match the owned project session",
            ));
        }
        match &self.authority {
            SessionAuthority::Legacy {
                session,
                project_query,
            } => {
                if normalize_path(&session.context.root) != session_root {
                    return Err(unavailable(
                        McdevErrorCode::IncompleteProjectContext,
                        "workspace root does not match the owned project session",
                    ));
                }
                if !project_query.is_open(&session.identity) {
                    return Err(unavailable(
                        McdevErrorCode::IncompleteProjectContext,
                        "project session is closed",
                    ));
                }
                if !project_query
                    .validate(&session.identity, &Vec::new())
                    .map_err(map_source_error)?
                {
                    return Err(unavailable(
                        McdevErrorCode::IncompleteProjectContext,
                        "project session generations are stale",
                    ));
                }
                session
                    .class_bytes_provider
                    .current_revision()
                    .map_err(map_source_error)?;
            }
            SessionAuthority::Evaluation { check_active, .. } => {
                check_active().map_err(map_source_error)?;
            }
        }
        Ok(())
    }

    fn validate_project(
        &self,
        project: &ProjectContext,
        workspace_root: &str,
    ) -> Result<(), CommandFailure> {
        self.validate_workspace(workspace_root)?;
        if project.project_id != self.cached_session.context.project_id
            || normalize_path(&project.root) != normalize_path(&self.cached_session.context.root)
        {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "request project is not the owned project session",
            ));
        }
        Ok(())
    }

    fn validate_semantic_capability(
        &self,
        context: &McdevRequestContext,
    ) -> Result<(), CommandFailure> {
        if HostMixinFacade::is_kotlin_document(&context.document_uri)
            && !self.host_supports_kotlin_semantics
        {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "normalized Kotlin semantic facts are unavailable",
            ));
        }
        Ok(())
    }

    fn require_config_selection(&self) -> Result<(), CommandFailure> {
        if !self.config_selection_complete() {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "complete Mixin config candidates and source-set ownership are unavailable",
            ));
        }
        Ok(())
    }

    fn config_selection_complete(&self) -> bool {
        !matches!(
            self.authority,
            SessionAuthority::Evaluation {
                config_selection_complete: false,
                ..
            }
        )
    }

    fn mapping_discovery_complete(&self) -> bool {
        match &self.authority {
            SessionAuthority::Legacy { .. } => true,
            SessionAuthority::Evaluation {
                mapping_discovery_complete,
                ..
            } => *mapping_discovery_complete,
        }
    }

    fn require_registered_sources_for_completion(
        &self,
        context: &McdevRequestContext,
    ) -> Result<(), CommandFailure> {
        if self.config_selection_complete() {
            return Ok(());
        }
        let site = usize::try_from(context.position.line)
            .ok()
            .zip(usize::try_from(context.position.character).ok())
            .and_then(|(line, character)| {
                mcdev_core::position_to_offset(
                    &context.buffer_text,
                    mcdev_core::McTextPosition { line, character },
                )
            })
            .and_then(|offset| {
                AnnotationContextExtractor::extract_at_offset(&context.buffer_text, offset)
            });
        if site.is_some_and(|site| {
            site.annotation == MixinAnnotation::Share
                && site.slot == AnnotationSlot::Value
                && site.attribute_name.as_deref() != Some("namespace")
        }) {
            self.require_config_selection()?;
        }
        Ok(())
    }

    fn project_key(&self) -> Option<HostProjectKey> {
        match &self.authority {
            SessionAuthority::Legacy { session, .. } => Some(HostProjectKey {
                project_id: session.context.project_id.clone(),
                generations: session.identity.generations,
            }),
            SessionAuthority::Evaluation { .. } => None,
        }
    }

    fn facade<'a>(&'a self, document_version: Option<i64>) -> HostMixinServiceFacade<'a> {
        let key = self.project_key();
        let project_root = normalize_path(&self.cached_session.context.root);
        let mut facade = HostMixinServiceFacade::new(
            self.checked_class_index.as_ref(),
            self.checked_bytecode_index.as_ref(),
            self.semantic_provider.as_ref(),
            self.semantic_cache.as_ref(),
        );
        facade.context = Some(self.cached_session.context.as_ref());
        if let Some(key) = &key {
            facade = facade.with_expression_cache(self.expression_cache.clone(), key.clone());
        }
        facade = facade.with_registered_sources(self.source_provider.as_ref());
        if self.config_selection_complete() {
            facade = facade.with_registered_handler_sources(self.source_provider.as_ref());
        }
        facade.project = key.clone();
        facade.project_session_version = self.cached_session.version;
        facade.document_version = document_version;
        facade.host_supports_kotlin_semantics = self.host_supports_kotlin_semantics;
        let resolver_key = key.clone();
        facade.project_resolver = Box::new(move |_, requested_workspace, _| {
            if requested_workspace.is_some_and(|root| {
                uri_to_path(root)
                    .ok()
                    .map(|path| normalize_path(&path))
                    .as_ref()
                    != Some(&project_root)
            }) {
                return Err(HostSourceError::Failure(
                    "project root does not match the owned project session".into(),
                ));
            }
            Ok(resolver_key.clone())
        });
        let ready_key = key.clone();
        facade.source_query_ready = Box::new(move |candidate| {
            if ready_key.as_ref() == Some(candidate) {
                Ok(())
            } else {
                Err(HostSourceError::Failure(
                    "source query project identity does not match the session".into(),
                ))
            }
        });
        facade.specifier_supported = Box::new(|_, _| self.specifier_supported());
        facade.additional_at_values = Box::new(|_, _| self.additional_at_values());
        facade
    }

    fn specifier_supported(&self) -> Result<bool, HostSourceError> {
        match &self.authority {
            SessionAuthority::Evaluation {
                specifier_supported,
                ..
            } => specifier_supported(),
            SessionAuthority::Legacy { .. } => Err(HostSourceError::Failure(
                "specifier support facts are unavailable".into(),
            )),
        }
    }

    fn additional_at_values(&self) -> Result<Vec<String>, HostSourceError> {
        match &self.authority {
            SessionAuthority::Evaluation {
                additional_at_values,
                ..
            } => additional_at_values(),
            SessionAuthority::Legacy { .. } => Err(HostSourceError::Failure(
                "additional AtValue facts are unavailable".into(),
            )),
        }
    }

    fn semantic_model(
        &self,
        context: &McdevRequestContext,
        snapshot_version: i64,
        project: &ProjectContext,
        cancellation: &dyn CancellationChecker,
    ) -> Result<SemanticSnapshot, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        self.validate_semantic_capability(context)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let key = self.project_key();
        let environment =
            HostMixinServiceFacade::semantic_environment(Some(project), &context.document_uri);
        let answer = self
            .semantic_cache
            .get(
                &HostSemanticRequest {
                    source: &context.buffer_text,
                    source_uri: &context.document_uri,
                    // An internal snapshot sequence is not a host LSP version.
                    document_version: match &self.authority {
                        SessionAuthority::Legacy { .. } => Some(snapshot_version),
                        SessionAuthority::Evaluation { .. } => context.document_version,
                    },
                    environment: &environment,
                    project: key.as_ref(),
                },
                self.semantic_provider.as_ref(),
            )
            .map_err(map_source_error)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        match answer.answer.fact {
            HostFact::Known(model) => {
                let snapshot = SemanticSnapshot {
                    model: model.model,
                    cache_hit: answer.cache_hit,
                    parse_ms: i64::try_from(model.parse_ms).unwrap_or(i64::MAX),
                };
                self.validate_workspace(&context.workspace_root)?;
                Ok(snapshot)
            }
            HostFact::Missing => Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "host semantic model is missing",
            )),
            HostFact::Unavailable { reason } => Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                reason,
            )),
        }
    }

    fn build_facade_request<'a>(
        &'a self,
        facade: &HostMixinServiceFacade<'a>,
        context: &McdevRequestContext,
        line: i32,
        character: i32,
    ) -> Result<MixinFacadeRequest, CommandFailure> {
        facade
            .build_request(
                &context.buffer_text,
                &context.document_uri,
                line,
                character,
                self.source_provider.as_ref(),
            )
            .map_err(map_facade_error)
    }

    fn completion_request(
        context: &McdevRequestContext,
        options: &mcdev_core::mixin_completion::MixinCompletionOptions,
    ) -> McdevCompletionRequest {
        McdevCompletionRequest {
            context: context.clone(),
            trigger: McdevCompletionTrigger {
                kind: "invoked".into(),
                character: None,
            },
            options: McdevCompletionOptions {
                preferred_at_target: options.preferred_at_target.clone(),
                mixin_class_insert: match options.class_insert_mode {
                    mcdev_core::mixin_completion::MixinClassInsertMode::Fqn => "fqn",
                    mcdev_core::mixin_completion::MixinClassInsertMode::Import => "import",
                }
                .into(),
                inject_method_descriptor: match options.inject_method_descriptor_mode {
                    mcdev_core::mixin_completion::InjectMethodDescriptorMode::Always => "always",
                    mcdev_core::mixin_completion::InjectMethodDescriptorMode::Never => "never",
                    mcdev_core::mixin_completion::InjectMethodDescriptorMode::Auto => "auto",
                }
                .into(),
            },
        }
    }

    fn buffer_only_at_values(&self) -> AtValueCompletionService<'_> {
        AtValueCompletionService::new(
            || {
                self.specifier_supported()
                    .unwrap_or_else(|error| std::panic::panic_any(error))
            },
            || {
                self.additional_at_values()
                    .unwrap_or_else(|error| std::panic::panic_any(error))
            },
        )
    }

    fn source_entries_for(
        &self,
        context: &McdevRequestContext,
        family: SourceFamily,
    ) -> Result<Vec<SourceScanEntry>, CommandFailure> {
        let entries = match &self.source_inventory {
            HostFact::Known(entries) => entries.clone(),
            HostFact::Missing => {
                return Err(unavailable(
                    McdevErrorCode::IncompleteProjectContext,
                    "source inventory is missing",
                ))
            }
            HostFact::Unavailable { reason } => {
                return Err(unavailable(
                    McdevErrorCode::IncompleteProjectContext,
                    reason,
                ))
            }
        };
        let mut filtered = entries
            .into_iter()
            .filter(|entry| match family {
                SourceFamily::JavaKotlin => {
                    AwAtBufferSupport::detect_file_type("", &entry.document_uri).is_none()
                }
                SourceFamily::AwAt => {
                    AwAtBufferSupport::detect_file_type("", &entry.document_uri).is_some()
                }
            })
            .collect::<Vec<_>>();
        let current_is_family = match family {
            SourceFamily::JavaKotlin => {
                AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
                    .is_none()
            }
            SourceFamily::AwAt => {
                AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
                    .is_some()
            }
        };
        if current_is_family {
            filtered = HostMixinFacade::collect_source_entries(
                &context.document_uri,
                &context.buffer_text,
                filtered,
            );
        }
        Ok(filtered)
    }

    fn resolve_java_fix(
        &self,
        context: &McdevRequestContext,
        fix: &McFix,
        config_content: Option<&str>,
    ) -> Result<Option<(String, McFix)>, CommandFailure> {
        match fix {
            McFix::AddMixinConfigEntryFix { .. } => {
                let content = config_content.ok_or_else(|| {
                    unavailable(
                        McdevErrorCode::IncompleteProjectContext,
                        "selected mixin config content is unavailable",
                    )
                })?;
                Ok(MixinCodeActionService::apply_fix(fix, content)
                    .map(|resolved| (content.to_string(), resolved)))
            }
            McFix::AddMethodDescriptorFix { .. }
            | McFix::GenerateAccessorMethodFix { .. }
            | McFix::GenerateInvokerMethodFix { .. } => {
                Ok(MixinCodeActionService::apply_fix(fix, &context.buffer_text)
                    .map(|resolved| (context.buffer_text.clone(), resolved)))
            }
            McFix::WorkspaceEditFix { .. } => Ok(Some((context.buffer_text.clone(), fix.clone()))),
            _ => Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "the core produced a fix whose host application domain is unavailable",
            )),
        }
    }
}

impl CommandHost for CoreCommandHost {
    fn now_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
            .unwrap_or(0)
    }

    fn build_identity(&self) -> Result<BuildIdentity, CommandFailure> {
        Ok(self.build_identity.clone())
    }

    fn has_project_facts(&self, context: &McdevRequestContext) -> Result<bool, CommandFailure> {
        self.validate_workspace(&context.workspace_root)?;
        Ok(true)
    }

    fn load_session(&self, workspace_root: &str) -> Result<Arc<ProjectContext>, CommandFailure> {
        self.validate_workspace(workspace_root)?;
        Ok(self.cached_session.context.clone())
    }

    fn load_cached_session(
        &self,
        workspace_root: &str,
    ) -> Result<CachedProjectSession, CommandFailure> {
        self.validate_workspace(workspace_root)?;
        Ok(self.cached_session.clone())
    }

    fn reindex(&self, workspace_root: &str) -> Result<Arc<ProjectContext>, CommandFailure> {
        self.validate_workspace(workspace_root)?;
        // ktlsp ea8be67, crates/ktlsp/src/lsp.rs, sets this only after refresh succeeds.
        if matches!(
            &self.authority,
            SessionAuthority::Evaluation {
                refresh_generation: Some(_),
                ..
            }
        ) {
            Ok(self.cached_session.context.clone())
        } else {
            Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "host project refresh authority is unavailable",
            ))
        }
    }

    fn clear_project_cache(&self) -> Result<(), CommandFailure> {
        self.validate_workspace(self.workspace_root_uri())?;
        if matches!(
            &self.authority,
            SessionAuthority::Evaluation {
                refresh_generation: Some(_),
                ..
            }
        ) {
            self.semantic_cache.clear();
            Ok(())
        } else {
            Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "host project refresh authority is unavailable",
            ))
        }
    }

    fn semantic_model(
        &self,
        context: &McdevRequestContext,
        snapshot_version: i64,
        project: &ProjectContext,
        cancellation: &dyn CancellationChecker,
    ) -> Result<SemanticSnapshot, CommandFailure> {
        self.semantic_model(context, snapshot_version, project, cancellation)
    }

    fn buffer_only_completion(
        &self,
        context: &McdevRequestContext,
        options: &mcdev_core::mixin_completion::MixinCompletionOptions,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Option<RuntimeCompletion>, CommandFailure> {
        self.require_registered_sources_for_completion(context)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let request = Self::completion_request(context, options);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            HostMixinFacade::try_complete_buffer_only(
                &context.buffer_text,
                context.position.line,
                context.position.character,
                options,
                &context.document_uri,
                &context.language_id,
                false,
                self.buffer_only_at_values(),
            )
        }));
        let result = match result {
            Ok(result) => result.map_err(CommandFailure::Cancelled),
            Err(payload) => Err(panic_failure(payload)),
        }?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let result = result
            .map(|value| completion_from_core(&request, value.result, value.context))
            .transpose()?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(result)
    }

    fn complete(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        options: &mcdev_core::mixin_completion::MixinCompletionOptions,
        semantic: &mcdev_core::mixin_source::MixinClassModel,
        project_version: u64,
        cancellation: &dyn CancellationChecker,
    ) -> Result<RuntimeCompletion, CommandFailure> {
        self.require_registered_sources_for_completion(context)?;
        self.validate_project(project, &context.workspace_root)?;
        self.validate_semantic_capability(context)?;
        if project_version != self.cached_session.version {
            return Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "completion project session version is stale",
            ));
        }
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let facade = self.facade(context.document_version);
        let mut request = self.build_facade_request(
            &facade,
            context,
            context.position.line,
            context.position.character,
        )?;
        request.semantic_model = Some(semantic.clone());
        let mut facade = facade;
        facade.reference_types = Some(self.reference_types.as_ref());
        let result =
            catch_facade(|| facade.complete_with_debug(&request, options, &context.language_id))?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let completion =
            completion_from_core(&Self::completion_request(context, options), result, None)?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(completion)
    }

    fn candidate_cache_metrics(&self) -> Result<(Option<bool>, Option<i64>), CommandFailure> {
        let debug = candidate_cache_debug();
        Ok((
            Some(debug.cache_hit),
            Some(debug.build_ms.min(i64::MAX as u64) as i64),
        ))
    }

    fn awat_complete(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<mcdev_core::McCompletionItem>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        let file_type =
            AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
                .ok_or_else(|| {
                    unavailable(
                        McdevErrorCode::NoApplicableContext,
                        "request is not an Access Widener or Access Transformer document",
                    )
                })?;
        let line = usize::try_from(context.position.line).map_err(|_| {
            unavailable(
                McdevErrorCode::NoApplicableContext,
                "completion line is negative",
            )
        })?;
        let character = usize::try_from(context.position.character).map_err(|_| {
            unavailable(
                McdevErrorCode::NoApplicableContext,
                "completion character is negative",
            )
        })?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        mapping_coverage::guard(
            self.mapping_discovery_complete(),
            file_type,
            &context.buffer_text,
            Some((line, character)),
        )?;
        let mappings =
            mapping_coverage::context(&project.mappings, self.mapping_discovery_complete());
        let service = AwAtServiceFacade::new(self.checked_class_index.as_ref(), Some(&mappings));
        let result = catch_command(|| {
            service.complete(&AwAtFacadeRequest {
                buffer_text: &context.buffer_text,
                line,
                character,
                document_uri: &context.document_uri,
                file_type,
            })
        })?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(result)
    }

    fn diagnostics(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<mcdev_core::McDiagnostic>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        if family == SourceFamily::AwAt {
            let file_type =
                AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
                    .ok_or_else(|| {
                        unavailable(
                            McdevErrorCode::NoApplicableContext,
                            "AW/AT file type is unknown",
                        )
                    })?;
            let line = usize::try_from(context.position.line).map_err(|_| {
                unavailable(
                    McdevErrorCode::NoApplicableContext,
                    "diagnostic line is negative",
                )
            })?;
            let character = usize::try_from(context.position.character).map_err(|_| {
                unavailable(
                    McdevErrorCode::NoApplicableContext,
                    "diagnostic character is negative",
                )
            })?;
            mapping_coverage::guard(
                self.mapping_discovery_complete(),
                file_type,
                &context.buffer_text,
                None,
            )?;
            let mappings =
                mapping_coverage::context(&project.mappings, self.mapping_discovery_complete());
            let service =
                AwAtServiceFacade::new(self.checked_class_index.as_ref(), Some(&mappings));
            let diagnostics = catch_command(|| {
                service.diagnose(&AwAtFacadeRequest {
                    buffer_text: &context.buffer_text,
                    line,
                    character,
                    document_uri: &context.document_uri,
                    file_type,
                })
            })?;
            self.validate_workspace(&context.workspace_root)?;
            return Ok(diagnostics);
        }
        if !HostMixinFacade::is_kotlin_document(&context.document_uri) {
            self.require_config_selection()?;
        }
        self.validate_semantic_capability(context)?;
        let facade = self.facade(context.document_version);
        let request = self.build_facade_request(
            &facade,
            context,
            context.position.line,
            context.position.character,
        )?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let mut facade = facade;
        facade.reference_types = Some(self.reference_types.as_ref());
        let diagnostics = catch_facade(|| facade.diagnose(&request))?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(diagnostics)
    }

    fn definitions(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McDefinitionTarget>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        if family == SourceFamily::AwAt {
            let file_type =
                AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
                    .ok_or_else(|| {
                        unavailable(
                            McdevErrorCode::NoApplicableContext,
                            "AW/AT file type is unknown",
                        )
                    })?;
            let line = usize::try_from(context.position.line).map_err(|_| {
                unavailable(
                    McdevErrorCode::NoApplicableContext,
                    "definition line is negative",
                )
            })?;
            let character = usize::try_from(context.position.character).map_err(|_| {
                unavailable(
                    McdevErrorCode::NoApplicableContext,
                    "definition character is negative",
                )
            })?;
            mapping_coverage::guard(
                self.mapping_discovery_complete(),
                file_type,
                &context.buffer_text,
                Some((line, character)),
            )?;
            let mappings =
                mapping_coverage::context(&project.mappings, self.mapping_discovery_complete());
            let service =
                AwAtServiceFacade::new(self.checked_class_index.as_ref(), Some(&mappings));
            let targets = catch_command(|| {
                service.definitions_at(&AwAtFacadeRequest {
                    buffer_text: &context.buffer_text,
                    line,
                    character,
                    document_uri: &context.document_uri,
                    file_type,
                })
            })?;
            self.validate_workspace(&context.workspace_root)?;
            return Ok(targets);
        }
        self.validate_semantic_capability(context)?;
        let mut facade = self.facade(context.document_version);
        facade.reference_types = Some(self.reference_types.as_ref());
        let line = usize::try_from(context.position.line).map_err(|_| {
            unavailable(
                McdevErrorCode::NoApplicableContext,
                "definition line is negative",
            )
        })?;
        let character = usize::try_from(context.position.character).map_err(|_| {
            unavailable(
                McdevErrorCode::NoApplicableContext,
                "definition character is negative",
            )
        })?;
        let targets = catch_facade(|| {
            facade.definitions(&context.buffer_text, line, character, &context.document_uri)
        })?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(targets)
    }

    fn resolve_definitions(
        &self,
        project: &ProjectContext,
        workspace_root: &str,
        targets: &[McDefinitionTarget],
    ) -> Result<Vec<ResolvedDefinition>, CommandFailure> {
        self.validate_project(project, workspace_root)?;
        let resolved = catch_command(|| {
            DefinitionResolutionService::new(vec![self.definitions.as_ref()]).resolve_all(
                targets,
                project,
                workspace_root,
            )
        })?
        .into_iter()
        .map(convert_definition)
        .collect();
        self.validate_workspace(workspace_root)?;
        Ok(resolved)
    }

    fn source_entries(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        family: SourceFamily,
    ) -> Result<Vec<SourceScanEntry>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        let entries = self.source_entries_for(context, family)?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(entries)
    }

    fn references(
        &self,
        project: &ProjectContext,
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<mcdev_core::McReferenceLocation>, CommandFailure> {
        self.validate_project(project, self.workspace_root_uri())?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let result = match family {
            SourceFamily::JavaKotlin => {
                mcdev_core::mixin_navigation::MixinReferenceService::find_references(
                    target, sources,
                )
            }
            SourceFamily::AwAt => {
                let mappings =
                    mapping_coverage::context(&project.mappings, self.mapping_discovery_complete());
                let service =
                    AwAtServiceFacade::new(self.checked_class_index.as_ref(), Some(&mappings));
                catch_command(|| service.find_references(target, sources))?
            }
        };
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        self.validate_workspace(self.workspace_root_uri())?;
        Ok(result)
    }

    fn code_actions(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        range: &McdevRange,
        diagnostic_code: Option<&str>,
        family: SourceFamily,
        cancellation: &dyn CancellationChecker,
    ) -> Result<Vec<McFix>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        if family == SourceFamily::AwAt {
            let file_type =
                AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
                    .ok_or_else(|| {
                        unavailable(
                            McdevErrorCode::NoApplicableContext,
                            "AW/AT file type is unknown",
                        )
                    })?;
            let line = usize::try_from(range.start.line).map_err(|_| {
                unavailable(
                    McdevErrorCode::NoApplicableContext,
                    "code action line is negative",
                )
            })?;
            let character = usize::try_from(range.start.character).map_err(|_| {
                unavailable(
                    McdevErrorCode::NoApplicableContext,
                    "code action character is negative",
                )
            })?;
            mapping_coverage::guard(
                self.mapping_discovery_complete(),
                file_type,
                &context.buffer_text,
                None,
            )?;
            let mappings =
                mapping_coverage::context(&project.mappings, self.mapping_discovery_complete());
            let service =
                AwAtServiceFacade::new(self.checked_class_index.as_ref(), Some(&mappings));
            let fixes = catch_command(|| {
                service.code_actions(
                    &AwAtFacadeRequest {
                        buffer_text: &context.buffer_text,
                        line,
                        character,
                        document_uri: &context.document_uri,
                        file_type,
                    },
                    diagnostic_code,
                )
            })?;
            self.validate_workspace(&context.workspace_root)?;
            return Ok(fixes);
        }
        if !HostMixinFacade::is_kotlin_document(&context.document_uri)
            && !diagnostic_code
                .is_some_and(MixinCodeActionService::is_config_independent_diagnostic)
        {
            self.require_config_selection()?;
        }
        self.validate_semantic_capability(context)?;
        let facade = self.facade(context.document_version);
        let request = facade
            .build_code_action_request(
                &context.buffer_text,
                &context.document_uri,
                range.start.line,
                range.start.character,
                diagnostic_code,
                self.source_provider.as_ref(),
            )
            .map_err(map_facade_error)?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        let mut facade = facade;
        facade.reference_types = Some(self.reference_types.as_ref());
        let fixes = catch_facade(|| facade.code_actions(&request, diagnostic_code))?;
        cancellation
            .check_cancelled()
            .map_err(CommandFailure::Cancelled)?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(fixes)
    }

    fn selected_mixin_config_content(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
    ) -> Result<Option<String>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        self.require_config_selection()?;
        let Some(config) = HostMixinFacade::selected_mixin_config(
            project,
            &context.buffer_text,
            Some(&context.document_uri),
            self.source_provider.as_ref(),
        )
        .map_err(map_facade_error)?
        else {
            self.validate_workspace(&context.workspace_root)?;
            return Ok(None);
        };
        let result = match self
            .source_provider
            .read_source_file(&config.path)
            .map_err(map_source_error)?
            .fact
        {
            HostFact::Known(content) => Ok(Some(content)),
            HostFact::Missing => Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                "selected mixin config content is missing",
            )),
            HostFact::Unavailable { reason } => Err(unavailable(
                McdevErrorCode::IncompleteProjectContext,
                reason,
            )),
        }?;
        self.validate_workspace(&context.workspace_root)?;
        Ok(result)
    }

    fn resolve_fix(
        &self,
        project: &ProjectContext,
        context: &McdevRequestContext,
        fix: &McFix,
        config_content: Option<&str>,
    ) -> Result<Option<(String, McFix)>, CommandFailure> {
        self.validate_project(project, &context.workspace_root)?;
        let result = if let Some(file_type) =
            AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri)
        {
            let resolved = if matches!(fix, McFix::WorkspaceEditFix { .. }) {
                Some(fix.clone())
            } else {
                mapping_coverage::guard(
                    self.mapping_discovery_complete(),
                    file_type,
                    &context.buffer_text,
                    None,
                )?;
                let mappings =
                    mapping_coverage::context(&project.mappings, self.mapping_discovery_complete());
                let service =
                    AwAtServiceFacade::new(self.checked_class_index.as_ref(), Some(&mappings));
                catch_command(|| service.apply_fix(fix, &context.buffer_text))?
            }
            .ok_or_else(|| {
                unavailable(
                    McdevErrorCode::IncompleteProjectContext,
                    "the AW/AT core could not apply the selected fix to the current buffer",
                )
            })?;
            Some((context.buffer_text.clone(), resolved))
        } else {
            self.resolve_java_fix(context, fix, config_content)?
        };
        self.validate_workspace(&context.workspace_root)?;
        Ok(result)
    }
}

fn unavailable(code: McdevErrorCode, message: impl Into<String>) -> CommandFailure {
    CommandFailure::Unavailable(McdevError {
        code,
        message: message.into(),
        details: Default::default(),
    })
}

fn bytecode_unavailable(message: impl Into<String>) -> CommandFailure {
    unavailable(McdevErrorCode::BytecodeUnavailable, message)
}

fn map_source_error(error: HostSourceError) -> CommandFailure {
    match error {
        HostSourceError::Cancelled(message) | HostSourceError::Interrupted(message) => {
            CommandFailure::Cancelled(Cancellation(Arc::new(std::io::Error::other(message))))
        }
        HostSourceError::Failure(message) => {
            unavailable(McdevErrorCode::IncompleteProjectContext, message)
        }
    }
}

fn map_facade_error(error: HostFacadeError) -> CommandFailure {
    match error {
        HostFacadeError::Source(error) => map_source_error(error),
        HostFacadeError::Cancelled(error) => CommandFailure::Cancelled(error),
        HostFacadeError::Unavailable(message) => {
            unavailable(McdevErrorCode::IncompleteProjectContext, message)
        }
    }
}

fn panic_checked_failure(error: HostSourceError) -> ! {
    let failure = match error {
        HostSourceError::Cancelled(message) | HostSourceError::Interrupted(message) => {
            CommandFailure::Cancelled(Cancellation(Arc::new(std::io::Error::other(message))))
        }
        HostSourceError::Failure(message) => bytecode_unavailable(message),
    };
    std::panic::panic_any(failure)
}

fn catch_command<T>(call: impl FnOnce() -> T) -> Result<T, CommandFailure> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)).map_err(panic_failure)
}

fn catch_facade<T>(call: impl FnOnce() -> Result<T, HostFacadeError>) -> Result<T, CommandFailure> {
    catch_command(call)?.map_err(map_facade_error)
}

fn panic_failure(payload: Box<dyn Any + Send>) -> CommandFailure {
    if let Some(error) = payload.downcast_ref::<CommandFailure>() {
        return error.clone();
    }
    if let Some(error) = payload.downcast_ref::<HostSourceError>() {
        return map_source_error(error.clone());
    }
    if let Some(error) = payload.downcast_ref::<HostFacadeError>() {
        return map_facade_error(error.clone());
    }
    if let Some(error) = payload.downcast_ref::<Cancellation>() {
        return CommandFailure::Cancelled(error.clone());
    }
    CommandFailure::Runtime("core command panicked".into())
}

pub(super) fn convert_definition(
    value: mcdev_core::mixin_navigation::ResolvedDefinition,
) -> ResolvedDefinition {
    ResolvedDefinition {
        target: value.target,
        document_uri: value.document_uri,
        range: value.range,
        resolution: match value.resolution {
            DefinitionResolution::Source => McdevDefinitionResolution::Source,
            DefinitionResolution::Jdt => McdevDefinitionResolution::Jdt,
            DefinitionResolution::BytecodeOnly => McdevDefinitionResolution::BytecodeOnly,
            DefinitionResolution::Unresolved => McdevDefinitionResolution::Unresolved,
        },
        resolution_message: value.resolution_message,
    }
}
