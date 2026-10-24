use mcdev_core::{bytecode::*, host_source::*, *};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
fn project(version: u64) -> HostProjectKey {
    HostProjectKey {
        project_id: "project".into(),
        generations: HostGenerations {
            project: version,
            ..Default::default()
        },
    }
}
struct Scope {
    authoritative: AtomicBool,
    registrations: AtomicUsize,
}
impl Default for Scope {
    fn default() -> Self {
        Self {
            authoritative: AtomicBool::new(true),
            registrations: AtomicUsize::new(0),
        }
    }
}
impl HostReadValidator for Scope {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        self.registrations.fetch_add(1, Ordering::SeqCst);
        Ok(self.authoritative.load(Ordering::SeqCst))
    }
}
impl HostCacheReadScope for Scope {
    fn current_read_set(&self) -> HostReadSet {
        vec![]
    }
    fn current_reads_are_authoritative(&self) -> bool {
        self.authoritative.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
struct Index {
    method: String,
    candidate: String,
    bytes: Option<Vec<u8>>,
    super_class: Option<String>,
    method_reads: AtomicUsize,
    candidate_reads: AtomicUsize,
}
impl ClassIndex for Index {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        vec![]
    }
    fn find_class(&self, _: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
        None
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        self.method_reads.fetch_add(1, Ordering::SeqCst);
        if self.method.is_empty() {
            vec![]
        } else {
            vec![MethodIndexEntry {
                name: self.method.clone(),
                descriptor: "()V".into(),
                is_static: false,
                readable_signature: format!("{}(): void", self.method),
            }]
        }
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
impl BytecodeIndex for Index {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        self.candidate_reads.fetch_add(1, Ordering::SeqCst);
        if self.candidate.is_empty() {
            vec![]
        } else {
            vec![AtTargetCandidate::new(
                AtTargetKind::Invoke,
                "example/Hook",
                &self.candidate,
                "()V",
            )]
        }
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        0
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        (owner == "example/Target")
            .then(|| self.bytes.clone())
            .flatten()
    }
    fn get_class_bytes_ref(&self, owner: &str) -> Option<&[u8]> {
        (owner == "example/Target")
            .then(|| self.bytes.as_deref())
            .flatten()
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        if (left, right) == ("Lexample/ChildOne;", "Lexample/ChildTwo;") {
            self.super_class.clone()
        } else {
            None
        }
    }
}
#[test]
fn class_index_member_cache_does_not_cross_contaminate_different_delegates() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let scope = Arc::new(Scope::default());
    let first = caches.class_index(
        Arc::new(Index {
            method: "first".into(),
            ..Default::default()
        }),
        "first-delegate".into(),
        project(1),
        scope.clone(),
    );
    let second = caches.class_index(
        Arc::new(Index {
            method: "second".into(),
            ..Default::default()
        }),
        "second-delegate".into(),
        project(1),
        scope,
    );
    assert_eq!(
        first
            .get_methods("example/Target")
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["first"]
    );
    assert_eq!(
        second
            .get_methods("example/Target")
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["second"]
    );
}
#[test]
fn class_index_member_cache_is_bounded_across_project_session_versions() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let scope = Arc::new(Scope::default());
    let delegate = Arc::new(Index {
        method: "run".into(),
        ..Default::default()
    });
    for version in 0..600 {
        caches
            .class_index(
                delegate.clone(),
                "stable".into(),
                project(version),
                scope.clone(),
            )
            .get_methods("example/Target");
    }
    assert!(caches.sizes().0 <= 256);
}
#[test]
fn bytecode_index_at_target_cache_is_bounded_across_project_session_versions() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let scope = Arc::new(Scope::default());
    let delegate = Arc::new(Index {
        candidate: "hook".into(),
        ..Default::default()
    });
    for version in 0..600 {
        caches
            .bytecode_index(
                delegate.clone(),
                "stable".into(),
                project(version),
                scope.clone(),
            )
            .get_at_target_candidates("example/Target", "run", Some("()V"), "INVOKE");
    }
    assert!(caches.sizes().2 <= 256);
}
#[test]
fn bytecode_index_get_class_bytes_delegates_exact_bytes_and_missing_stays_null() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let delegate = Arc::new(Index {
        bytes: Some(vec![0xca, 0xfe, 0xba, 0xbe]),
        ..Default::default()
    });
    let wrapped = caches.bytecode_index(
        delegate.clone(),
        "stable".into(),
        project(1),
        Arc::new(Scope::default()),
    );
    caches.reset_debug();
    assert_eq!(caches.sizes().2, 0);
    assert!(std::ptr::eq(
        delegate.get_class_bytes_ref("example/Target").unwrap(),
        wrapped.get_class_bytes_ref("example/Target").unwrap()
    ));
    assert_eq!(
        wrapped.get_class_bytes("example/Target"),
        Some(vec![0xca, 0xfe, 0xba, 0xbe])
    );
    assert_eq!(wrapped.get_class_bytes("missing/Class"), None);
    assert_eq!(caches.sizes().2, 0);
    assert_eq!(caches.debug(), CandidateCacheDebug::default());
}
#[test]
fn bytecode_index_resolve_common_super_class_delegates_values_and_missing_stays_null() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let delegate = Arc::new(Index {
        super_class: Some("Lexample/Base;".into()),
        ..Default::default()
    });
    let wrapped = caches.bytecode_index(
        delegate.clone(),
        "stable".into(),
        project(1),
        Arc::new(Scope::default()),
    );
    caches.reset_debug();
    assert_eq!(caches.sizes().2, 0);
    assert_eq!(
        wrapped.resolve_common_super_class("Lexample/ChildOne;", "Lexample/ChildTwo;"),
        Some("Lexample/Base;".into())
    );
    assert_eq!(
        delegate.resolve_common_super_class("Lexample/ChildOne;", "Lexample/ChildTwo;"),
        Some("Lexample/Base;".into())
    );
    assert_eq!(
        wrapped.resolve_common_super_class("Lmissing/Alpha;", "Lmissing/Beta;"),
        None
    );
    assert_eq!(caches.sizes().2, 0);
    assert_eq!(caches.debug(), CandidateCacheDebug::default());
}
#[test]
fn expression_member_completion_cache_reuses_same_instance_for_same_delegates_and_version() {
    let caches = CompletionIndexCaches::new();
    let scope = Scope::default();
    let first = caches
        .expression_member_completion_cache("class1".into(), "bytecode1".into(), project(1), &scope)
        .unwrap();
    let second = caches
        .expression_member_completion_cache("class1".into(), "bytecode1".into(), project(1), &scope)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}
#[test]
fn expression_member_completion_cache_returns_different_instances_when_delegate_or_version_changes()
{
    let caches = CompletionIndexCaches::new();
    let scope = Scope::default();
    let baseline = caches
        .expression_member_completion_cache("class1".into(), "bytecode1".into(), project(1), &scope)
        .unwrap();
    for (class, bytes, version) in [
        ("class2", "bytecode1", 1),
        ("class1", "bytecode2", 1),
        ("class1", "bytecode1", 2),
    ] {
        assert!(!Arc::ptr_eq(
            &baseline,
            &caches
                .expression_member_completion_cache(
                    class.into(),
                    bytes.into(),
                    project(version),
                    &scope
                )
                .unwrap()
        ));
    }
}
#[test]
fn expression_member_completion_cache_is_bounded_across_project_session_versions() {
    let caches = CompletionIndexCaches::new();
    let scope = Scope::default();
    for version in 0..600 {
        caches
            .expression_member_completion_cache(
                "class".into(),
                "bytes".into(),
                project(version),
                &scope,
            )
            .unwrap();
    }
    assert!(caches.sizes().3 <= 16);
}
#[test]
fn bytecode_index_at_target_cache_does_not_cross_contaminate_different_delegates() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let scope = Arc::new(Scope::default());
    let first = caches.bytecode_index(
        Arc::new(Index {
            candidate: "firstHook".into(),
            ..Default::default()
        }),
        "first".into(),
        project(1),
        scope.clone(),
    );
    let second = caches.bytecode_index(
        Arc::new(Index {
            candidate: "secondHook".into(),
            ..Default::default()
        }),
        "second".into(),
        project(1),
        scope,
    );
    assert_eq!(
        first
            .get_at_target_candidates("example/Target", "run", Some("()V"), "INVOKE")
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["firstHook"]
    );
    assert_eq!(
        second
            .get_at_target_candidates("example/Target", "run", Some("()V"), "INVOKE")
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["secondHook"]
    );
}
#[test]
fn unavailable_empty_results_are_never_cached_as_authoritative_absence() {
    let caches = Arc::new(CompletionIndexCaches::new());
    let scope = Arc::new(Scope::default());
    scope.authoritative.store(false, Ordering::SeqCst);
    let delegate = Arc::new(Index::default());
    let wrapped = caches.class_index(delegate.clone(), "stable".into(), project(1), scope.clone());
    assert!(wrapped.get_methods("example/Target").is_empty());
    assert!(wrapped.get_methods("example/Target").is_empty());
    assert_eq!(delegate.method_reads.load(Ordering::SeqCst), 2);
    assert_eq!(caches.sizes().0, 0);
    scope.authoritative.store(true, Ordering::SeqCst);
    wrapped.get_methods("example/Target");
    wrapped.get_methods("example/Target");
    assert_eq!(delegate.method_reads.load(Ordering::SeqCst), 3);
    assert_eq!(caches.sizes().0, 1);
}
#[test]
fn utf16_expression_presence_does_not_emit_a_false_missing_annotation() {
    use mcdev_core::{
        extras_model::*, mixinextras_diagnostics::*, mixinextras_handler::HandlerSignatureService,
    };
    let source="@Mixin(Target.class) class M { @ModifyExpressionValue(method=\"run()V\", at=@At(\"MIXINEXTRAS:EXPRESSION\")) private int adjust(int value) { return value; } }";
    let site = HandlerSignatureService::find_annotation_sites(source)
        .into_iter()
        .next()
        .unwrap();
    let context = ResolvedMixinExtrasContext {
        handler_range: site.handler_method.as_ref().unwrap().range,
        context: ExpressionContext {
            expression_index: MixinExtrasExpressionIndex {
                expressions: vec![MixinExtrasExpression::from_utf16(
                    String::new(),
                    vec![vec![0xd800]],
                )],
            },
            ..Default::default()
        },
    };
    let index = Index::default();
    let diagnostics =
        MixinExtrasDiagnosticsService::new(&index, &index).analyze(&MixinExtrasDiagnosticRequest {
            source: source.into(),
            document_uri: "file:///M.java".into(),
            resolved_contexts: vec![context],
        });
    let code = mcdev_core::mixinextras_handler::diagnostic_codes::MISSING_EXPRESSION_ANNOTATION;
    assert!(
        !diagnostics.iter().any(|d| d.code == code),
        "{diagnostics:?}"
    );
    let empty =
        MixinExtrasDiagnosticsService::new(&index, &index).analyze(&MixinExtrasDiagnosticRequest {
            source: source.into(),
            document_uri: "file:///M.java".into(),
            resolved_contexts: vec![],
        });
    assert!(empty.iter().any(|d| d.code == code), "{empty:?}");
}
