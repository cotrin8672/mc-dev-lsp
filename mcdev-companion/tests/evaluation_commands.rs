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
    )
}

fn evaluation_host_with_provider(
    fixture: &Fixture,
    inventory: HostFact<Vec<SourceScanEntry>>,
    refresh_generation: Option<&str>,
    reference_types: Arc<dyn HandlerReferenceTypeResolver>,
    semantic_provider: Arc<dyn HostSemanticModelProvider>,
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
            context: fixture.session.context.clone(),
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
    };
    CoreCommandHost::with_evaluation(
        inputs,
        semantic_provider,
        Arc::new(Sources {
            config: fixture.session.context.mixin_configs[0].clone(),
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
