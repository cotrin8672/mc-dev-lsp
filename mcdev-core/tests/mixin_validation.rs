mod mixin_validation_support;
use mcdev_core::{
    mixin_diagnostics::MixinDiagnosticCodes as C, mixin_source::*, mixin_validation::*, model::*,
};
use mixin_validation_support::*;
fn shadow(
    name: &str,
    descriptor: &str,
    is_method: bool,
    is_static: bool,
) -> ShadowMemberDeclaration {
    ShadowMemberDeclaration {
        name: name.into(),
        descriptor: descriptor.into(),
        is_method,
        is_static,
        range: McTextRange::default(),
        parse_source: ParseSource::HandWritten,
        confidence: ParseConfidence::High,
        warnings: vec![],
    }
}
fn accessor(
    name: &str,
    ret: &str,
    params: &[&str],
    explicit: Option<&str>,
) -> AccessorMethodDeclaration {
    AccessorMethodDeclaration {
        method_name: name.into(),
        return_type_descriptor: Some(ret.into()),
        parameter_descriptors: params.iter().map(|s| s.to_string()).collect(),
        explicit_field_name: explicit.map(str::to_string),
        range: McTextRange::default(),
        parse_source: ParseSource::HandWritten,
        confidence: ParseConfidence::High,
        warnings: vec![],
    }
}
fn invoker(
    name: &str,
    ret: Option<&str>,
    params: &[&str],
    explicit: Option<&str>,
) -> InvokerMethodDeclaration {
    InvokerMethodDeclaration {
        method_name: name.into(),
        return_type_descriptor: ret.map(str::to_string),
        parameter_descriptors: params.iter().map(|s| s.to_string()).collect(),
        explicit_target_name: explicit.map(str::to_string),
        range: McTextRange::default(),
        parse_source: ParseSource::HandWritten,
        confidence: ParseConfidence::High,
        warnings: vec![],
    }
}
#[test]
fn shadow_field_method_static_descriptor_prefix_and_multiowner() {
    let idx = Index::default();
    let svc = ShadowValidationService::new(&idx);
    let targets = vec![OWNER.into()];
    for (name, desc, m, stat, code) in [
        ("currentScreen", FD, false, false, None),
        (
            "missingField",
            "I",
            false,
            false,
            Some(C::SHADOW_TARGET_NOT_FOUND),
        ),
        (
            "currentScreen",
            FD,
            false,
            true,
            Some(C::SHADOW_STATIC_MISMATCH),
        ),
        (
            "currentScreen",
            "I",
            false,
            false,
            Some(C::SHADOW_DESCRIPTOR_MISMATCH),
        ),
        ("tick", "()V", true, false, None),
        (
            "missingMethod",
            "()V",
            true,
            false,
            Some(C::SHADOW_TARGET_NOT_FOUND),
        ),
        (
            "tick",
            "(I)V",
            true,
            false,
            Some(C::SHADOW_DESCRIPTOR_MISMATCH),
        ),
        (
            "tick",
            "bad",
            true,
            false,
            Some(C::SHADOW_DESCRIPTOR_MISMATCH),
        ),
        ("tick", "()V", true, true, Some(C::SHADOW_STATIC_MISMATCH)),
        ("currentScreen", "bad", false, false, None),
    ] {
        let ds = svc.validate(&targets, &shadow(name, desc, m, stat), None, true);
        assert_eq!(ds.first().map(|d| d.code.as_str()), code, "{name} {desc}");
    }
    assert!(svc
        .validate(
            &targets,
            &shadow("shadow$currentScreen", FD, false, false),
            Some("shadow$"),
            false
        )
        .is_empty());
    assert!(svc
        .validate(&[], &shadow("missing", "I", false, false), None, true)
        .is_empty());
    assert!(svc
        .validate(
            &["missing".into(), OWNER.into()],
            &shadow("tick", "()V", true, false),
            None,
            true
        )
        .is_empty());
    let ds = svc.validate(
        &["missing".into(), OWNER.into()],
        &shadow("tick", "()V", true, true),
        None,
        true,
    );
    assert_eq!(ds[0].code, C::SHADOW_TARGET_NOT_FOUND);
}
#[test]
fn accessor_inference_all_shapes_and_signature_branches() {
    let idx = Index::default();
    let svc = AccessorService::new(&idx);
    let owners = vec![OWNER.into()];
    for (name, ret, params, explicit, inferred, kind, code) in [
        (
            "getCurrentScreen",
            FD,
            vec![],
            None,
            Some("currentScreen"),
            AccessorKind::Getter,
            None,
        ),
        (
            "setCurrentScreen",
            "V",
            vec![FD],
            None,
            Some("currentScreen"),
            AccessorKind::Setter,
            None,
        ),
        (
            "isCounter",
            "I",
            vec![],
            None,
            Some("counter"),
            AccessorKind::Getter,
            None,
        ),
        (
            "read",
            "I",
            vec![],
            Some("counter"),
            Some("counter"),
            AccessorKind::Getter,
            None,
        ),
        (
            "write",
            "V",
            vec!["I"],
            Some("counter"),
            Some("counter"),
            AccessorKind::Setter,
            None,
        ),
        (
            "getMissing",
            "I",
            vec![],
            None,
            Some("missing"),
            AccessorKind::Getter,
            Some(C::ACCESSOR_FIELD_NOT_FOUND),
        ),
        (
            "getCurrentScreen",
            "I",
            vec![],
            Some("currentScreen"),
            Some("currentScreen"),
            AccessorKind::Getter,
            Some(C::ACCESSOR_SIGNATURE_MISMATCH),
        ),
        (
            "setCurrentScreen",
            "I",
            vec![FD],
            None,
            Some("currentScreen"),
            AccessorKind::Setter,
            Some(C::ACCESSOR_SIGNATURE_MISMATCH),
        ),
        (
            "odd",
            "V",
            vec![],
            Some("counter"),
            Some("counter"),
            AccessorKind::Unknown,
            Some(C::ACCESSOR_SIGNATURE_MISMATCH),
        ),
        (
            "unknown",
            "V",
            vec![],
            None,
            None,
            AccessorKind::Unknown,
            Some(C::ACCESSOR_FIELD_NOT_FOUND),
        ),
    ] {
        let d = accessor(name, ret, &params, explicit);
        assert_eq!(AccessorService::infer_field_name(&d).as_deref(), inferred);
        assert_eq!(AccessorService::infer_kind(&d), kind);
        assert_eq!(
            svc.validate(&owners, &d).first().map(|d| d.code.as_str()),
            code,
            "{name}"
        );
    }
    let items = svc.complete_fields(&owners, "current");
    assert_eq!(items[0].insert_text, "currentScreen");
    assert_eq!(items[0].metadata.source, "mixin.accessor");
    assert!(AccessorService::generate_method_stub(
        &field("currentScreen", FD, false),
        AccessorKind::Getter,
        "  "
    )
    .contains("getCurrentScreen"));
    assert!(AccessorService::generate_method_stub(
        &field("counter", "I", false),
        AccessorKind::Setter,
        "  "
    )
    .contains("void setCounter(int value)"));
    assert!(AccessorService::generate_method_stub(
        &field("counter", "I", false),
        AccessorKind::Unknown,
        ""
    )
    .is_empty());
    assert_eq!(AccessorService::readable_field_type("I"), "int");
    assert_eq!(AccessorService::readable_field_type("invalid"), "invalid");
}
#[test]
fn invoker_exact_overloads_explicit_inference_constructor_and_stubs() {
    let idx = Index::default();
    let svc = InvokerService::new(&idx);
    let owners = vec![OWNER.into()];
    for (name, ret, params, explicit, code) in [
        ("invokeSetScreen", Some("V"), vec![FD], None, None),
        ("invokeRender", Some("V"), vec![], None, None),
        (
            "invokeMissing",
            Some("V"),
            vec![],
            Some("missing"),
            Some(C::INVOKER_METHOD_NOT_FOUND),
        ),
        (
            "invokeSetScreen",
            Some("V"),
            vec!["I"],
            Some("setScreen"),
            Some(C::INVOKER_DESCRIPTOR_MISMATCH),
        ),
        ("bad", None, vec![], None, Some(C::INVOKER_METHOD_NOT_FOUND)),
        ("construct", Some("V"), vec!["I"], Some("<init>"), None),
    ] {
        let d = invoker(name, ret, &params, explicit);
        assert_eq!(
            svc.validate(&owners, &d).first().map(|d| d.code.as_str()),
            code,
            "{name}"
        );
    }
    assert_eq!(
        InvokerService::infer_target_name(&invoker("invokeSetScreen", Some("V"), &[FD], None))
            .as_deref(),
        Some("setScreen")
    );
    let items = svc.complete_methods(&owners, "set");
    assert_eq!(items[0].insert_text, "setScreen");
    assert_eq!(items[0].metadata.source, "mixin.invoker");
    assert!(
        InvokerService::generate_method_stub(&method("setScreen", DESC, false), "    ")
            .contains("invokeSetScreen")
    );
}
#[test]
fn shadow_completions_deduplicate_exclude_initializers_and_strip_prefix() {
    let idx = Index::default();
    let svc = ShadowValidationService::new(&idx);
    let owners = vec![OWNER.into(), OWNER.into()];
    assert_eq!(svc.complete_fields(&owners, "current", None).len(), 1);
    let ms = svc.complete_methods(&owners, "", None);
    assert!(!ms
        .iter()
        .any(|m| m.name == "<init>" || m.name == "<clinit>"));
    assert_eq!(ms.iter().filter(|m| m.name == "render").count(), 2);
}
#[test]
fn overwrite_static_descriptor_and_multiowner() {
    let idx = Index::default();
    let svc = OverwriteValidationService::new(&idx);
    for (name, desc, stat, code) in [
        ("tick", "()V", false, None),
        ("missing", "()V", false, Some(C::OVERWRITE_TARGET_NOT_FOUND)),
        ("tick", "bad", false, Some(C::OVERWRITE_DESCRIPTOR_MISMATCH)),
        (
            "tick",
            "(I)V",
            false,
            Some(C::OVERWRITE_DESCRIPTOR_MISMATCH),
        ),
        ("tick", "()V", true, Some(C::OVERWRITE_STATIC_MISMATCH)),
    ] {
        let d = OverwriteMethodDeclaration {
            name: name.into(),
            descriptor: desc.into(),
            is_static: stat,
            range: McTextRange::default(),
            parse_source: ParseSource::HandWritten,
            confidence: ParseConfidence::High,
            warnings: vec![],
        };
        assert_eq!(
            svc.validate(&[OWNER.into()], &d)
                .first()
                .map(|d| d.code.as_str()),
            code
        );
        assert!(svc.validate(&[], &d).is_empty());
    }
}
#[test]
fn member_resolver_every_policy_and_exact_descriptor_gate() {
    let idx = Index::default();
    let r = ClassIndexMemberResolver::new(&idx);
    use AmbiguityPolicy::*;
    assert!(matches!(
        r.resolve_method("missing", "tick", None, ExactDescriptorRequired),
        MethodResolution::UnresolvedOwner(_)
    ));
    assert!(matches!(
        r.resolve_field("missing", "counter", None, ExactDescriptorRequired),
        FieldResolution::UnresolvedOwner(_)
    ));
    assert!(matches!(
        r.resolve_method(OWNER, "absent", None, ExactDescriptorRequired),
        MethodResolution::NotFound
    ));
    assert!(matches!(
        r.resolve_method(OWNER, "tick", None, ExactDescriptorRequired),
        MethodResolution::MissingDescriptor(_)
    ));
    assert!(matches!(
        r.resolve_method(OWNER, "render", None, SingleCandidateIfDescriptorMissing),
        MethodResolution::Ambiguous(_)
    ));
    assert!(matches!(
        r.resolve_method(OWNER, "tick", None, AllowAmbiguousForCompletion),
        MethodResolution::Ambiguous(_)
    ));
    assert!(matches!(
        r.resolve_method(OWNER, "tick", None, SingleCandidateIfDescriptorMissing),
        MethodResolution::Resolved(_)
    ));
    assert!(matches!(
        r.resolve_method(OWNER, "render", Some("()V"), ExactDescriptorRequired),
        MethodResolution::Resolved(_)
    ));
    assert!(matches!(
        r.resolve_method(
            OWNER,
            "render",
            Some("(J)V"),
            SingleCandidateIfDescriptorMissing
        ),
        MethodResolution::DescriptorMismatch(_)
    ));
    assert!(matches!(
        r.resolve_field(
            OWNER,
            "counter",
            Some("J"),
            SingleCandidateIfDescriptorMissing
        ),
        FieldResolution::DescriptorMismatch(_)
    ));
    assert!(matches!(
        r.resolve_field(OWNER, "counter", Some("I"), ExactDescriptorRequired),
        FieldResolution::Resolved(_)
    ));
    assert!(matches!(
        r.resolve_field(OWNER, "counter", None, ExactDescriptorRequired),
        FieldResolution::MissingDescriptor(_)
    ));
    assert!(matches!(
        r.resolve_field(OWNER, "counter", None, AllowAmbiguousForCompletion),
        FieldResolution::Ambiguous(_)
    ));
    assert!(matches!(
        r.resolve_field(OWNER, "counter", None, SingleCandidateIfDescriptorMissing),
        FieldResolution::Resolved(_)
    ));
    assert!(matches!(
        r.resolve_field(OWNER, "absent", None, SingleCandidateIfDescriptorMissing),
        FieldResolution::NotFound
    ));
}
