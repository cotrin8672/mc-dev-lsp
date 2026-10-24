#[allow(dead_code)]
#[path = "fixtures/resources/mod.rs"]
mod resources;
use mcdev_core::{
    mixin_navigation::*,
    project::{ProjectContext, ProjectContextBuilder},
    *,
};
use std::sync::atomic::{AtomicBool, Ordering};
#[test]
fn host_index_unsupported_kind_preserves_legacy_message() {
    let resolved = simple_index(false).resolve(&target(
        MemberKind::Constructor,
        "com/example/target/SimpleTarget",
        Some("<init>"),
        Some("()V"),
    ));
    assert_eq!(resolved.resolution, DefinitionResolution::Unresolved);
    assert_eq!(
        resolved.resolution_message,
        Some("unsupported member kind CONSTRUCTOR".into())
    );
}
fn target(
    kind: MemberKind,
    owner: &str,
    name: Option<&str>,
    descriptor: Option<&str>,
) -> McDefinitionTarget {
    McDefinitionTarget {
        kind,
        owner_internal_name: owner.into(),
        owner_fqn: Some(owner.replace('/', ".")),
        name: name.map(str::to_owned),
        descriptor: descriptor.map(str::to_owned),
        namespace: MappingNamespace::Named,
        source_range: None,
        direct_source_document_uri: None,
    }
}
fn class_target() -> McDefinitionTarget {
    target(
        MemberKind::Class,
        "com/example/target/SimpleTarget",
        None,
        None,
    )
}
fn context() -> ProjectContext {
    ProjectContextBuilder::empty("source-boundary", "/workspace")
}
fn r(a: usize, b: usize, c: usize, d: usize) -> McTextRange {
    McTextRange {
        start: McTextPosition {
            line: a,
            character: b,
        },
        end: McTextPosition {
            line: c,
            character: d,
        },
    }
}
fn token_range(source: &str, needle: &str) -> McTextRange {
    let start = source.find(needle).unwrap();
    McTextRange {
        start: offset_to_position(source, start).unwrap(),
        end: offset_to_position(source, start + needle.len()).unwrap(),
    }
}
// These are the host's normalized declaration facts for the original fixture.
// The boundary consumes them; it never reparses or scans the Java source.
fn simple_index(mapped: bool) -> SourceIndex {
    let path = if mapped {
        "fixtures/fabric-loom-e2e/mapped-sources/com/example/target/SimpleTarget.java"
    } else {
        "fixtures/fabric-basic/src/main/java/com/example/target/SimpleTarget.java"
    };
    let text = resources::text(path);
    let uri = if mapped {
        "file:///workspace/mapped-sources/com/example/target/SimpleTarget.java"
    } else {
        "file:///workspace/src/main/java/com/example/target/SimpleTarget.java"
    };
    let declarations = vec![
        HostSourceDeclaration {
            target: class_target(),
            range: token_range(text, "public class SimpleTarget"),
        },
        HostSourceDeclaration {
            target: target(
                MemberKind::Field,
                "com/example/target/SimpleTarget",
                Some("counter"),
                Some("I"),
            ),
            range: token_range(text, "private int counter;"),
        },
        HostSourceDeclaration {
            target: target(
                MemberKind::Method,
                "com/example/target/SimpleTarget",
                Some("draw"),
                Some("(Ljava/lang/String;FF)V"),
            ),
            range: token_range(text, "draw"),
        },
    ];
    SourceIndex::from_host_snapshots(vec![HostSourceSnapshot {
        entry: SourceScanEntry {
            document_uri: uri.into(),
            text: text.into(),
        },
        generation: 1,
        owner_internal_names: vec!["com/example/target/SimpleTarget".into()],
        declarations,
    }])
}
fn fallback(target: &McDefinitionTarget, range: McTextRange) -> ResolvedDefinition {
    ResolvedDefinition {
        target: target.clone(),
        document_uri: "file:///jdt/SimpleTarget.java".into(),
        range,
        resolution: DefinitionResolution::Jdt,
        resolution_message: None,
    }
}
#[test]
fn source_index_test_resolves_class_declaration_in_project_source() {
    let index = simple_index(false);
    let resolved = index.resolve(&class_target());
    assert_eq!(resolved.resolution, DefinitionResolution::Source);
    assert_eq!(
        resolved.document_uri,
        index.snapshots()[0].entry.document_uri
    );
    assert_eq!(resolved.range.start.line, 2);
    assert_eq!(
        resolved.range,
        token_range(
            &index.snapshots()[0].entry.text,
            "public class SimpleTarget"
        )
    );
}
#[test]
fn source_index_test_resolves_field_declaration_in_project_source() {
    let index = simple_index(false);
    let resolved = index.resolve(&target(
        MemberKind::Field,
        "com/example/target/SimpleTarget",
        Some("counter"),
        Some("I"),
    ));
    assert_eq!(resolved.resolution, DefinitionResolution::Source);
    assert_eq!(
        resolved.document_uri,
        index.snapshots()[0].entry.document_uri
    );
    assert_eq!(resolved.range.start.line, 3);
}
#[test]
fn source_index_test_resolves_method_declaration_with_descriptor_disambiguation() {
    let resolved = simple_index(false).resolve(&target(
        MemberKind::Method,
        "com/example/target/SimpleTarget",
        Some("draw"),
        Some("(Ljava/lang/String;FF)V"),
    ));
    assert_eq!(resolved.resolution, DefinitionResolution::Source);
    assert_eq!(resolved.range.start.line, 5);
}
#[test]
fn source_index_test_resolves_same_arity_overload_by_parameter_descriptor() {
    let source="package com.example.target;\n\npublic class OverloadedTarget {\n    public void setValue(int value) {\n    }\n\n    public void setValue(String value) {\n    }\n}";
    let owner = "com/example/target/OverloadedTarget";
    let first = source.find("setValue").unwrap();
    let second = source[first + 1..].find("setValue").unwrap() + first + 1;
    let declarations = [("(I)V", first), ("(Ljava/lang/String;)V", second)]
        .into_iter()
        .map(|(descriptor, start)| HostSourceDeclaration {
            target: target(
                MemberKind::Method,
                owner,
                Some("setValue"),
                Some(descriptor),
            ),
            range: McTextRange {
                start: offset_to_position(source, start).unwrap(),
                end: offset_to_position(source, start + 8).unwrap(),
            },
        })
        .collect();
    let index = SourceIndex::from_host_snapshots(vec![HostSourceSnapshot {
        entry: SourceScanEntry {
            document_uri:
                "file:///workspace/src/main/java/com/example/target/OverloadedTarget.java".into(),
            text: source.into(),
        },
        generation: 1,
        owner_internal_names: vec![owner.into()],
        declarations,
    }]);
    let resolved = index.resolve(&target(
        MemberKind::Method,
        owner,
        Some("setValue"),
        Some("(Ljava/lang/String;)V"),
    ));
    assert_eq!(resolved.resolution, DefinitionResolution::Source);
    assert_eq!(resolved.range.start.line, 6);
}
#[test]
fn source_index_test_returns_unresolved_when_source_file_missing() {
    let resolved = SourceIndex::default().resolve(&target(
        MemberKind::Class,
        "com/example/missing/Missing",
        None,
        None,
    ));
    assert_eq!(resolved.resolution, DefinitionResolution::Unresolved);
    assert!(resolved
        .resolution_message
        .unwrap()
        .contains("no project source file"));
}
#[test]
fn definition_resolution_service_test_prefers_source_backend_over_jdt_backend() {
    let index = simple_index(false);
    let source = SourceDefinitionBackend {
        source_index: &index,
    };
    let called = AtomicBool::new(false);
    let host = |target: &McDefinitionTarget, _: &ProjectContext, _: &str| {
        called.store(true, Ordering::SeqCst);
        Some(fallback(target, r(0, 0, 0, 1)))
    };
    let result = DefinitionResolutionService::new(vec![&source, &host]).resolve_all(
        &[class_target()],
        &context(),
        "file:///workspace",
    );
    assert_eq!(result[0].resolution, DefinitionResolution::Source);
    assert!(!called.load(Ordering::SeqCst));
}
#[test]
fn definition_resolution_service_test_falls_back_to_jdt_backend_when_source_missing() {
    let index = SourceIndex::default();
    let source = SourceDefinitionBackend {
        source_index: &index,
    };
    let host = |target: &McDefinitionTarget, _: &ProjectContext, _: &str| {
        Some(fallback(target, r(4, 0, 4, 10)))
    };
    let resolved = DefinitionResolutionService::new(vec![&source, &host]).resolve_all(
        &[class_target()],
        &context(),
        "file:///workspace",
    );
    assert_eq!(resolved[0].resolution, DefinitionResolution::Jdt);
    assert!(resolved[0].document_uri.contains("jdt"));
    assert_eq!(resolved[0].range, r(4, 0, 4, 10));
    assert_eq!(resolved[0].target, class_target());
}
#[test]
fn definition_resolution_service_test_resolves_loom_mapped_sources_without_jdt_backend() {
    let index = simple_index(true);
    let source = SourceDefinitionBackend {
        source_index: &index,
    };
    let no_host = |_: &McDefinitionTarget, _: &ProjectContext, _: &str| None;
    let resolved = DefinitionResolutionService::new(vec![&source, &no_host]).resolve_all(
        &[class_target()],
        &context(),
        "file:///workspace",
    );
    assert_eq!(resolved[0].resolution, DefinitionResolution::Source);
    assert!(resolved[0].document_uri.contains("mapped-sources"));
    assert!(resolved[0].document_uri.contains("SimpleTarget.java"));
}
#[test]
fn definition_resolution_service_test_returns_unresolved_when_no_backend_matches() {
    let index = SourceIndex::default();
    let source = SourceDefinitionBackend {
        source_index: &index,
    };
    let no_host = |_: &McDefinitionTarget, _: &ProjectContext, _: &str| None;
    let resolved = DefinitionResolutionService::new(vec![&source, &no_host]).resolve_all(
        &[target(
            MemberKind::Class,
            "com/example/missing/Missing",
            None,
            None,
        )],
        &context(),
        "file:///workspace",
    );
    assert_eq!(resolved[0].resolution, DefinitionResolution::Unresolved);
    assert!(resolved[0]
        .resolution_message
        .as_deref()
        .unwrap()
        .contains("no navigable definition"));
}
fn direct(uri: Option<&str>, range: Option<McTextRange>) -> McDefinitionTarget {
    McDefinitionTarget {
        kind: MemberKind::Class,
        owner_internal_name: String::new(),
        owner_fqn: None,
        name: Some("drawCall".into()),
        descriptor: None,
        namespace: MappingNamespace::Named,
        source_range: range,
        direct_source_document_uri: uri.map(str::to_owned),
    }
}
#[test]
fn definition_resolution_service_test_resolves_direct_source_target_without_backends() {
    let range = r(4, 10, 4, 18);
    let target = direct(Some("file:///ExampleMixin.java"), Some(range));
    let called = AtomicBool::new(false);
    let backend = |_: &McDefinitionTarget, _: &ProjectContext, _: &str| {
        called.store(true, Ordering::SeqCst);
        None
    };
    let result = DefinitionResolutionService::new(vec![&backend])
        .resolve_all(&[target.clone()], &context(), "file:///workspace")
        .remove(0);
    assert_eq!(result.resolution, DefinitionResolution::Source);
    assert_eq!(result.document_uri, "file:///ExampleMixin.java");
    assert_eq!(result.range, range);
    assert_eq!(result.target, target);
    assert!(!called.load(Ordering::SeqCst));
}
#[test]
fn definition_resolution_service_test_direct_source_target_with_blank_uri_fails_closed() {
    let result = DefinitionResolutionService::default()
        .resolve_all(
            &[direct(Some(""), Some(r(1, 0, 1, 8)))],
            &context(),
            "file:///workspace",
        )
        .remove(0);
    assert_eq!(result.resolution, DefinitionResolution::Unresolved);
    assert!(result
        .resolution_message
        .unwrap()
        .contains("direct source target missing uri or range"));
}
#[test]
fn definition_resolution_service_test_direct_source_target_with_missing_range_fails_closed() {
    let result = DefinitionResolutionService::default()
        .resolve_all(
            &[direct(Some("file:///ExampleMixin.java"), None)],
            &context(),
            "file:///workspace",
        )
        .remove(0);
    assert_eq!(result.resolution, DefinitionResolution::Unresolved);
    assert!(result
        .resolution_message
        .unwrap()
        .contains("direct source target missing uri or range"));
}
#[test]
fn definition_resolution_service_test_target_with_source_range_but_no_direct_uri_uses_backend() {
    let index = SourceIndex::default();
    let source = SourceDefinitionBackend {
        source_index: &index,
    };
    let called = AtomicBool::new(false);
    let host = |target: &McDefinitionTarget, _: &ProjectContext, _: &str| {
        called.store(true, Ordering::SeqCst);
        Some(fallback(target, r(2, 0, 2, 10)))
    };
    let mut target = class_target();
    target.source_range = Some(r(0, 8, 0, 20));
    let result = DefinitionResolutionService::new(vec![&source, &host])
        .resolve_all(&[target], &context(), "file:///workspace")
        .remove(0);
    assert_eq!(result.resolution, DefinitionResolution::Jdt);
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn host_snapshot_generations_and_utf16_locations_are_immutable_facts() {
    let mut old = simple_index(false).snapshots().to_vec();
    old[0].entry.text = "// 😀\r\nclass SimpleTarget {}".into();
    old[0].declarations[0].range = r(1, 0, 1, 18);
    let first = SourceIndex::from_host_snapshots(old.clone());
    let mut newer = old.clone();
    newer[0].generation = 2;
    newer[0].entry.text = "// 😀 😀\r\n\r\nclass SimpleTarget {}".into();
    newer[0].declarations[0].range = r(2, 0, 2, 18);
    let next = SourceIndex::from_host_snapshots(newer);
    assert_eq!(first.snapshots()[0].generation, 1);
    assert_eq!(next.snapshots()[0].generation, 2);
    assert_eq!(first.resolve(&class_target()).range, r(1, 0, 1, 18));
    assert_eq!(next.resolve(&class_target()).range, r(2, 0, 2, 18));
    assert_eq!(first.snapshots()[0].entry.text, old[0].entry.text);
}
#[test]
fn backend_result_preserves_request_namespace_and_order() {
    let index = simple_index(false);
    let source = SourceDefinitionBackend {
        source_index: &index,
    };
    let mut request = class_target();
    request.namespace = MappingNamespace::Intermediary;
    request.source_range = Some(r(20, 4, 20, 7));
    let resolved = DefinitionResolutionService::new(vec![&source]).resolve_all(
        &[request.clone()],
        &context(),
        "file:///workspace",
    );
    assert_eq!(resolved[0].target, request);
    assert_eq!(resolved[0].target.namespace, MappingNamespace::Intermediary);
    assert_ne!(resolved[0].range, request.source_range.unwrap());
}
#[test]
fn invalid_direct_target_does_not_call_fallback() {
    let called = AtomicBool::new(false);
    let backend = |_: &McDefinitionTarget, _: &ProjectContext, _: &str| {
        called.store(true, Ordering::SeqCst);
        None
    };
    let result = DefinitionResolutionService::new(vec![&backend]).resolve_all(
        &[direct(Some("  "), Some(r(0, 0, 0, 1)))],
        &context(),
        "file:///workspace",
    );
    assert!(!called.load(Ordering::SeqCst));
    assert_eq!(result[0].resolution, DefinitionResolution::Unresolved);
}
