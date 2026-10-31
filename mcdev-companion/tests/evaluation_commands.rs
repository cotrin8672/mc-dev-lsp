// Reuse the legacy command fixtures and exact DTO comparisons for this scope.
include!("core_commands.rs");

use mcdev_companion::commands::EvaluationCommandInputs;
use mcdev_core::{
    bytecode::{AtTargetCandidate, BytecodeIndex},
    mixin_navigation::DefinitionBackend,
    project::ProjectContext,
};

struct NoBytecode;
fn bytecode_unavailable() -> ! {
    std::panic::panic_any(CommandFailure::Unavailable(mcdev_protocol::McdevError {
        code: McdevErrorCode::BytecodeUnavailable,
        message: "bytecode unavailable".into(),
        details: Default::default(),
    }))
}
impl BytecodeIndex for NoBytecode {
    fn get_class_bytes(&self, _: &str) -> Option<Vec<u8>> {
        bytecode_unavailable()
    }
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        bytecode_unavailable()
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        bytecode_unavailable()
    }
}

struct ReferenceFacts;
impl HandlerReferenceTypeResolver for ReferenceFacts {
    fn is_known_reference(&self, _: &str) -> Option<bool> {
        Some(true)
    }
    fn is_assignable(&self, from: &str, to: &str) -> Option<bool> {
        Some(from == to)
    }
}

struct ProjectlessSemantic {
    calls: Arc<AtomicUsize>,
    stale_after_parse: Option<Arc<AtomicBool>>,
}
impl HostReadValidator for ProjectlessSemantic {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        panic!("evaluation semantic answers must not reuse a prior read set")
    }
}
impl HostSemanticModelProvider for ProjectlessSemantic {
    fn parse(
        &self,
        request: &HostSemanticRequest<'_>,
    ) -> Result<HostAnswer<HostSemanticModel>, HostSourceError> {
        assert!(
            request.project.is_none(),
            "evaluation must not invent a generation key"
        );
        SemanticFacts {
            stale_after_parse: self.stale_after_parse.clone(),
            parse_calls: self.calls.clone(),
        }
        .parse(request)
    }
}

fn evaluation_host(
    fixture: &Fixture,
    inventory: HostFact<Vec<SourceScanEntry>>,
    refresh_generation: Option<&str>,
    stale_after_parse: bool,
) -> Result<Arc<CoreCommandHost>, CommandFailure> {
    evaluation_host_with_references(
        fixture,
        inventory,
        refresh_generation,
        stale_after_parse,
        Arc::new(ReferenceFacts),
    )
}

#[test]
fn references_coalesce_file_uri_aliases_without_changing_ranges_or_preferred_uris() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let current_uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let other_uri = format!("{}/src/main/java/fixture/Other.java", fixture.workspace);
    let alias = |uri: &str| {
        if let Some(path) = uri.strip_prefix("file:///") {
            if path.as_bytes().get(1) == Some(&b':') {
                return format!("file:///{}%3A{}", &path[..1], &path[2..]);
            }
        }
        uri.replace("/fixture/", "/%66ixture/")
    };
    let other_alias = alias(&other_uri);
    let source = METHOD_REFERENCE_SOURCE;
    let other_source = format!("{source}\n{source}");
    let inventory = HostFact::Known(vec![
        SourceScanEntry {
            document_uri: alias(&current_uri),
            text: format!("// old captured text\n{source}"),
        },
        SourceScanEntry {
            document_uri: other_alias.clone(),
            text: other_source.clone(),
        },
        SourceScanEntry {
            document_uri: other_uri,
            text: other_source,
        },
    ]);
    let host = evaluation_host(&fixture, inventory, None, false).unwrap();
    let dispatcher = CommandDispatcher::for_evaluation(host, &[], true);
    let response = dispatcher.execute(McdevCommands::REFERENCES, &[serde_json::json!({
        "context":context(&fixture,source,&current_uri,position_at(source,source.find("\"tick()V\"").unwrap()+2))
    })]).unwrap();
    assert!(response.error.is_none(), "{:?}", response.error);
    let first_range = McdevRange {
        start: position_at(source, source.find("method =").unwrap()),
        end: position_at(
            source,
            source.find("\"tick()V\"").unwrap() + "\"tick()V\"".len(),
        ),
    };
    let second_range = McdevRange {
        start: McdevPosition {
            line: first_range.start.line + 1,
            character: first_range.start.character,
        },
        end: McdevPosition {
            line: first_range.end.line + 1,
            character: first_range.end.character,
        },
    };
    assert_eq!(
        response.result.unwrap(),
        serde_json::json!({"locations":[
            {"documentUri":current_uri,"range":first_range,"metadata":{"source":"mixin.inject"},"resolution":"SOURCE"},
            {"documentUri":other_alias,"range":first_range,"metadata":{"source":"mixin.inject"},"resolution":"SOURCE"},
            {"documentUri":other_alias,"range":second_range,"metadata":{"source":"mixin.inject"},"resolution":"SOURCE"}
        ]})
    );
}

fn evaluation_host_with_references(
    fixture: &Fixture,
    inventory: HostFact<Vec<SourceScanEntry>>,
    refresh_generation: Option<&str>,
    stale_after_parse: bool,
    reference_types: Arc<dyn HandlerReferenceTypeResolver>,
) -> Result<Arc<CoreCommandHost>, CommandFailure> {
    let provider = Arc::new(ProjectlessSemantic {
        calls: fixture.semantic_calls.clone(),
        stale_after_parse: stale_after_parse.then(|| fixture.query.stale.clone()),
    });
    evaluation_host_with_provider(
        fixture,
        inventory,
        refresh_generation,
        reference_types,
        provider,
        true,
        None,
        None,
        true,
    )
}

fn evaluation_host_with_provider(
    fixture: &Fixture,
    inventory: HostFact<Vec<SourceScanEntry>>,
    refresh_generation: Option<&str>,
    reference_types: Arc<dyn HandlerReferenceTypeResolver>,
    semantic_provider: Arc<dyn HostSemanticModelProvider>,
    config_selection_complete: bool,
    project_context: Option<Arc<ProjectContext>>,
    source_provider: Option<Arc<dyn HostMixinSourceProvider>>,
    mapping_discovery_complete: bool,
) -> Result<Arc<CoreCommandHost>, CommandFailure> {
    let stale = fixture.query.stale.clone();
    let open = fixture.query.open.clone();
    let definitions: Arc<dyn DefinitionBackend> = Arc::new(
        |_: &mcdev_core::McDefinitionTarget,
         _: &ProjectContext,
         _: &str|
         -> Option<ResolvedDefinition> { None },
    );
    let inputs = EvaluationCommandInputs {
        workspace_root_uri: fixture.workspace.clone(),
        cached_session: CachedProjectSession {
            context: project_context.unwrap_or_else(|| fixture.session.context.clone()),
            version: 7,
            cache_hit: false,
            fingerprint: "evaluation-owned".into(),
        },
        class_index: Arc::new(ClassFacts {
            expose_target: true,
        }),
        bytecode_index: Arc::new(NoBytecode),
        reference_types,
        definitions,
        check_active: Arc::new(move || {
            if !open.load(Ordering::Acquire) {
                Err(HostSourceError::Cancelled("evaluation cancelled".into()))
            } else if stale.load(Ordering::Acquire) {
                Err(HostSourceError::Failure("evaluation expired".into()))
            } else {
                Ok(())
            }
        }),
        specifier_supported: Arc::new(|| Ok(true)),
        additional_at_values: Arc::new(|| Ok(vec!["CUSTOM:RETURN".into()])),
        refresh_generation: refresh_generation.map(str::to_owned),
        config_selection_complete,
        mapping_discovery_complete,
    };
    CoreCommandHost::with_evaluation(
        inputs,
        semantic_provider,
        source_provider.unwrap_or_else(|| {
            Arc::new(Sources {
                config: fixture.session.context.mixin_configs[0].clone(),
            })
        }),
        inventory,
        BuildIdentity {
            commit: "fixture-commit".into(),
            build_time: "fixture-time".into(),
            version: "fixture-version".into(),
            artifact_location: "fixture-artifact".into(),
        },
        true,
    )
    .map(Arc::new)
}

struct VersionRecordingSemantic {
    versions: Arc<std::sync::Mutex<Vec<Option<i64>>>>,
}
impl HostReadValidator for VersionRecordingSemantic {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        Ok(false)
    }
}
impl HostSemanticModelProvider for VersionRecordingSemantic {
    fn parse(
        &self,
        request: &HostSemanticRequest<'_>,
    ) -> Result<HostAnswer<HostSemanticModel>, HostSourceError> {
        self.versions.lock().unwrap().push(request.document_version);
        SemanticFacts {
            stale_after_parse: None,
            parse_calls: Arc::new(AtomicUsize::new(0)),
        }
        .parse(request)
    }
}

#[test]
fn evaluation_semantic_version_is_host_version_not_local_snapshot_sequence() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let versions = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider = Arc::new(VersionRecordingSemantic {
        versions: versions.clone(),
    });
    let evaluation = evaluation_host_with_provider(
        &fixture,
        HostFact::Known(vec![]),
        None,
        Arc::new(ReferenceFacts),
        provider.clone(),
        true,
        None,
        None,
        true,
    )
    .unwrap();
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let mut request = context(
        &fixture,
        CONFIG_SOURCE,
        &uri,
        McdevPosition {
            line: 0,
            character: 0,
        },
    );
    request.document_version = None;
    CommandHost::semantic_model(
        evaluation.as_ref(),
        &request,
        9876,
        &fixture.session.context,
        &mcdev_core::expression_engine::NO_CANCELLATION,
    )
    .unwrap();
    request.document_version = Some(41);
    CommandHost::semantic_model(
        evaluation.as_ref(),
        &request,
        9877,
        &fixture.session.context,
        &mcdev_core::expression_engine::NO_CANCELLATION,
    )
    .unwrap();
    assert_eq!(*versions.lock().unwrap(), vec![None, Some(41)]);

    versions.lock().unwrap().clear();
    let legacy = CoreCommandHost::new(
        fixture.session.clone(),
        CachedProjectSession {
            context: fixture.session.context.clone(),
            version: fixture.session.identity.project_session_version,
            cache_hit: false,
            fingerprint: "legacy".into(),
        },
        Arc::new(fixture.query.clone()),
        provider,
        Arc::new(Sources {
            config: fixture.session.context.mixin_configs[0].clone(),
        }),
        HostFact::Known(vec![]),
        BuildIdentity {
            commit: "fixture".into(),
            build_time: "fixture".into(),
            version: "fixture".into(),
            artifact_location: "fixture".into(),
        },
        true,
    )
    .unwrap();
    request.document_version = None;
    CommandHost::semantic_model(
        &legacy,
        &request,
        9878,
        &fixture.session.context,
        &mcdev_core::expression_engine::NO_CANCELLATION,
    )
    .unwrap();
    assert_eq!(*versions.lock().unwrap(), vec![Some(9878)]);
}

#[test]
fn evaluation_commands_preserve_complete_core_outputs_without_generation_keys() {
    let other_uri = "fixture://other/Other.java".to_owned();
    let inventory = HostFact::Known(vec![SourceScanEntry {
        document_uri: other_uri,
        text: METHOD_REFERENCE_SOURCE.into(),
    }]);
    let fixture = fixture(true, false, true, inventory.clone(), false);
    let evaluation = evaluation_host(&fixture, inventory, None, false).unwrap();
    let dispatcher = CommandDispatcher::new(evaluation);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let cursor = SOURCE.find("\"ti\"").unwrap() + 3;
    let request = completion_request(context(
        &fixture,
        SOURCE,
        &uri,
        McdevPosition {
            line: 0,
            character: cursor as i32,
        },
    ));
    let args = [serde_json::to_value(request).unwrap()];
    let expected = fixture
        .dispatcher
        .execute_project_aware_completion(&args, &mcdev_core::expression_engine::NO_CANCELLATION)
        .unwrap();
    let actual = dispatcher
        .execute_project_aware_completion(&args, &mcdev_core::expression_engine::NO_CANCELLATION)
        .unwrap();
    assert!(
        expected.error.is_none(),
        "legacy completion error: {:?}",
        expected.error
    );
    assert!(
        actual.error.is_none(),
        "evaluation completion error: {:?}",
        actual.error
    );
    let mut expected = without_completion_measurements(expected.result.unwrap());
    let actual = without_completion_measurements(actual.result.unwrap());
    assert_eq!(actual["debug"]["projectSessionCacheHit"], false);
    expected["debug"]["projectSessionCacheHit"] = false.into();
    assert_eq!(actual, expected);

    let diagnostics = mcdev_protocol::McdevDiagnosticsRequest {
        context: context(
            &fixture,
            CONFIG_SOURCE,
            &uri,
            McdevPosition {
                line: 0,
                character: 0,
            },
        ),
    };
    let action = McdevCodeActionRequest {
        context: diagnostics.context.clone(),
        range: McdevRange::default(),
        diagnostic_codes: vec!["MIXIN_CLASS_NOT_LISTED_IN_CONFIG".into()],
    };
    let definition = McdevDefinitionRequest {
        context: context(
            &fixture,
            DIRECT_DEFINITION_SOURCE,
            &uri,
            position_at(
                DIRECT_DEFINITION_SOURCE,
                DIRECT_DEFINITION_SOURCE.rfind("drawCall").unwrap() + 2,
            ),
        ),
    };
    let references = McdevReferencesRequest {
        context: context(
            &fixture,
            METHOD_REFERENCE_SOURCE,
            &uri,
            position_at(
                METHOD_REFERENCE_SOURCE,
                METHOD_REFERENCE_SOURCE.find("\"tick()V\"").unwrap() + 2,
            ),
        ),
    };
    for (command, payload) in [
        (
            McdevCommands::DIAGNOSTICS,
            serde_json::to_value(diagnostics).unwrap(),
        ),
        (
            McdevCommands::CODE_ACTION,
            serde_json::to_value(action).unwrap(),
        ),
        (
            McdevCommands::DEFINITION,
            serde_json::to_value(definition).unwrap(),
        ),
        (
            McdevCommands::REFERENCES,
            serde_json::to_value(references).unwrap(),
        ),
    ] {
        let expected = fixture
            .dispatcher
            .execute(command, &[payload.clone()])
            .unwrap();
        let actual = dispatcher.execute(command, &[payload]).unwrap();
        assert!(expected.error.is_none(), "{command}: {:?}", expected.error);
        assert!(actual.error.is_none(), "{command}: {:?}", actual.error);
        assert_eq!(actual.result, expected.result, "{command}");
    }
    assert!(fixture.semantic_calls.load(Ordering::Acquire) >= 2);
}

#[test]
fn incomplete_config_selection_cannot_emit_java_warnings_or_a_successful_empty_result() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let semantic = Arc::new(ProjectlessSemantic {
        calls: fixture.semantic_calls.clone(),
        stale_after_parse: None,
    });
    let host = evaluation_host_with_provider(
        &fixture,
        HostFact::Known(vec![]),
        None,
        Arc::new(ReferenceFacts),
        semantic,
        false,
        None,
        None,
        true,
    )
    .unwrap();
    let dispatcher = CommandDispatcher::for_evaluation(host, &[], false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    for command in [McdevCommands::DIAGNOSTICS, McdevCommands::CONTEXT] {
        let response = dispatcher
            .execute(
                command,
                &[serde_json::json!({"context":context(&fixture, CONFIG_SOURCE, &uri, McdevPosition::default())})],
            )
            .unwrap();
        assert_eq!(
            response.error.unwrap().code,
            McdevErrorCode::IncompleteProjectContext
        );
        assert!(response.result.is_none());
    }
    assert_eq!(fixture.semantic_calls.load(Ordering::Acquire), 0);

    for (source, uri) in [
        (
            "accessWidener v2 named\n",
            format!("{}/src/main/resources/demo.aw", fixture.workspace),
        ),
        (
            "@Mixin(Target::class) class Patch {}",
            format!("{}/src/main/kotlin/fixture/Patch.kt", fixture.workspace),
        ),
    ] {
        let response = dispatcher
            .execute(
                McdevCommands::DIAGNOSTICS,
                &[serde_json::json!({"context":context(&fixture, source, &uri, McdevPosition::default())})],
            )
            .unwrap();
        assert!(response.error.is_none(), "{uri}: {:?}", response.error);
        assert_eq!(
            response.result.unwrap(),
            serde_json::json!({"diagnostics":[]})
        );
    }
    let uri = format!("{}/src/main/resources/demo.aw", fixture.workspace);
    let response = dispatcher.execute(McdevCommands::CODE_ACTION, &[serde_json::json!({
        "context":context(&fixture, "accessWidener v2 named\n", &uri, McdevPosition::default()),
        "range":McdevRange::default(), "diagnosticCodes":[]
    })]).unwrap();
    assert!(
        response.error.is_none(),
        "AW action unexpectedly depended on Mixin config selection: {:?}",
        response.error
    );
    assert_eq!(response.result.unwrap(), serde_json::json!({"actions":[]}));
}

#[test]
fn evaluation_owning_subproject_root_keeps_the_original_workspace_identity() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let mut project = fixture.session.context.as_ref().clone();
    project.root = project.root.join("subproject");
    project.project_id = "gradle::|:subproject".into();
    let semantic = Arc::new(ProjectlessSemantic {
        calls: fixture.semantic_calls.clone(),
        stale_after_parse: None,
    });
    let host = evaluation_host_with_provider(
        &fixture,
        HostFact::Known(vec![]),
        None,
        Arc::new(ReferenceFacts),
        semantic,
        false,
        Some(Arc::new(project.clone())),
        None,
        true,
    )
    .unwrap();
    let loaded = host.load_session(&fixture.workspace).unwrap();
    assert_eq!(loaded.root, project.root);
    assert_eq!(loaded.project_id, project.project_id);
    assert!(matches!(host.load_session(&path_to_uri(&project.root)),
        Err(CommandFailure::Unavailable(error)) if error.code == McdevErrorCode::IncompleteProjectContext));
    let uri = path_to_uri(&project.root.join("src/main/java/fixture/Patch.java"));
    let request = completion_request(context(
        &fixture,
        SOURCE,
        &uri,
        position_at(SOURCE, SOURCE.find("\"ti\"").unwrap() + 3),
    ));
    let response = CommandDispatcher::new(host)
        .execute_project_aware_completion(
            &[serde_json::to_value(request).unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert!(
        response.error.is_none(),
        "owning-project completion rejected its root: {:?}",
        response.error
    );
    assert!(response.result.unwrap()["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["label"]
            .as_str()
            .is_some_and(|label| label.contains("tick"))));
}

struct ForbiddenConfigSources;
impl HostMixinSourceProvider for ForbiddenConfigSources {
    fn config(&self, _: &Path) -> Result<HostAnswer<MixinConfigRef>, HostSourceError> {
        panic!("config-independent action read a Mixin config")
    }
    fn compilation_unit_source(
        &self,
        _: &str,
        _: Option<&Path>,
    ) -> Result<HostAnswer<RegisteredMixinSource>, HostSourceError> {
        panic!("config-independent action selected registered mixins")
    }
    fn read_source_file(&self, _: &Path) -> Result<HostAnswer<String>, HostSourceError> {
        panic!("config-independent action read config content")
    }
}

fn config_unknown_action_host(fixture: &Fixture) -> Arc<CoreCommandHost> {
    evaluation_host_with_provider(
        fixture,
        HostFact::Known(vec![]),
        None,
        Arc::new(ReferenceFacts),
        Arc::new(ProjectlessSemantic {
            calls: fixture.semantic_calls.clone(),
            stale_after_parse: None,
        }),
        false,
        None,
        Some(Arc::new(ForbiddenConfigSources)),
        false,
    )
    .unwrap()
}

#[test]
fn explicit_config_independent_actions_preserve_exact_fixes_and_handler_generation() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let prefix = "import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;\n@Mixin(Target.class) class Patch { ";
    let known = CommandDispatcher::new(
        evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap(),
    );
    for (tail, codes) in [
        (
            "@WrapMethod(method = \"tick\") int handler() { return 1; } }",
            vec![
                "MIXINEXTRAS_WRONG_RETURN_TYPE",
                "MIXINEXTRAS_MISSING_OPERATION_PARAMETER",
            ],
        ),
        (
            "@WrapMethod(method = \"tick\")\n}",
            vec!["MIXIN_DUPLICATE_TARGET"],
        ),
    ] {
        let source = format!("{prefix}{tail}");
        let position = position_at(&source, source.find("@WrapMethod").unwrap() + 5);
        let request = McdevCodeActionRequest {
            context: context(&fixture, &source, &uri, position.clone()),
            range: McdevRange {
                start: position.clone(),
                end: position,
            },
            diagnostic_codes: codes.into_iter().map(str::to_owned).collect(),
        };
        let args = [serde_json::to_value(request).unwrap()];
        let expected = known.execute(McdevCommands::CODE_ACTION, &args).unwrap();
        assert!(
            expected.error.is_none(),
            "known-config action failed: {:?}",
            expected.error
        );
        let actions = expected.result.as_ref().unwrap()["actions"]
            .as_array()
            .unwrap();
        assert!(
            !actions.is_empty(),
            "fixture must exercise actual fixes or unconditional handler generation: {tail}"
        );
        assert!(actions.iter().any(|action| action["edits"]
            .as_array()
            .is_some_and(|edits| !edits.is_empty())));
        for project_complete in [false, true] {
            let dispatcher = CommandDispatcher::for_evaluation(
                config_unknown_action_host(&fixture),
                &[],
                project_complete,
            );
            let actual = dispatcher
                .execute(McdevCommands::CODE_ACTION, &args)
                .unwrap();
            assert!(
                actual.error.is_none(),
                "config-independent action failed: {:?}",
                actual.error
            );
            assert_eq!(actual.result, expected.result);
        }
    }
}

#[test]
fn unknown_config_actions_require_a_nonempty_entirely_classified_independent_code_list() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let source = "import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;\n@Mixin(Target.class) class Patch { @WrapMethod(method = \"tick\")\n}";
    let position = position_at(source, source.find("@WrapMethod").unwrap() + 5);
    for project_complete in [false, true] {
        let dispatcher = CommandDispatcher::for_evaluation(
            config_unknown_action_host(&fixture),
            &[],
            project_complete,
        );
        for codes in [
            vec![],
            vec!["MIXIN_CLASS_NOT_LISTED_IN_CONFIG"],
            vec!["MIXIN_DUPLICATE_CONFIG_ENTRY"],
            vec!["MIXIN_FUTURE_CODE"],
            vec!["MIXINEXTRAS_FUTURE_CODE"],
            vec!["MIXIN_DUPLICATE_TARGET", "MIXIN_CLASS_NOT_LISTED_IN_CONFIG"],
            vec!["MIXINEXTRAS_WRONG_RETURN_TYPE", "MIXINEXTRAS_FUTURE_CODE"],
        ] {
            let request = McdevCodeActionRequest {
                context: context(&fixture, source, &uri, position.clone()),
                range: McdevRange {
                    start: position.clone(),
                    end: position.clone(),
                },
                diagnostic_codes: codes.into_iter().map(str::to_owned).collect(),
            };
            let response = dispatcher
                .execute(
                    McdevCommands::CODE_ACTION,
                    &[serde_json::to_value(request).unwrap()],
                )
                .unwrap();
            assert_eq!(
                response.error.unwrap().code,
                McdevErrorCode::IncompleteProjectContext
            );
            assert!(response.result.is_none());
        }
        let response = dispatcher
            .execute(
                McdevCommands::DIAGNOSTICS,
                &[serde_json::json!({
                    "context":context(&fixture, source, &uri, position.clone())
                })],
            )
            .unwrap();
        assert_eq!(
            response.error.unwrap().code,
            McdevErrorCode::IncompleteProjectContext
        );
        assert!(
            response.result.is_none(),
            "full diagnostics cannot return a partial success"
        );
    }
}

#[test]
fn unknown_mapping_discovery_preserves_proven_names_but_not_remap_or_default_namespace_answers() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let dispatcher =
        CommandDispatcher::for_evaluation(config_unknown_action_host(&fixture), &[], false);
    for (text, extension, expected_error) in [
        ("accessWidener v2 named\n", "aw", false),
        (
            "accessWidener v2 named\naccessible class game/Target\n",
            "aw",
            false,
        ),
        (
            "accessWidener v2 named\naccessible method game/Target tick ()V\n",
            "aw",
            false,
        ),
        ("accessWidener v2 named\naccessible class x/T\n", "aw", true),
        ("public game.Target\n", "at", false),
        ("public game.Target tick()V\n", "at", true),
    ] {
        let uri = format!("{}/src/main/resources/test.{extension}", fixture.workspace);
        let response = dispatcher
            .execute(
                McdevCommands::DIAGNOSTICS,
                &[serde_json::json!({
                    "context":context(&fixture,text,&uri,McdevPosition::default())
                })],
            )
            .unwrap();
        if expected_error {
            assert_eq!(
                response.error.unwrap().code,
                McdevErrorCode::IncompleteProjectContext,
                "{text}"
            );
            assert!(response.result.is_none());
        } else {
            assert!(response.error.is_none(), "{text}: {:?}", response.error);
            assert_eq!(
                response.result.unwrap(),
                serde_json::json!({"diagnostics":[]})
            );
        }
    }
    for (text, extension, expected_error, expected_label) in [
        ("accessWidener v2 i", "aw", false, "intermediary"),
        (
            "accessWidener v2 named\naccessible class Tar",
            "aw",
            false,
            "Target",
        ),
        (
            "accessWidener v2 intermediary\naccessible class Tar",
            "aw",
            true,
            "",
        ),
        ("public game.T", "at", false, "Target"),
        ("public game.Target ti", "at", true, ""),
    ] {
        let uri = format!("{}/src/main/resources/test.{extension}", fixture.workspace);
        let request =
            completion_request(context(&fixture, text, &uri, position_at(text, text.len())));
        let response = dispatcher
            .execute(
                McdevCommands::COMPLETION,
                &[serde_json::to_value(request).unwrap()],
            )
            .unwrap();
        if expected_error {
            assert_eq!(
                response.error.unwrap().code,
                McdevErrorCode::IncompleteProjectContext,
                "{text}"
            );
            assert!(response.result.is_none());
        } else {
            assert!(response.error.is_none(), "{text}: {:?}", response.error);
            assert!(
                response.result.unwrap()["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["label"]
                        .as_str()
                        .is_some_and(|label| label.contains(expected_label))),
                "{text}"
            );
        }
    }
}

#[test]
fn independent_root_domains_do_not_require_a_complete_context_dto() {
    let inventory = HostFact::Known(vec![SourceScanEntry {
        document_uri: "fixture://other/Other.java".into(),
        text: METHOD_REFERENCE_SOURCE.into(),
    }]);
    let fixture = fixture(true, false, true, inventory.clone(), false);
    let host = evaluation_host_with_provider(
        &fixture,
        inventory,
        None,
        Arc::new(ReferenceFacts),
        Arc::new(ProjectlessSemantic {
            calls: fixture.semantic_calls.clone(),
            stale_after_parse: None,
        }),
        true,
        None,
        None,
        false,
    )
    .unwrap();
    let dispatcher =
        CommandDispatcher::for_evaluation(host, &[], false).with_discovery_coverage(true, true);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let action = McdevCodeActionRequest {
        context: context(&fixture, CONFIG_SOURCE, &uri, McdevPosition::default()),
        range: McdevRange::default(),
        diagnostic_codes: vec!["MIXIN_CLASS_NOT_LISTED_IN_CONFIG".into()],
    };
    let args = [serde_json::to_value(action).unwrap()];
    let expected = fixture
        .dispatcher
        .execute(McdevCommands::CODE_ACTION, &args)
        .unwrap();
    let actual = dispatcher
        .execute(McdevCommands::CODE_ACTION, &args)
        .unwrap();
    assert!(actual.error.is_none(), "{:?}", actual.error);
    assert_eq!(actual.result, expected.result);
    assert!(!actual.result.unwrap()["actions"]
        .as_array()
        .unwrap()
        .is_empty());
    let refs = McdevReferencesRequest {
        context: context(
            &fixture,
            METHOD_REFERENCE_SOURCE,
            &uri,
            position_at(
                METHOD_REFERENCE_SOURCE,
                METHOD_REFERENCE_SOURCE.find("\"tick()V\"").unwrap() + 2,
            ),
        ),
    };
    let args = [serde_json::to_value(refs).unwrap()];
    let expected = fixture
        .dispatcher
        .execute(McdevCommands::REFERENCES, &args)
        .unwrap();
    let actual = dispatcher
        .execute(McdevCommands::REFERENCES, &args)
        .unwrap();
    assert!(actual.error.is_none(), "{:?}", actual.error);
    assert_eq!(actual.result, expected.result);
    assert!(!actual.result.unwrap()["locations"]
        .as_array()
        .unwrap()
        .is_empty());
    let info=dispatcher.execute(McdevCommands::INFO,&[serde_json::json!({"context":context(&fixture,CONFIG_SOURCE,&uri,McdevPosition::default())})]).unwrap();
    assert_eq!(
        info.error.unwrap().code,
        McdevErrorCode::IncompleteProjectContext
    );
    assert!(info.result.is_none());
}

#[test]
fn unknown_config_roster_cannot_be_reported_as_successful_empty_share_completion() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let dispatcher =
        CommandDispatcher::for_evaluation(config_unknown_action_host(&fixture), &[], false);
    let source="@Mixin(Target.class) class Patch { @WrapMethod(method = \"tick\") void handler(@Share(\"te\") Object value) {} }";
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let request = completion_request(context(
        &fixture,
        source,
        &uri,
        position_at(source, source.find("\"te\"").unwrap() + 3),
    ));
    let response = dispatcher
        .execute(
            McdevCommands::COMPLETION,
            &[serde_json::to_value(request).unwrap()],
        )
        .unwrap();
    let error = response.error.unwrap();
    assert_eq!(error.code, McdevErrorCode::IncompleteProjectContext);
    assert!(error.message.contains("Mixin config"));
    assert!(response.result.is_none());
}

#[test]
fn evaluation_refresh_requires_host_issued_generation_and_live_scope() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let ordinary = evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap();
    assert!(matches!(
        ordinary.reindex(&fixture.workspace),
        Err(CommandFailure::Unavailable(_))
    ));
    assert!(matches!(
        ordinary.clear_project_cache(),
        Err(CommandFailure::Unavailable(_))
    ));
    assert!(matches!(
        evaluation_host(
            &fixture,
            HostFact::Known(vec![]),
            Some("not-decimal"),
            false
        ),
        Err(CommandFailure::Unavailable(_))
    ));
    assert!(matches!(
        evaluation_host(
            &fixture,
            HostFact::Known(vec![]),
            Some("18446744073709551616"),
            false
        ),
        Err(CommandFailure::Unavailable(_))
    ));
    let refreshed = evaluation_host(&fixture, HostFact::Known(vec![]), Some("129"), false).unwrap();
    assert!(refreshed.clear_project_cache().is_ok());
    assert!(refreshed.reindex(&fixture.workspace).is_ok());
    fixture.query.stale.store(true, Ordering::Release);
    assert!(matches!(
        refreshed.reindex(&fixture.workspace),
        Err(CommandFailure::Unavailable(_))
    ));
}

#[test]
fn evaluation_rejects_expiry_after_semantic_compute() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host(&fixture, HostFact::Known(vec![]), None, true).unwrap();
    let dispatcher = CommandDispatcher::new(evaluation);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let request = completion_request(context(
        &fixture,
        SOURCE,
        &uri,
        position_at(SOURCE, SOURCE.find("\"ti\"").unwrap() + 3),
    ));
    let result = dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(request).unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(
        result.error.unwrap().code,
        McdevErrorCode::IncompleteProjectContext
    );
}

#[test]
fn evaluation_keeps_unavailable_inventory_and_cancelled_validation() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host(
        &fixture,
        HostFact::Unavailable {
            reason: "inventory incomplete".into(),
        },
        None,
        false,
    )
    .unwrap();
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let request = context(
        &fixture,
        SOURCE,
        &uri,
        McdevPosition {
            line: 0,
            character: 0,
        },
    );
    assert!(matches!(
        evaluation.source_entries(
            fixture.session.context.as_ref(),
            &request,
            SourceFamily::JavaKotlin
        ),
        Err(CommandFailure::Unavailable(_))
    ));
    fixture.query.stale.store(true, Ordering::Release);
    assert!(matches!(
        evaluation.load_session(&fixture.workspace),
        Err(CommandFailure::Unavailable(_))
    ));
    fixture.query.stale.store(false, Ordering::Release);
    fixture.query.open.store(false, Ordering::Release);
    assert!(matches!(
        evaluation.load_session(&fixture.workspace),
        Err(CommandFailure::Cancelled(_))
    ));
}

#[test]
fn evaluation_buffer_completion_uses_scoped_at_value_callbacks() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap();
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    for (partial, expected) in [("CUSTOM:R", "CUSTOM:RETURN"), ("INVOKE:F", "INVOKE:FIRST")] {
        let source = format!(
            "@Mixin(Target.class) class Patch {{ @Inject(method = \"tick\", at = @At(\"{partial}\")) void handler() {{}} }}"
        );
        let offset = source.find(partial).unwrap() + partial.len();
        let request = context(&fixture, &source, &uri, position_at(&source, offset));
        let result = evaluation
            .buffer_only_completion(
                &request,
                &mcdev_core::mixin_completion::MixinCompletionOptions::default(),
                &mcdev_core::expression_engine::NO_CANCELLATION,
            )
            .unwrap()
            .expect("At value completion is buffer-only");
        assert!(
            result.items.iter().any(|item| item.label == expected),
            "{partial}: {:?}",
            result.items
        );
    }
}

struct UnknownReferenceFacts;
impl HandlerReferenceTypeResolver for UnknownReferenceFacts {
    fn is_known_reference(&self, _: &str) -> Option<bool> {
        None
    }
    fn is_assignable(&self, _: &str, _: &str) -> Option<bool> {
        None
    }
}

#[test]
fn evaluation_accepts_host_native_workspace_path_for_project_completion() {
    let mut fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    // The host sends file_key(root), while the facade asks using a file URI.
    fixture.workspace = fixture.session.context.root.to_string_lossy().into_owned();
    let uri = path_to_uri(
        &fixture
            .session
            .context
            .root
            .join("src/main/java/fixture/Patch.java"),
    );
    let evaluation = evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap();
    let dispatcher = CommandDispatcher::new(evaluation);
    let request = completion_request(context(
        &fixture,
        SOURCE,
        &uri,
        position_at(SOURCE, SOURCE.find("\"ti\"").unwrap() + 3),
    ));
    let response = dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(request).unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert!(
        response.error.is_none(),
        "native workspace path rejected: {:?}",
        response.error
    );
    let response: McdevCompletionResponse =
        serde_json::from_value(response.result.unwrap()).unwrap();
    assert!(response
        .items
        .iter()
        .any(|item| item.label.contains("tick")));
}

#[test]
fn evaluation_facade_preserves_unknown_references_and_bytecode_error_codes() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host_with_references(
        &fixture,
        HostFact::Known(vec![]),
        None,
        false,
        Arc::new(UnknownReferenceFacts),
    )
    .unwrap();
    let dispatcher = CommandDispatcher::new(evaluation);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let expression = dispatcher
        .execute_project_aware_completion(
            &[serde_json::to_value(completion_request(context(
                &fixture,
                EXPRESSION_SOURCE,
                &uri,
                position_at(
                    EXPRESSION_SOURCE,
                    EXPRESSION_SOURCE.rfind("Target.").unwrap() + 7,
                ),
            )))
            .unwrap()],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(
        expression.error.unwrap().code,
        McdevErrorCode::BytecodeUnavailable
    );
    assert!(expression.result.is_none());
    let diagnostics = dispatcher
        .execute(
            McdevCommands::DIAGNOSTICS,
            &[
                serde_json::to_value(mcdev_protocol::McdevDiagnosticsRequest {
                    context: context(
                        &fixture,
                        REFERENCE_SOURCE,
                        &uri,
                        McdevPosition {
                            line: 0,
                            character: 0,
                        },
                    ),
                })
                .unwrap(),
            ],
        )
        .unwrap();
    assert_eq!(
        diagnostics.error.unwrap().code,
        McdevErrorCode::IncompleteProjectContext
    );
    assert!(diagnostics.result.is_none());
}

#[test]
fn incomplete_evaluation_returns_empty_references_only_after_resolving_targets() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap();
    let dispatcher = CommandDispatcher::for_evaluation(evaluation, &[], false);
    let uri = format!("{}/src/main/resources/access.aw", fixture.workspace);
    let response = dispatcher
        .execute(
            McdevCommands::REFERENCES,
            &[serde_json::to_value(McdevReferencesRequest {
                context: context(
                    &fixture,
                    "accessWidener v2 named\n",
                    &uri,
                    McdevPosition::default(),
                ),
            })
            .unwrap()],
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        serde_json::json!({
            "protocolVersion":1,"serverVersion":"0.1.0","capabilities":["references"],
            "result":{"locations":[]}
        })
    );

    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let source = METHOD_REFERENCE_SOURCE;
    let response = dispatcher
        .execute(
            McdevCommands::REFERENCES,
            &[serde_json::to_value(McdevReferencesRequest {
                context: context(
                    &fixture,
                    source,
                    &uri,
                    position_at(source, source.find("\"tick()V\"").unwrap() + 2),
                ),
            })
            .unwrap()],
        )
        .unwrap();
    let error = response.error.unwrap();
    assert_eq!(error.code, McdevErrorCode::IncompleteProjectContext);
    assert_eq!(
        error.message,
        "complete project source-set and resource inventory is unavailable"
    );
    assert!(response.result.is_none());
    assert!(fixture.semantic_calls.load(Ordering::Acquire) > 0);
}

#[test]
fn source_reference_absence_needs_normalized_facts_regardless_of_project_coverage() {
    struct FallbackSemantic;
    impl HostReadValidator for FallbackSemantic {
        fn validate_current_reads(
            &self,
            _: HostGenerations,
            _: &HostReadSet,
        ) -> Result<bool, HostSourceError> {
            panic!("evaluation source facts cannot reuse a prior read set")
        }
    }
    impl HostSemanticModelProvider for FallbackSemantic {
        fn parse(
            &self,
            request: &HostSemanticRequest<'_>,
        ) -> Result<HostAnswer<HostSemanticModel>, HostSourceError> {
            let model = HostSemanticModel::fallback(
                request.source,
                request.source_uri,
                "host declaration coverage and ranges are unavailable",
                true,
                0,
            );
            assert!(
                !model.model.targets.is_empty(),
                "fallback contains a known Mixin target"
            );
            Ok(HostAnswer::known(model, vec![]))
        }
    }

    for project_complete in [false, true] {
        for (language, extension, source) in [
            (
                "java",
                "java",
                "import org.spongepowered.asm.mixin.Mixin;\n@Mixin(Target.class)\nclass Patch {}",
            ),
            (
                "kotlin",
                "kt",
                "import org.spongepowered.asm.mixin.Mixin\n@Mixin(Target::class)\nclass Patch {}",
            ),
        ] {
            let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
            let uri = format!("{}/src/main/fixture/Patch.{extension}", fixture.workspace);
            let mut context = context(&fixture, source, &uri, position_at(source, source.len()));
            context.language_id = language.into();
            for fallback in [false, true] {
                let host = if fallback {
                    evaluation_host_with_provider(
                        &fixture,
                        HostFact::Known(vec![]),
                        None,
                        Arc::new(ReferenceFacts),
                        Arc::new(FallbackSemantic),
                        true,
                        None,
                        None,
                        true,
                    )
                } else {
                    evaluation_host(&fixture, HostFact::Known(vec![]), None, false)
                }
                .unwrap();
                let response = CommandDispatcher::for_evaluation(host, &[], project_complete)
                    .execute(
                        McdevCommands::REFERENCES,
                        &[serde_json::json!({"context":context})],
                    )
                    .unwrap();
                if fallback {
                    let error = response.error.unwrap();
                    assert_eq!(error.code, McdevErrorCode::IncompleteProjectContext);
                    assert_eq!(
                        error.message,
                        "host source declaration coverage and ranges are unavailable"
                    );
                    assert!(response.result.is_none());
                } else {
                    assert!(response.error.is_none());
                    assert_eq!(
                        response.result.unwrap(),
                        serde_json::json!({"locations":[]})
                    );
                }
            }
        }
    }
}

#[test]
fn incomplete_evaluation_keeps_kotlin_actions_empty_and_project_dtos_unavailable() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap();
    let dispatcher = CommandDispatcher::for_evaluation(evaluation, &[], false);
    let uri = format!("{}/src/main/java/fixture/Patch.kt", fixture.workspace);
    let mut request_context = context(&fixture, "class Patch", &uri, McdevPosition::default());
    request_context.language_id = "kotlin".into();
    let response = dispatcher
        .execute(
            McdevCommands::CODE_ACTION,
            &[serde_json::to_value(McdevCodeActionRequest {
                context: request_context.clone(),
                range: McdevRange::default(),
                diagnostic_codes: vec![],
            })
            .unwrap()],
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(response).unwrap(),
        serde_json::json!({
            "protocolVersion":1,"serverVersion":"0.1.0","capabilities":["codeAction"],
            "result":{"actions":[]}
        })
    );
    assert_eq!(fixture.semantic_calls.load(Ordering::Acquire), 0);

    request_context.document_uri =
        format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    request_context.language_id = "java".into();
    for command in [
        McdevCommands::CODE_ACTION,
        McdevCommands::INFO,
        McdevCommands::DUMP_CONTEXT,
    ] {
        let response = dispatcher
            .execute(command, &[serde_json::json!({"context":request_context})])
            .unwrap();
        assert_eq!(
            response.error.unwrap().code,
            McdevErrorCode::IncompleteProjectContext,
            "{command}"
        );
        assert!(response.result.is_none());
    }
}

#[test]
fn acknowledged_info_reports_only_negotiated_commands() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let evaluation = evaluation_host(&fixture, HostFact::Known(vec![]), None, false).unwrap();
    let commands = [
        "mcdev.references".into(),
        "mcdev.unimplemented".into(),
        "mcdev.info".into(),
        "mcdev.info".into(),
    ];
    let dispatcher = CommandDispatcher::for_evaluation(evaluation, &commands, true);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let response = dispatcher
        .execute(
            McdevCommands::INFO,
            &[serde_json::json!({
                "context": context(&fixture, CONFIG_SOURCE, &uri, McdevPosition::default())
            })],
        )
        .unwrap();
    assert!(response.error.is_none());
    let result = response.result.unwrap();
    assert_eq!(
        result["registeredCommands"],
        serde_json::json!(["mcdev.info", "mcdev.references"])
    );
    let lines = result["lines"].as_array().unwrap();
    assert_eq!(
        &lines[lines.len() - 3..],
        &[
            serde_json::json!("Registered commands:"),
            serde_json::json!("  - mcdev.info"),
            serde_json::json!("  - mcdev.references"),
        ]
    );
}

#[test]
fn refreshed_generation_does_not_certify_a_complete_project_summary() {
    let fixture = fixture(true, false, true, HostFact::Known(vec![]), false);
    let uri = format!("{}/src/main/java/fixture/Patch.java", fixture.workspace);
    let payload = serde_json::json!({"context":context(&fixture, CONFIG_SOURCE, &uri, McdevPosition::default())});
    for generation in [None, Some("129")] {
        let evaluation =
            evaluation_host(&fixture, HostFact::Known(vec![]), generation, false).unwrap();
        let dispatcher = CommandDispatcher::for_evaluation(evaluation, &[], false);
        for command in [
            McdevCommands::REINDEX,
            McdevCommands::RELOAD_PROJECT_CONTEXT,
        ] {
            let response = dispatcher.execute(command, &[payload.clone()]).unwrap();
            let error = response.error.unwrap();
            assert_eq!(error.code, McdevErrorCode::IncompleteProjectContext);
            assert_eq!(
                error.message,
                if generation.is_some() {
                    "complete project source-set and resource inventory is unavailable"
                } else {
                    "host project refresh authority is unavailable"
                }
            );
            assert!(response.result.is_none());
        }
    }
    let evaluation =
        evaluation_host(&fixture, HostFact::Known(vec![]), Some("130"), false).unwrap();
    let dispatcher = CommandDispatcher::for_evaluation(evaluation, &[], false);
    fixture.query.open.store(false, Ordering::Release);
    for command in [
        McdevCommands::REINDEX,
        McdevCommands::RELOAD_PROJECT_CONTEXT,
    ] {
        assert!(matches!(
            dispatcher.execute(command, &[payload.clone()]),
            Err(CommandFailure::Cancelled(_))
        ));
    }
}
