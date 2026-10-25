use mcdev_companion::commands::{
    BuildIdentity, CommandDispatcher, CommandFailure, CommandHost, CoreCommandHost, SourceFamily,
};
use mcdev_core::{
    host_project::{
        HostBytesPurpose, HostClassIndexOverlay, HostClasspathEntry, HostProjectQuery,
        HostProjectSession, HostSessionIdentity,
    },
    host_source::{
        HostAnswer, HostFact, HostGenerations, HostReadSet, HostReadStamp, HostReadValidator,
        HostSemanticModel, HostSemanticModelProvider, HostSemanticRequest, HostSourceError,
        ProjectSourceQuery, SemanticAvailability, SourceAuthority, SourceQueryResult,
    },
    mixin_facade::{HostMixinSourceProvider, RegisteredMixinSource},
    mixin_source::{MixinSemanticModelParser, ParseConfidence, ParseSource},
    mixinextras_handler::HandlerReferenceTypeResolver,
    model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry, SourceScanEntry},
    project::{
        path_to_uri, CachedProjectSession, ClasspathSnapshot, HostProjectFacts, MixinConfigRef,
        ProjectContextBuilder,
    },
};
use mcdev_protocol::{
    McdevClientInfo, McdevCodeActionResponse, McdevCommands, McdevCompletionOptions,
    McdevCompletionRequest, McdevCompletionResponse, McdevCompletionTrigger,
    McdevDiagnosticsResponse, McdevErrorCode, McdevPosition, McdevRange, McdevRequestContext,
    McdevTextEdit, PROTOCOL_VERSION,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

const SOURCE: &str = r#"@Mixin(Target.class) class Patch { @Inject(method = "ti", at = @At("HEAD")) void handler() {} }"#;
const EXPRESSION_SOURCE: &str = r#"@Mixin(Target.class) class Patch { @ModifyExpressionValue(method = "tick", at = @At("MIXINEXTRAS:EXPRESSION")) @Expression("Target.") int handler(int value) { return value; } }"#;
const REFERENCE_SOURCE: &str = r#"@Mixin(Target.class) class Patch { @Inject(method = "tick", at = @At("HEAD")) UnknownType handler(UnknownType value) { return value; } }"#;
const CONFIG_SOURCE: &str = r#"package fixture; @Mixin(Target.class) class Patch {}"#;
const CONFIG_CONTENT: &str = r#"{"package":"fixture","mixins":[]}"#;

#[derive(Clone)]
struct Query {
    open: Arc<AtomicBool>,
    stale: Arc<AtomicBool>,
    catalog_reads: Arc<AtomicUsize>,
    bytes_reads: Arc<AtomicUsize>,
    catalog_unavailable: bool,
    bytes_unavailable: bool,
}

impl HostProjectQuery for Query {
    fn is_open(&self, _identity: &HostSessionIdentity) -> bool {
        self.open.load(Ordering::Acquire)
    }

    fn validate(
        &self,
        _identity: &HostSessionIdentity,
        _reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        Ok(!self.stale.load(Ordering::Acquire))
    }

    fn catalog(
        &self,
        _identity: &HostSessionIdentity,
        _entry: &HostClasspathEntry,
    ) -> Result<HostAnswer<Vec<String>>, HostSourceError> {
        self.catalog_reads.fetch_add(1, Ordering::AcqRel);
        if self.catalog_unavailable {
            Ok(HostAnswer::unavailable(
                "generic class catalog is unavailable",
            ))
        } else {
            Ok(HostAnswer::known(vec!["game/Target".into()], vec![]))
        }
    }

    fn class_bytes(
        &self,
        _identity: &HostSessionIdentity,
        _entry: &HostClasspathEntry,
        _owner: &str,
        _purpose: HostBytesPurpose,
    ) -> Result<HostAnswer<Arc<[u8]>>, HostSourceError> {
        self.bytes_reads.fetch_add(1, Ordering::AcqRel);
        if self.bytes_unavailable {
            Ok(HostAnswer::unavailable(
                "target class bytes are unavailable",
            ))
        } else {
            Ok(HostAnswer::missing(vec![]))
        }
    }
}

struct SemanticFacts {
    stale_after_parse: Option<Arc<AtomicBool>>,
}

impl HostReadValidator for SemanticFacts {
    fn validate_current_reads(
        &self,
        _generations: HostGenerations,
        _reads: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        Ok(true)
    }
}

impl HostSemanticModelProvider for SemanticFacts {
    fn parse(
        &self,
        request: &HostSemanticRequest<'_>,
    ) -> Result<HostAnswer<HostSemanticModel>, HostSourceError> {
        let mut model = MixinSemanticModelParser::parse(request.source, request.source_uri);
        for target in &mut model.targets {
            if target.internal_name == "Target" {
                target.internal_name = "game/Target".into();
            }
        }
        model.parse_source = ParseSource::HostNormalized;
        model.confidence = ParseConfidence::High;
        model.debug_info.parse_source = ParseSource::HostNormalized;
        model.debug_info.used_java_project = true;
        let answer = HostAnswer::known(
            HostSemanticModel {
                model,
                availability: SemanticAvailability::Normalized,
                parse_ms: 0,
            },
            vec![HostReadStamp {
                kind: "semantic".into(),
                key: request.source_uri.into(),
                generation: 1,
                missing: false,
            }],
        );
        if let Some(stale) = &self.stale_after_parse {
            stale.store(true, Ordering::Release);
        }
        Ok(answer)
    }
}

struct ClassFacts {
    expose_target: bool,
}

struct CompleteClassSource {
    expose_target: bool,
}

impl CompleteClassSource {
    fn answer<T>(value: T) -> Result<SourceQueryResult<T>, HostSourceError> {
        Ok(SourceQueryResult::new(value, SourceAuthority::Complete))
    }

    fn target() -> ClassIndexEntry {
        ClassIndexEntry {
            simple_name: "Target".into(),
            package_name: "game".into(),
            internal_name: "game/Target".into(),
        }
    }

    fn tick() -> MethodIndexEntry {
        MethodIndexEntry {
            name: "tick".into(),
            descriptor: "()V".into(),
            is_static: false,
            readable_signature: "tick(): void".into(),
        }
    }
}

impl ProjectSourceQuery for CompleteClassSource {
    fn find_classes(
        &self,
        _prefix: &str,
        limit: usize,
    ) -> Result<SourceQueryResult<Vec<ClassIndexEntry>>, HostSourceError> {
        Self::answer(if self.expose_target {
            vec![Self::target()].into_iter().take(limit).collect()
        } else {
            vec![]
        })
    }

    fn find_class(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        Self::answer((self.expose_target && owner == "game/Target").then(Self::target))
    }

    fn find_class_by_fqn(
        &self,
        fqn: &str,
    ) -> Result<SourceQueryResult<Option<ClassIndexEntry>>, HostSourceError> {
        Self::answer((self.expose_target && fqn == "game.Target").then(Self::target))
    }

    fn get_methods(
        &self,
        owner: &str,
    ) -> Result<SourceQueryResult<Vec<MethodIndexEntry>>, HostSourceError> {
        Self::answer(if self.expose_target && owner == "game/Target" {
            vec![Self::tick()]
        } else {
            vec![]
        })
    }

    fn get_fields(
        &self,
        _owner: &str,
    ) -> Result<SourceQueryResult<Vec<FieldIndexEntry>>, HostSourceError> {
        Self::answer(vec![])
    }

    fn covers_project_dependencies(&self) -> bool {
        true
    }
}

impl ClassIndex for ClassFacts {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        if self.expose_target && "Target".starts_with(prefix) {
            vec![self.target()].into_iter().take(limit).collect()
        } else {
            vec![]
        }
    }

    fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry> {
        (self.expose_target && matches!(internal_name, "Target" | "game/Target"))
            .then(|| self.target())
    }

    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        self.find_class(&fqn.replace('.', "/"))
    }

    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        if self.expose_target && owner == "game/Target" {
            vec![MethodIndexEntry {
                name: "tick".into(),
                descriptor: "()V".into(),
                is_static: false,
                readable_signature: "tick(): void".into(),
            }]
        } else {
            vec![]
        }
    }

    fn get_fields(&self, _owner: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}

impl ClassFacts {
    fn target(&self) -> ClassIndexEntry {
        ClassIndexEntry {
            simple_name: "Target".into(),
            package_name: "game".into(),
            internal_name: "game/Target".into(),
        }
    }
}

struct Sources {
    config: MixinConfigRef,
}

impl HostMixinSourceProvider for Sources {
    fn config(&self, path: &Path) -> Result<HostAnswer<MixinConfigRef>, HostSourceError> {
        if path == self.config.path {
            Ok(HostAnswer::known(self.config.clone(), vec![]))
        } else {
            Ok(HostAnswer::missing(vec![]))
        }
    }

    fn compilation_unit_source(
        &self,
        _fqn: &str,
        _current_path: Option<&Path>,
    ) -> Result<HostAnswer<RegisteredMixinSource>, HostSourceError> {
        Ok(HostAnswer::missing(vec![]))
    }

    fn read_source_file(&self, path: &Path) -> Result<HostAnswer<String>, HostSourceError> {
        if path == self.config.path {
            Ok(HostAnswer::known(CONFIG_CONTENT.into(), vec![]))
        } else {
            Ok(HostAnswer::missing(vec![]))
        }
    }
}

struct Fixture {
    host: Arc<CoreCommandHost>,
    dispatcher: CommandDispatcher<CoreCommandHost>,
    session: Arc<HostProjectSession>,
    workspace: String,
    query: Query,
}

fn fixture(
    source_overlay: bool,
    catalog_unavailable: bool,
    bytes_unavailable: bool,
    source_inventory: HostFact<Vec<SourceScanEntry>>,
    stale_during_compute: bool,
) -> Fixture {
    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "mcdev-core-command-fixture-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    let workspace = path_to_uri(&root);
    let config_path = root
        .join("src")
        .join("main")
        .join("resources")
        .join("fixture.mixins.json");
    let config = MixinConfigRef {
        path: config_path,
        package_name: Some("fixture".into()),
        ..Default::default()
    };

    let classpath_entry = root.join("classes");
    let mut classpath = ClasspathSnapshot::default();
    classpath.project_outputs.push(classpath_entry.clone());
    classpath.entry_timestamps.insert(classpath_entry, 1);
    let facts = HostProjectFacts {
        classpath,
        project_generation: 1,
        dependency_generation: 1,
        generated_source_generation: 1,
        config_generation: 1,
        provider_generation: 1,
        ..Default::default()
    };
    let identity = HostSessionIdentity {
        workspace_root_uri: workspace.clone(),
        project_session_version: 7,
        facts,
        generations: HostGenerations {
            project: 1,
            model: 1,
            namespace: 1,
            classpath: 1,
            dependency: 1,
            generated_source: 1,
            config: 1,
            provider: 1,
        },
    };
    let query = Query {
        open: Arc::new(AtomicBool::new(true)),
        stale: Arc::new(AtomicBool::new(false)),
        catalog_reads: Arc::new(AtomicUsize::new(0)),
        bytes_reads: Arc::new(AtomicUsize::new(0)),
        catalog_unavailable,
        bytes_unavailable,
    };
    let query_arc: Arc<dyn HostProjectQuery> = Arc::new(query.clone());
    let mut context = ProjectContextBuilder::empty("fixture", root);
    context.mixin_configs.push(config.clone());
    let context = Arc::new(context);
    let session = if source_overlay {
        let class_facts = Arc::new(ClassFacts {
            expose_target: true,
        });
        let overlay: Arc<HostClassIndexOverlay> = Arc::new(move |_binary| class_facts.clone());
        let source: Arc<dyn ProjectSourceQuery> = Arc::new(CompleteClassSource {
            expose_target: true,
        });
        Arc::new(HostProjectSession::with_checked_source_index(
            context,
            identity,
            query_arc.clone(),
            overlay,
            source,
        ))
    } else {
        Arc::new(HostProjectSession::create(
            context,
            identity,
            query_arc.clone(),
        ))
    };
    let cached_session = CachedProjectSession {
        context: session.context.clone(),
        version: session.identity.project_session_version,
        cache_hit: true,
        fingerprint: "fixture-fingerprint".into(),
    };
    let host = Arc::new(
        CoreCommandHost::new(
            session.clone(),
            cached_session,
            query_arc,
            Arc::new(SemanticFacts {
                stale_after_parse: stale_during_compute.then_some(query.stale.clone()),
            }),
            Arc::new(Sources { config }),
            source_inventory,
            BuildIdentity {
                commit: "fixture-commit".into(),
                build_time: "fixture-time".into(),
                version: "fixture-version".into(),
                artifact_location: "fixture-artifact".into(),
            },
            true,
        )
        .expect("fixture host identity is valid"),
    );
    let dispatcher = CommandDispatcher::new(host.clone());
    Fixture {
        host,
        dispatcher,
        session,
        workspace,
        query,
    }
}

fn context(
    fixture: &Fixture,
    source: &str,
    uri: &str,
    position: McdevPosition,
) -> McdevRequestContext {
    McdevRequestContext {
        protocol_version: PROTOCOL_VERSION,
        workspace_root: fixture.workspace.clone(),
        document_uri: uri.into(),
        language_id: "java".into(),
        position,
        buffer_text: source.into(),
        document_version: Some(1),
        buffer_text_fallback_used: false,
        client: McdevClientInfo {
            name: "core-command-fixture".into(),
            version: "1".into(),
        },
    }
}

fn completion_request(context: McdevRequestContext) -> McdevCompletionRequest {
    McdevCompletionRequest {
        context,
        trigger: McdevCompletionTrigger {
            kind: "manual".into(),
            character: None,
        },
        options: McdevCompletionOptions::default(),
    }
}

fn envelope(
    fixture: &Fixture,
    command: &str,
    payload: impl serde::Serialize,
) -> mcdev_companion::commands::CommandEnvelope {
    fixture
        .dispatcher
        .execute(command, &[serde_json::to_value(payload).unwrap()])
        .unwrap()
}

fn without_completion_measurements(mut value: serde_json::Value) -> serde_json::Value {
    for key in [
        "totalMs",
        "payloadBytes",
        "bufferTextBytes",
        "loadSessionMs",
        "documentSnapshotMs",
        "astParseMs",
        "candidateBuildMs",
        "dtoConvertMs",
    ] {
        value["debug"].as_object_mut().unwrap().remove(key);
    }
    value
}

fn utf16_offset(text: &str, line: i32, character: i32) -> usize {
    let line = usize::try_from(line).unwrap();
    let character = usize::try_from(character).unwrap();
    let start = if line == 0 {
        0
    } else {
        text.match_indices('\n')
            .nth(line - 1)
            .map_or(text.len(), |(offset, _)| offset + 1)
    };
    let end = text[start..]
        .find('\n')
        .map_or(text.len(), |offset| start + offset);
    let mut utf16 = 0;
    for (offset, value) in text[start..end].char_indices() {
        if utf16 >= character {
            return start + offset;
        }
        utf16 += value.len_utf16();
    }
    end
}

fn apply_protocol_edits(text: &str, edits: &[McdevTextEdit]) -> String {
    let mut edits = edits
        .iter()
        .map(|edit| {
            (
                utf16_offset(text, edit.range.start.line, edit.range.start.character),
                utf16_offset(text, edit.range.end.line, edit.range.end.character),
                edit.new_text.clone(),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by(|left, right| right.0.cmp(&left.0));
    let mut result = text.to_owned();
    for (start, end, replacement) in edits {
        result.replace_range(start..end, &replacement);
    }
    result
}

#[test]
fn dispatcher_reaches_core_for_completion_diagnostics_and_config_edit() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);

    let cursor = SOURCE.find("\"ti\"").unwrap() + 3;
    let completion = fixture
        .dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(completion_request(context(
                &fixture,
                SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: cursor as i32,
                },
            )))
            .unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert!(
        completion.error.is_none(),
        "completion error: {:?}",
        completion.error
    );
    let completion: McdevCompletionResponse =
        serde_json::from_value(completion.result.unwrap()).unwrap();
    let completion_json =
        without_completion_measurements(serde_json::to_value(&completion).unwrap());
    assert_eq!(
        completion_json,
        serde_json::json!({
            "items": [{
                "label": "tick(): void",
                "detail": "game.Target",
                "documentation": "()V",
                "filterText": "tick tick(): void",
                "insertText": "tick",
                "kind": "value",
                "sortKey": "0200_tick",
                "edit": {
                    "range": {
                        "start": {"line": 0, "character": 53},
                        "end": {"line": 0, "character": 55}
                    },
                    "newText": "tick"
                },
                "additionalEdits": [],
                "metadata": {
                    "descriptor": "()V",
                    "name": "tick",
                    "namespace": "NAMED",
                    "owner": "game/Target",
                    "source": "mixin.injectMethod"
                },
                "insertTextFormat": "plainText"
            }],
            "isIncomplete": false,
            "warnings": [{"code": "MIXIN_PARSE_SOURCE", "message": "HOST_NORMALIZED"}],
            "debug": {
                "command": "mcdev.completion",
                "documentUri": uri,
                "languageId": "java",
                "bufferTextFallbackUsed": false,
                "projectSessionCacheHit": true,
                "projectSessionVersion": 7,
                "documentVersion": 1,
                "documentCacheHit": false,
                "semanticCacheHit": false,
                "parseSource": "HOST_NORMALIZED",
                "parseConfidence": "HIGH",
                "usedCompilationUnit": false,
                "usedJavaProject": true,
                "bindingResolvedCount": 0,
                "bindingFailedCount": 0,
                "semanticContextFound": true,
                "fallbackAnnotationContextUsed": false,
                "semanticTargetCount": 1,
                "semanticMemberCount": 0,
                "completionContextKind": "InjectMethod",
                "owner": "game/Target",
                "methodName": "ti",
                "candidateCacheHit": false,
                "candidateCountBeforeFilter": 1,
                "candidateCountAfterFilter": 1,
                "negativeCacheHit": false,
                "inFlightDedupHit": false,
                "warnings": []
            }
        })
    );
    assert!(completion
        .items
        .iter()
        .any(|item| item.label.contains("tick")));

    let diagnostics = envelope(
        &fixture,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: context(
                &fixture,
                CONFIG_SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: 0,
                },
            ),
        },
    );
    assert!(
        diagnostics.error.is_none(),
        "diagnostic error: {:?}",
        diagnostics.error
    );
    let diagnostics: McdevDiagnosticsResponse =
        serde_json::from_value(diagnostics.result.unwrap()).unwrap();
    let config_uri = format!(
        "{}/src/main/resources/fixture.mixins.json",
        fixture.workspace
    );
    assert_eq!(
        serde_json::to_value(&diagnostics).unwrap(),
        serde_json::json!({
            "diagnostics": [{
                "code": "MIXIN_CLASS_NOT_LISTED_IN_CONFIG",
                "severity": "warning",
                "message": "Mixin class 'Patch' is not listed in mixin config",
                "range": {
                    "start": {"line": 0, "character": 38},
                    "end": {"line": 0, "character": 49}
                },
                "metadata": {
                    "configPath": config_uri,
                    "mixinClass": "Patch"
                }
            }]
        })
    );
    assert!(diagnostics
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "MIXIN_CLASS_NOT_LISTED_IN_CONFIG"));

    let actions = envelope(
        &fixture,
        McdevCommands::CODE_ACTION,
        mcdev_protocol::McdevCodeActionRequest {
            context: context(
                &fixture,
                CONFIG_SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: 0,
                },
            ),
            range: McdevRange::default(),
            diagnostic_codes: vec!["MIXIN_CLASS_NOT_LISTED_IN_CONFIG".into()],
        },
    );
    assert!(
        actions.error.is_none(),
        "code action error: {:?}",
        actions.error
    );
    let actions: McdevCodeActionResponse = serde_json::from_value(actions.result.unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&actions).unwrap(),
        serde_json::json!({
            "actions": [{
                "title": "Add 'Patch' to mixin config",
                "kind": "quickfix.mixin.config",
                "edits": [{
                    "documentUri": config_uri,
                    "edits": [{
                        "range": {
                            "start": {"line": 0, "character": 31},
                            "end": {"line": 0, "character": 31}
                        },
                        "newText": "\"Patch\""
                    }]
                }],
                "metadata": {
                    "configPath": config_uri,
                    "mixinClass": "Patch",
                    "mixinPackage": "fixture"
                }
            }]
        })
    );
    let edit = &actions.actions[0].edits[0];
    assert_eq!(
        apply_protocol_edits(CONFIG_CONTENT, &edit.edits),
        r#"{"package":"fixture","mixins":["Patch"]}"#
    );
    assert!(actions
        .actions
        .iter()
        .any(|action| action.title.contains("Add 'Patch'")));
}

#[test]
fn unavailable_catalog_and_class_bytes_are_typed_failures_but_source_overlay_is_authoritative() {
    let no_target = fixture(false, true, false, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", no_target.workspace);
    let completion = no_target
        .dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(completion_request(context(
                &no_target,
                SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: (SOURCE.find("\"ti\"").unwrap() + 3) as i32,
                },
            )))
            .unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(
        completion.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::BytecodeUnavailable)
    );
    assert!(completion.result.is_none());
    assert!(no_target.query.catalog_reads.load(Ordering::Acquire) > 0);

    let plain = fixture(false, true, false, HostFact::Known(vec![]), false);
    let plain_uri = format!("{}/src/main/java/fixture/Patch.java", plain.workspace);
    let diagnostics = envelope(
        &plain,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: context(
                &plain,
                "class Patch {}",
                &plain_uri,
                McdevPosition {
                    line: 0,
                    character: 0,
                },
            ),
        },
    );
    assert!(diagnostics.error.is_none());
    assert!(diagnostics.result.is_some());
    assert_eq!(plain.query.catalog_reads.load(Ordering::Acquire), 0);
    assert_eq!(plain.query.bytes_reads.load(Ordering::Acquire), 0);

    let target_bytes_missing = fixture(false, false, true, HostFact::Known(vec![]), false);
    let uri = format!(
        "{}/src/main/java/fixture/Patch.java",
        target_bytes_missing.workspace
    );
    assert!(target_bytes_missing
        .session
        .class_index
        .find_class("game/Target")
        .is_some());
    let completion = target_bytes_missing
        .dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(completion_request(context(
                &target_bytes_missing,
                SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: (SOURCE.find("\"ti\"").unwrap() + 3) as i32,
                },
            )))
            .unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(
        completion.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::BytecodeUnavailable)
    );
    assert!(completion.result.is_none());
    assert!(
        target_bytes_missing
            .query
            .bytes_reads
            .load(Ordering::Acquire)
            > 0
    );
    let diagnostics = envelope(
        &target_bytes_missing,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: context(
                &target_bytes_missing,
                SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: 0,
                },
            ),
        },
    );
    assert_eq!(
        diagnostics.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::BytecodeUnavailable)
    );
    assert!(diagnostics.result.is_none());

    let overlay = fixture(true, false, true, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", overlay.workspace);
    let completion = overlay
        .dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(completion_request(context(
                &overlay,
                SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: (SOURCE.find("\"ti\"").unwrap() + 3) as i32,
                },
            )))
            .unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert!(
        completion.error.is_none(),
        "overlay error: {:?}",
        completion.error
    );
    assert!(completion.result.is_some());
    assert_eq!(overlay.query.bytes_reads.load(Ordering::Acquire), 0);
}

#[test]
fn stale_root_unknown_source_and_cancellation_never_become_empty_success() {
    let fixture = fixture(
        true,
        false,
        false,
        HostFact::Unavailable {
            reason: "source inventory unavailable".into(),
        },
        false,
    );
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let project = fixture.host.load_session(&fixture.workspace).unwrap();
    let source_context = context(
        &fixture,
        SOURCE,
        &uri,
        McdevPosition {
            line: 0,
            character: 0,
        },
    );
    assert!(matches!(
        fixture
            .host
            .source_entries(&project, &source_context, SourceFamily::JavaKotlin),
        Err(CommandFailure::Unavailable(error))
            if error.code == McdevErrorCode::IncompleteProjectContext
    ));

    fixture.query.stale.store(true, Ordering::Release);
    let stale = envelope(
        &fixture,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: source_context.clone(),
        },
    );
    assert_eq!(
        stale.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::IncompleteProjectContext)
    );

    fixture.query.stale.store(false, Ordering::Release);
    let mut wrong_root = source_context.clone();
    wrong_root.workspace_root =
        path_to_uri(&std::env::temp_dir().join("mcdev-core-command-different-workspace"));
    let wrong_root = envelope(
        &fixture,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: wrong_root,
        },
    );
    assert_eq!(
        wrong_root.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::IncompleteProjectContext)
    );

    let cancelled = || {
        Err(mcdev_core::expression_engine::Cancellation(Arc::new(
            std::io::Error::other("cancelled"),
        )))
    };
    let result = fixture.dispatcher.execute_project_aware_completion(
        &[serde_json::to_value(completion_request(source_context)).unwrap()],
        &cancelled,
    );
    assert!(matches!(result, Err(CommandFailure::Cancelled(_))));
}

#[test]
fn stale_project_after_semantic_compute_is_a_typed_failure() {
    let fixture = fixture(true, false, false, HostFact::Known(vec![]), true);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let response = envelope(
        &fixture,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: context(
                &fixture,
                SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: 0,
                },
            ),
        },
    );
    assert_eq!(
        response.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::IncompleteProjectContext),
        "a project changed during semantic computation must not become an empty success: {:?}",
        response
    );
    assert!(response.result.is_none());
}

#[test]
fn unavailable_reference_facts_are_strict_on_the_core_facade_path() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let compatibility = fixture.session.reference_types();
    assert_eq!(
        compatibility.is_known_reference("java/lang/Object"),
        None,
        "compatibility Option remains unknown, never false"
    );
    assert_eq!(
        compatibility.is_assignable("game/Target", "java/lang/Object"),
        None,
        "compatibility Option remains unknown, never false"
    );

    let checked = fixture.session.checked_reference_types();
    let reference_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        checked.is_known_reference("java/lang/Object");
    }))
    .expect_err("unavailable reference facts must not become an Option success");
    assert!(matches!(
        reference_panic.downcast_ref::<HostSourceError>(),
        Some(HostSourceError::Failure(message)) if message.contains("reference type facts")
    ));
    let assignability_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        checked.is_assignable("game/Target", "java/lang/Object");
    }))
    .expect_err("unavailable assignability must not become an Option success");
    assert!(matches!(
        assignability_panic.downcast_ref::<HostSourceError>(),
        Some(HostSourceError::Failure(message)) if message.contains("reference assignability")
    ));
}

#[test]
fn actual_facade_preserves_unavailable_bytecode_and_reference_facts() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);

    let expression = fixture
        .dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(completion_request(context(
                &fixture,
                EXPRESSION_SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: (EXPRESSION_SOURCE.rfind("Target.").unwrap() + 7) as i32,
                },
            )))
            .unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(
        expression.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::BytecodeUnavailable),
        "expression bytecode failure must not become empty diagnostics: {:?}",
        expression
    );
    assert!(expression.result.is_none());

    let reference = envelope(
        &fixture,
        McdevCommands::DIAGNOSTICS,
        mcdev_protocol::McdevDiagnosticsRequest {
            context: context(
                &fixture,
                REFERENCE_SOURCE,
                &uri,
                McdevPosition {
                    line: 0,
                    character: 0,
                },
            ),
        },
    );
    assert_eq!(
        reference.error.as_ref().map(|error| error.code),
        Some(McdevErrorCode::IncompleteProjectContext),
        "reference facts must remain typed when reached through the facade: {:?}",
        reference.error
    );
    assert!(reference.result.is_none());
}
