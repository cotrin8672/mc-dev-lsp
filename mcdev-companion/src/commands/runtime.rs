use super::{cache::*, host::*};
use mcdev_core::{
    awat::AwAtBufferSupport,
    expression_engine::{CancellationChecker, NO_CANCELLATION},
    mixin_actions::MixinCodeActionService,
    mixin_navigation::{reference_document_key, ReferenceDocumentKey},
    mixin_source::{JavaTypeDescriptorResolver, ParseConfidence, ParseSource},
    project::{InfoService, ProjectContext},
    McDefinitionTarget, MemberKind,
};
use mcdev_protocol::{
    convert::{range_to_dto, CodeActionConverter, DefinitionConverter, DiagnosticConverter},
    *,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

pub type CommandEnvelope = McdevResponseEnvelope<Value>;
pub struct CommandDispatcher<H: CommandHost> {
    pub(super) host: Arc<H>,
    pub(super) negative: Mutex<NegativeCompletionCache>,
    pub(super) documents: Mutex<DocumentSnapshotCache>,
    pub(super) in_flight: Mutex<BTreeMap<String, Arc<Mutex<()>>>>,
    registered_commands: Vec<String>,
    project_complete: bool,
    config_complete: bool,
    references_complete: bool,
    evaluation_scoped: bool,
}
pub(super) fn runtime_error(error: impl std::fmt::Display) -> CommandFailure {
    CommandFailure::Runtime(error.to_string())
}
pub(super) fn success(
    result: impl Serialize,
    caps: &[&str],
) -> Result<CommandEnvelope, CommandFailure> {
    Ok(McdevResponseEnvelope::success(
        serde_json::to_value(result).map_err(runtime_error)?,
        caps.iter().map(|s| s.to_string()),
    ))
}
pub(super) fn failure(code: McdevErrorCode, message: impl Into<String>) -> CommandEnvelope {
    McdevResponseEnvelope::failure(McdevError {
        code,
        message: message.into(),
        details: Default::default(),
    })
}
pub(super) fn family(context: &McdevRequestContext) -> SourceFamily {
    if AwAtBufferSupport::detect_file_type(&context.language_id, &context.document_uri).is_some() {
        SourceFamily::AwAt
    } else {
        SourceFamily::JavaKotlin
    }
}
pub(super) fn is_kotlin(context: &McdevRequestContext) -> bool {
    JavaTypeDescriptorResolver::is_kotlin_source(&context.buffer_text, &context.document_uri)
}
impl<H: CommandHost> CommandDispatcher<H> {
    pub fn new(host: Arc<H>) -> Self {
        Self {
            host,
            negative: Mutex::new(Default::default()),
            documents: Mutex::new(Default::default()),
            in_flight: Mutex::new(Default::default()),
            registered_commands: COMMANDS.iter().map(|command| command.id().into()).collect(),
            project_complete: true,
            config_complete: true,
            references_complete: true,
            evaluation_scoped: false,
        }
    }

    /// Restrict one evaluation to its acknowledged commands and legacy discovery coverage.
    pub fn for_evaluation(host: Arc<H>, commands: &[String], project_complete: bool) -> Self {
        let mut dispatcher = Self::new(host);
        dispatcher
            .registered_commands
            .retain(|command| commands.contains(command));
        dispatcher.project_complete = project_complete;
        dispatcher.config_complete = project_complete;
        dispatcher.references_complete = project_complete;
        dispatcher.evaluation_scoped = true;
        dispatcher
    }

    /// Keep independent root discovery domains separate from the full context DTO.
    pub fn with_discovery_coverage(
        mut self,
        config_complete: bool,
        references_complete: bool,
    ) -> Self {
        self.config_complete = config_complete;
        self.references_complete = references_complete;
        self
    }

    fn require_discovery(complete: bool) -> Result<(), CommandFailure> {
        if complete {
            Ok(())
        } else {
            Err(CommandFailure::Unavailable(McdevError {
                code: McdevErrorCode::IncompleteProjectContext,
                message: "complete project source-set and resource inventory is unavailable".into(),
                details: Default::default(),
            }))
        }
    }
    pub fn execute(
        &self,
        command: &str,
        args: &[Value],
    ) -> Result<CommandEnvelope, CommandFailure> {
        self.execute_with_cancellation(command, args, &NO_CANCELLATION)
    }
    pub fn execute_with_cancellation(
        &self,
        command: &str,
        args: &[Value],
        cancellation: &dyn CancellationChecker,
    ) -> Result<CommandEnvelope, CommandFailure> {
        let decoded = match ProtocolPayloadDecoder.decode_command(command, args) {
            Ok(v) => v,
            Err(e) => return Ok(McdevResponseEnvelope::failure(e.as_protocol_error())),
        };
        self.execute_decoded(decoded, false, cancellation)
    }
    pub fn execute_project_aware_completion(
        &self,
        args: &[Value],
        cancellation: &dyn CancellationChecker,
    ) -> Result<CommandEnvelope, CommandFailure> {
        let request = match ProtocolPayloadDecoder.decode_completion_request(args) {
            Ok(v) => v,
            Err(e) => return Ok(McdevResponseEnvelope::failure(e.as_protocol_error())),
        };
        self.execute_decoded(DecodedCommand::Completion(request), true, cancellation)
    }
    pub fn execute_decoded(
        &self,
        command: DecodedCommand,
        project_aware: bool,
        cancellation: &dyn CancellationChecker,
    ) -> Result<CommandEnvelope, CommandFailure> {
        let result = self.dispatch(command, project_aware, cancellation);
        match result {
            Err(CommandFailure::Unavailable(error)) => Ok(McdevResponseEnvelope::failure(error)),
            Err(CommandFailure::Runtime(message)) => {
                Ok(failure(McdevErrorCode::InternalError, message))
            }
            other => other,
        }
    }
    pub(super) fn validate(
        &self,
        context: &McdevRequestContext,
        require_project: bool,
    ) -> Result<Option<CommandEnvelope>, CommandFailure> {
        if context.protocol_version != PROTOCOL_VERSION {
            return Ok(Some(protocol_mismatch(context.protocol_version)));
        }
        if context.workspace_root.trim().is_empty() {
            return Ok(Some(failure(
                McdevErrorCode::IncompleteProjectContext,
                "workspace root is required",
            )));
        }
        if require_project && is_kotlin(context) && !self.host.has_project_facts(context)? {
            return Ok(Some(failure(
                McdevErrorCode::IncompleteProjectContext,
                "JDT project dependencies are not available yet",
            )));
        }
        Ok(None)
    }
    fn dispatch(
        &self,
        command: DecodedCommand,
        project_aware: bool,
        cancellation: &dyn CancellationChecker,
    ) -> Result<CommandEnvelope, CommandFailure> {
        if let DecodedCommand::Completion(request) = command {
            return self.handle_completion(&request, project_aware, cancellation);
        }
        let context = command.context();
        if let Some(response) =
            self.validate(context, !matches!(command, DecodedCommand::CodeAction(_)))?
        {
            return Ok(response);
        }
        match command {
            DecodedCommand::Info(request) => {
                Self::require_discovery(self.project_complete)?;
                let project = self.host.load_session(&request.context.workspace_root)?;
                let build = self.host.build_identity()?;
                let commands = self.registered_commands.clone();
                let mut lines =
                    InfoService::format_lines(&project, PROTOCOL_VERSION as u32, &build.version);
                lines.extend([
                    "Extension loaded: true".into(),
                    format!("Extension build commit: {}", build.commit),
                    format!("Extension build time: {}", build.build_time),
                    format!("Extension jar location: {}", build.artifact_location),
                    "Registered commands:".into(),
                ]);
                lines.extend(commands.iter().map(|c| format!("  - {c}")));
                success(
                    McdevInfoResponse {
                        lines,
                        build_commit: Some(build.commit),
                        build_time: Some(build.build_time),
                        version: Some(build.version),
                        jar_location: Some(build.artifact_location),
                        registered_commands: commands,
                    },
                    &[
                        "info",
                        "completion",
                        "diagnostics",
                        "hover",
                        "definition",
                        "references",
                        "codeAction",
                        "reindex",
                        "projectContext",
                    ],
                )
            }
            DecodedCommand::Reindex(request) => {
                let project = self.host.reindex(&request.context.workspace_root)?;
                Self::require_discovery(self.project_complete)?;
                success(
                    McdevReindexResponse {
                        status: "reindex complete".into(),
                        index_state: InfoService::index_state_name(project.index_state).into(),
                        classpath_entries: i32::try_from(project.classpath.entry_count())
                            .map_err(runtime_error)?,
                    },
                    &["reindex"],
                )
            }
            DecodedCommand::ReloadProjectContext(request) => {
                self.host.clear_project_cache()?;
                let project = self.host.reindex(&request.context.workspace_root)?;
                Self::require_discovery(self.project_complete)?;
                success(
                    McdevReloadProjectContextResponse {
                        status: "project context reloaded".into(),
                        index_state: InfoService::index_state_name(project.index_state).into(),
                        classpath_entries: i32::try_from(project.classpath.entry_count())
                            .map_err(runtime_error)?,
                        lines: InfoService::format_lines(
                            &project,
                            PROTOCOL_VERSION as u32,
                            SERVER_VERSION,
                        ),
                    },
                    &["reloadProjectContext", "dumpContext", "info", "reindex"],
                )
            }
            DecodedCommand::DumpContext(request) => {
                Self::require_discovery(self.project_complete)?;
                let project = self.host.load_session(&request.context.workspace_root)?;
                success(dump(&project)?, &["dumpContext", "info"])
            }
            DecodedCommand::Diagnostics(request) | DecodedCommand::Context(request) => {
                let project = self.host.load_session(&request.context.workspace_root)?;
                let diagnostics = self.host.diagnostics(
                    &project,
                    &request.context,
                    family(&request.context),
                    cancellation,
                )?;
                success(
                    McdevDiagnosticsResponse {
                        diagnostics: DiagnosticConverter::to_dtos(&diagnostics)
                            .map_err(runtime_error)?,
                    },
                    &["diagnostics"],
                )
            }
            DecodedCommand::Definition(request) => {
                let project = self.host.load_session(&request.context.workspace_root)?;
                let targets = self.host.definitions(
                    &project,
                    &request.context,
                    family(&request.context),
                    cancellation,
                )?;
                let resolved = self.host.resolve_definitions(
                    &project,
                    &request.context.workspace_root,
                    &targets,
                )?;
                success(
                    McdevDefinitionResponse {
                        locations: DefinitionConverter::to_locations(&resolved)
                            .map_err(runtime_error)?,
                    },
                    &["definition"],
                )
            }
            DecodedCommand::References(request) => {
                let project = self.host.load_session(&request.context.workspace_root)?;
                let targets = self.host.definitions(
                    &project,
                    &request.context,
                    family(&request.context),
                    cancellation,
                )?;
                if targets.is_empty() {
                    if self.evaluation_scoped
                        && family(&request.context) == SourceFamily::JavaKotlin
                    {
                        let semantic = self.host.semantic_model(
                            &request.context,
                            request.context.document_version.unwrap_or_default(),
                            &project,
                            cancellation,
                        )?;
                        if semantic.model.parse_source != ParseSource::HostNormalized
                            || semantic.model.confidence != ParseConfidence::High
                        {
                            return Ok(failure(
                                McdevErrorCode::IncompleteProjectContext,
                                "host source declaration coverage and ranges are unavailable",
                            ));
                        }
                    }
                    return success(
                        McdevReferencesResponse { locations: vec![] },
                        &["references"],
                    );
                }
                Self::require_discovery(self.references_complete)?;
                let java = self.host.source_entries(
                    &project,
                    &request.context,
                    SourceFamily::JavaKotlin,
                )?;
                let awat =
                    self.host
                        .source_entries(&project, &request.context, SourceFamily::AwAt)?;
                // Recorded legacy hosts keep raw URI identity in their callbacks and DTOs.
                let document_key = |uri: &str| {
                    if self.evaluation_scoped {
                        reference_document_key(uri)
                    } else {
                        ReferenceDocumentKey::Opaque(uri.into())
                    }
                };
                let mut seen = BTreeSet::new();
                let sources = java
                    .into_iter()
                    .chain(awat.iter().cloned())
                    .filter(|s| seen.insert(document_key(&s.document_uri)))
                    .collect::<Vec<_>>();
                let mut references = vec![];
                for target in targets {
                    let java_refs = self.host.references(
                        &project,
                        &target,
                        &sources,
                        SourceFamily::JavaKotlin,
                        cancellation,
                    )?;
                    let awat_refs = self.host.references(
                        &project,
                        &target,
                        &awat,
                        SourceFamily::AwAt,
                        cancellation,
                    )?;
                    let mut seen = BTreeSet::new();
                    references.extend(java_refs.into_iter().chain(awat_refs).filter(|r| {
                        seen.insert((
                            document_key(&r.document_uri),
                            r.range.start.line,
                            r.range.start.character,
                        ))
                    }));
                }
                success(
                    McdevReferencesResponse {
                        locations: DefinitionConverter::references_to_locations(&references)
                            .map_err(runtime_error)?,
                    },
                    &["references"],
                )
            }
            DecodedCommand::Hover(request) => {
                let project = self.host.load_session(&request.context.workspace_root)?;
                let targets = self.host.definitions(
                    &project,
                    &request.context,
                    family(&request.context),
                    cancellation,
                )?;
                let mut seen = BTreeSet::new();
                let contents = targets
                    .iter()
                    .filter(|t| {
                        seen.insert((
                            format!("{:?}", t.kind),
                            t.owner_internal_name.clone(),
                            t.name.clone(),
                            t.descriptor.clone(),
                        ))
                    })
                    .map(format_hover)
                    .collect();
                success(
                    McdevHoverResponse {
                        contents,
                        range: targets
                            .first()
                            .and_then(|t| t.source_range)
                            .map(range_to_dto)
                            .transpose()
                            .map_err(runtime_error)?,
                    },
                    &["hover"],
                )
            }
            DecodedCommand::CodeAction(request) => {
                if is_kotlin(&request.context) {
                    return success(McdevCodeActionResponse { actions: vec![] }, &["codeAction"]);
                }
                let source_family = family(&request.context);
                let config_independent = !request.diagnostic_codes.is_empty()
                    && request
                        .diagnostic_codes
                        .iter()
                        .all(|code| MixinCodeActionService::is_config_independent_diagnostic(code));
                if source_family == SourceFamily::JavaKotlin && !config_independent {
                    Self::require_discovery(self.config_complete)?;
                }
                let project = self.host.load_session(&request.context.workspace_root)?;
                // Legacy hosts retain this callback for AW/AT and independent fixes too.
                let config = if !self.evaluation_scoped
                    || (source_family == SourceFamily::JavaKotlin && !config_independent)
                {
                    self.host
                        .selected_mixin_config_content(&project, &request.context)?
                } else {
                    None
                };
                let mut fixes = vec![];
                if request.diagnostic_codes.is_empty() {
                    fixes = self.host.code_actions(
                        &project,
                        &request.context,
                        &request.range,
                        None,
                        family(&request.context),
                        cancellation,
                    )?;
                } else {
                    for code in &request.diagnostic_codes {
                        fixes.extend(self.host.code_actions(
                            &project,
                            &request.context,
                            &request.range,
                            Some(code),
                            family(&request.context),
                            cancellation,
                        )?);
                    }
                    fixes=mcdev_core::mixinextras_actions::MixinExtrasCodeActionService::deduplicate_fixes(&fixes);
                }
                let mut actions = vec![];
                for fix in &fixes {
                    let resolved = if matches!(
                        fix,
                        mcdev_core::McFix::WorkspaceEditFix { .. }
                            | mcdev_core::McFix::GenerateAccessorMethodFix { .. }
                            | mcdev_core::McFix::GenerateInvokerMethodFix { .. }
                    ) {
                        None
                    } else {
                        self.host
                            .resolve_fix(&project, &request.context, fix, config.as_deref())?
                    };
                    if let Some(action) =
                        CodeActionConverter::to_dto(fix, &request.context.buffer_text, |_| {
                            resolved
                                .as_ref()
                                .map(|(source, workspace)| (source.as_str(), workspace.clone()))
                        })
                        .map_err(runtime_error)?
                    {
                        actions.push(action);
                    }
                }
                success(McdevCodeActionResponse { actions }, &["codeAction"])
            }
            DecodedCommand::Completion(_) => unreachable!(),
        }
    }
}
fn format_hover(target: &McDefinitionTarget) -> String {
    let owner = target
        .owner_fqn
        .clone()
        .unwrap_or_else(|| target.owner_internal_name.replace('/', "."));
    let name = target.name.as_deref().unwrap_or("null");
    let descriptor = target.descriptor.as_deref().unwrap_or("");
    let (kind, symbol) = match target.kind {
        MemberKind::Class => ("class", owner),
        MemberKind::Method => ("method", format!("{owner}.{name}{descriptor}")),
        MemberKind::Field => ("field", format!("{owner}.{name}:{descriptor}")),
        MemberKind::Constructor => ("constructor", format!("{owner}.<init>{descriptor}")),
    };
    format!(
        "```mcdev\n{kind} {symbol}\nowner: {}\n{}namespace: {}\n```",
        target.owner_internal_name,
        target
            .descriptor
            .as_ref()
            .map_or(String::new(), |d| format!("descriptor: {d}\n")),
        InfoService::namespace_name(target.namespace)
    )
}
fn paths(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}
fn dump(project: &ProjectContext) -> Result<McdevDumpContextResponse, CommandFailure> {
    let mappings = &project.mappings;
    let classpath = &project.classpath;
    Ok(McdevDumpContextResponse {
        lines: InfoService::format_lines(project, PROTOCOL_VERSION as u32, SERVER_VERSION),
        project_id: project.project_id.clone(),
        root: project.root.to_string_lossy().into_owned(),
        platform: format!("{:?}", project.platform).to_ascii_lowercase(),
        mappings: McdevMappingContextDto {
            source_namespace: InfoService::namespace_name(mappings.source_namespace).into(),
            runtime_namespace: InfoService::namespace_name(mappings.runtime_namespace).into(),
            aw_namespace: mappings
                .aw_namespace
                .map(|n| InfoService::namespace_name(n).into()),
            at_namespace: mappings
                .at_namespace
                .map(|n| InfoService::namespace_name(n).into()),
            available_namespaces: mappings
                .available_namespaces
                .iter()
                .map(|n| InfoService::namespace_name(*n).to_string())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        },
        classpath: McdevClasspathDto {
            entry_count: i32::try_from(classpath.entry_count()).map_err(runtime_error)?,
            project_outputs: paths(&classpath.project_outputs),
            dependency_jars: paths(&classpath.dependency_jars),
            minecraft_jars: paths(&classpath.minecraft_jars),
            generated_outputs: paths(&classpath.generated_outputs),
            captured_at: i64::try_from(classpath.captured_at).map_err(runtime_error)?,
        },
        source_sets: project
            .source_sets
            .iter()
            .map(|s| McdevSourceSetDto {
                name: s.name.clone(),
                source_directories: paths(&s.source_directories),
                resource_directories: paths(&s.resource_directories),
                output_directory: s
                    .output_directory
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
            })
            .collect(),
        mixin_configs: project
            .mixin_configs
            .iter()
            .map(|r| r.path.to_string_lossy().into_owned())
            .collect(),
        access_wideners: project
            .access_wideners
            .iter()
            .map(|r| r.path.to_string_lossy().into_owned())
            .collect(),
        access_transformers: project
            .access_transformers
            .iter()
            .map(|r| r.path.to_string_lossy().into_owned())
            .collect(),
        minecraft_jars: paths(&project.minecraft_jars),
        index_state: InfoService::index_state_name(project.index_state).into(),
    })
}
