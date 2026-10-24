#[allow(dead_code)]
#[path = "fixtures/commands/host.rs"]
mod command_host;
mod two_ops_process;
mod two_ops_support;
use mcdev_companion::{
    host_facts::{bridge::*, draft::*, *},
    wire::{HandleKind, Registry},
};
use mcdev_core::host_source::HostFact;
use mcdev_protocol::{McdevCommands, McdevErrorCode};
use serde_json::json;
use two_ops_support::*;

#[test]
fn optional_operations_are_a_per_instance_intersection_with_legacy_fallback() {
    for host in [
        json!({"capabilities":["semantic-v1"]}),
        json!({"capabilities":["semantic-v1",CAPABILITY]}),
        json!({"capabilities":["semantic-v1"],"supportedOps":{CAPABILITY:["projectContext","typeJvm"]}}),
    ] {
        assert!(SupportedOps::intersect(&host).unwrap().is_empty());
    }
    let ops=SupportedOps::intersect(&json!({"capabilities":["semantic-v1",CAPABILITY],"supportedOps":{CAPABILITY:["typeJvm","projectContext","classFile","typeJvm"]}})).unwrap();
    assert_eq!(ops.accepted(), vec!["projectContext", "typeJvm"]);
    assert!(SupportedOps::intersect(
        &json!({"capabilities":[CAPABILITY],"supportedOps":{CAPABILITY:[12]}})
    )
    .is_err());
    let mut reader = Reader::new();
    let mut registry = new_registry();
    let none = SupportedOps::default();
    let mut scoped = ScopedFacts::new(EVALUATION, &none, &mut registry, &mut reader).unwrap();
    assert!(matches!(
        scoped.project_context(),
        Err(FactsError::Unsupported(_))
    ));
    assert!(reader.requests.is_empty());
}

#[test]
fn project_metadata_keeps_independent_knowledge_and_classpath_order() {
    let mut reader = Reader::new();
    reader.project["scope"] = unknown("MissingScope");
    reader.project["classpath"]["value"]["coverage"] = unknown("PartialClasspath");
    let mut registry = new_registry();
    let ops = ops();
    let mut scoped = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader).unwrap();
    let project = scoped.project_context().unwrap();
    assert!(project.workspace_uri.found().is_some());
    assert!(project.scope.found().is_none());
    let cp = project.classpath.found().unwrap();
    assert_eq!(cp.entries[0].display_path, "/display/b.jar");
    assert_eq!(cp.entries[1].display_path, "/display/a.jar");
    assert!(cp.coverage.found().is_none());
    assert!(matches!(project.sdk.found().unwrap().api, SdkApi::Full));
    assert_eq!(
        reader.requests[0],
        json!({"evaluation":EVALUATION,"queries":[{"op":"projectContext"}]})
    );
}

#[test]
fn sdk_full_release_and_unknown_are_distinct() {
    let mut project: ProjectContext = serde_json::from_value(project()).unwrap();
    assert!(matches!(project.sdk.found().unwrap().api, SdkApi::Full));
    let mut json = serde_json::to_value(&project).unwrap();
    json["sdk"]["value"]["api"] = json!({"kind":"release","release":17});
    project = serde_json::from_value(json).unwrap();
    assert!(matches!(
        project.sdk.found().unwrap().api,
        SdkApi::Release { release: 17 }
    ));
    let mut json = serde_json::to_value(&project).unwrap();
    json["sdk"] = unknown("MissingSdk");
    project = serde_json::from_value(json).unwrap();
    assert!(project.sdk.found().is_none());
}

#[test]
fn scoped_type_requires_a_host_issued_current_type_handle_before_io() {
    let mut reader = Reader::new();
    let mut registry = Registry::default();
    let ops = ops();
    let mut scoped = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader).unwrap();
    assert!(scoped.type_jvm(TYPE).is_err());
    assert!(reader.requests.is_empty());
    let mut registry = new_registry();
    let mut scoped = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader).unwrap();
    scoped.type_jvm(TYPE).unwrap();
    assert_eq!(
        reader.requests[0],
        json!({"evaluation":EVALUATION,"queries":[{"op":"typeJvm","target":TYPE}]})
    );
}

#[test]
fn complete_editing_coverage_and_known_instance_do_not_establish_jvm_abi() {
    let mut reader = Reader::new();
    reader.ty["abi"] = unknown("SourceOnly");
    reader.ty["jvmAbiCoverage"] = unknown("SourceOnly");
    reader.ty["classFlags"] = unknown("MissingFacts");
    let mut registry = new_registry();
    let ops = ops();
    let mut scoped = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader).unwrap();
    let facts = scoped.type_jvm(TYPE).unwrap();
    assert!(facts.declared_members.coverage.found().is_some());
    assert_eq!(
        facts.declared_members.members[0]
            .member
            .dispatch
            .found()
            .map(String::as_str),
        Some("instance")
    );
    assert!(matches!(abi_members(&facts), Err(FactsError::Unknown(_))));
    assert!(registry
        .require("editing-method", HandleKind::Member)
        .is_ok());
    assert!(registry
        .require("editing-signature", HandleKind::Signature)
        .is_ok());
}

#[test]
fn complete_raw_abi_retains_generated_overloads_initializers_and_exact_flags() {
    let mut reader = Reader::new();
    let original = reader.ty.clone();
    let mut registry = new_registry();
    let ops = ops();
    let facts = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader)
        .unwrap()
        .type_jvm(TYPE)
        .unwrap();
    assert_eq!(serde_json::to_value(&facts).unwrap(), original);
    let members = abi_members(&facts).unwrap();
    assert_eq!(members.methods.len(), 6);
    assert_eq!(members.fields.len(), 2);
    assert!(members
        .methods
        .iter()
        .any(|m| m.name == "<clinit>" && m.descriptor == "()V" && m.is_static));
    assert!(members
        .methods
        .iter()
        .any(|m| m.name == "<init>" && !m.is_static));
    let raw = facts.abi.found().unwrap();
    let bridge = raw
        .methods
        .iter()
        .find(|m| m.name == "deserialize" && m.descriptor.ends_with("Ljava/lang/Object;"))
        .unwrap();
    let exact = raw
        .methods
        .iter()
        .find(|m| m.name == "deserialize" && m.descriptor.ends_with("Lfixture/Model;"))
        .unwrap();
    assert_eq!(bridge.access_flags, 0x1041);
    assert_eq!(exact.access_flags, 1);
    assert_eq!(raw.fields[1].access_flags, 0);
    assert_eq!(raw.fields[1].attribute_names, vec!["Synthetic"]);
    assert_eq!(facts.declared_members.members.len(), 1);
    assert!(raw.methods.iter().any(|m| m.name == "getName"));
    assert!(facts
        .declared_members
        .members
        .iter()
        .all(|m| m.member.name != "getName"));
    for raw in serde_json::to_value(raw).unwrap()["methods"]
        .as_array()
        .unwrap()
    {
        assert!(raw.get("id").is_none());
        assert!(raw.get("declaration").is_none());
        assert!(raw.get("range").is_none());
    }
    assert!(registry.require("getName", HandleKind::Member).is_err());
}

#[test]
fn raw_header_coverage_needs_matching_binary_owner_and_exact_digest() {
    for mutate in [0, 1, 2, 3, 4] {
        let mut reader = Reader::new();
        match mutate {
            0 => {
                reader.ty["abi"]["value"]["identity"]["internalName"] =
                    json!("fixture/Model$$serializer")
            }
            1 => reader.ty["abi"]["value"]["identity"]["contentSha256"] = json!("A".repeat(64)),
            2 => reader.ty["binaryName"]["value"]["descriptor"] = json!("Lwrong/Owner;"),
            3 => reader.ty["jvmAbiCoverage"] = unknown("IncompleteAbi"),
            _ => reader.ty["abi"]["value"]["methods"][0]["descriptor"] = json!("not-a-descriptor"),
        };
        let mut registry = new_registry();
        let ops = ops();
        let result = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader)
            .unwrap()
            .type_jvm(TYPE);
        assert!(result.as_ref().is_err() || abi_members(&result.unwrap()).is_err());
    }
}

#[test]
fn raw_members_cannot_receive_fabricated_editing_ids() {
    let mut reader = Reader::new();
    reader.ty["abi"]["value"]["methods"][1]["id"] = json!("invented-source-member");
    let mut registry = new_registry();
    let ops = ops();
    assert!(
        ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader)
            .unwrap()
            .type_jvm(TYPE)
            .is_err()
    );
}

#[test]
fn full_metadata_and_abi_still_do_not_supply_project_sessions_or_executable_code() {
    assert!(matches!(
        full_project_session(),
        HostFact::Unavailable { .. }
    ));
    assert_eq!(
        require_complete(full_project_session(), false)
            .unwrap_err()
            .protocol()
            .code,
        McdevErrorCode::IncompleteProjectContext
    );
    assert_eq!(
        executable_bytecode().unwrap_err().protocol().code,
        McdevErrorCode::BytecodeUnavailable
    );
    let host = std::sync::Arc::new(command_host::RecordedHost::new());
    *host.session_failure.lock().unwrap() = Some(
        require_complete(full_project_session(), false)
            .unwrap_err()
            .command(),
    );
    let runtime = std::sync::Arc::new(mcdev_companion::commands::CommandDispatcher::new(host));
    let request = json!({"protocolVersion":1,"workspaceRoot":"file:///workspace","documentUri":"file:///workspace/Unsaved.java","languageId":"java","position":{"line":0,"character":0},"bufferText":"class Unsaved {}","client":{"name":"facts-test","version":"1"}});
    let response = runtime
        .execute(McdevCommands::INFO, &[request.clone()])
        .unwrap();
    assert!(response.result.is_none());
    assert_eq!(
        response.error.unwrap().code,
        McdevErrorCode::IncompleteProjectContext
    );
    let dto = mcdev_companion::commands::DelegateCommandHandler::new(runtime)
        .execute_command(
            McdevCommands::INFO,
            &[request],
            &mcdev_core::expression_engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(dto["error"]["code"], "INCOMPLETE_PROJECT_CONTEXT");
    assert!(dto.get("result").is_none());
}

#[test]
fn host_cancelled_or_stale_reads_never_return_partial_facts() {
    for reason in [
        "source replaced",
        "compiled cleared",
        "namespace changed",
        "model replaced",
        "sdk replaced",
        "classpath bytes replaced",
        "provider restarted",
    ] {
        let mut reader = Reader::new();
        reader.failure = Some(FactsError::Cancelled(reason.into()));
        let mut registry = new_registry();
        let ops = ops();
        assert!(matches!(
            ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader)
                .unwrap()
                .type_jvm(TYPE),
            Err(FactsError::Cancelled(_))
        ));
    }
}

#[test]
fn malformed_cardinality_and_unknown_reasons_are_rejected() {
    let mut reader = Reader::new();
    reader.extra_fact = true;
    let mut registry = new_registry();
    let ops = ops();
    assert!(
        ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader)
            .unwrap()
            .project_context()
            .is_err()
    );
    let mut reader = Reader::new();
    reader.ty["abi"] = json!({"state":"unknown","reasons":[]});
    let mut registry = new_registry();
    assert!(
        ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut reader)
            .unwrap()
            .type_jvm(TYPE)
            .is_err()
    );
}

#[test]
fn two_ops_process_worker() {
    if std::env::var("MCDEV_TWO_OPS_TEST_PORT").is_ok() {
        two_ops_process::worker();
    } else {
        assert!(SupportedOps::default().is_empty());
    }
}
#[test]
fn native_two_op_process_projection_reaches_core_and_dto_without_a_fake_session() {
    two_ops_process::run();
}
