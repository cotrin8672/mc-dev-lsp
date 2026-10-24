use mcdev_core::{extras_model::*, mixinextras_handler::*, *};
use std::collections::BTreeSet;
struct Index;
impl ClassIndex for Index {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        vec![]
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        (name == "test/Target").then(|| ClassIndexEntry {
            simple_name: "Target".into(),
            package_name: "test".into(),
            internal_name: name.into(),
        })
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.find_class(&name.replace('.', "/"))
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        if owner == "test/Target" {
            vec![MethodIndexEntry {
                name: "read".into(),
                descriptor: "()Ljava/lang/String;".into(),
                is_static: false,
                readable_signature: "read(): String".into(),
            }]
        } else {
            vec![]
        }
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
struct Facts(Option<bool>);
impl HandlerReferenceTypeResolver for Facts {
    fn is_known_reference(&self, _: &str) -> Option<bool> {
        Some(true)
    }
    fn is_assignable(&self, _: &str, _: &str) -> Option<bool> {
        self.0
    }
}
struct SdkFacts;
impl HandlerReferenceTypeResolver for SdkFacts {
    fn is_known_reference(&self, name: &str) -> Option<bool> {
        Some(name == "java/lang/RuntimeException")
    }
    fn is_assignable(&self, _: &str, _: &str) -> Option<bool> {
        None
    }
}
#[test]
fn source_type_enrichment_uses_authoritative_sdk_facts_for_reflection_fallback() {
    let declaration = HandlerMethodDeclaration {
        return_type_name: "RuntimeException[][]".into(),
        ..Default::default()
    };
    let unavailable = HandlerSignatureService::enrich_handler_types(&declaration, &Index, None);
    assert_eq!(unavailable.return_type_descriptor, None);
    let resolved = HandlerSignatureService::enrich_handler_types_with_reference_types(
        &declaration,
        &Index,
        None,
        Some(&SdkFacts),
    );
    assert_eq!(
        resolved.return_type_descriptor.as_deref(),
        Some("[[Ljava/lang/RuntimeException;")
    );
}
fn input() -> (MixinExtrasAnnotationSite, HandlerMethodDeclaration) {
    let site = MixinExtrasAnnotationSite {
        annotation: MixinExtrasAnnotation::ModifyReturnValue,
        method_attribute: "read()Ljava/lang/String;".into(),
        at_value: None,
        at_target: None,
        at_args: vec![],
        at_id: None,
        at_ordinal: None,
        at_shift: AtShiftSpec::Before,
        has_constant_selector: false,
        annotation_range: McTextRange::default(),
        handler_method: None,
        expand_zero_conditions: BTreeSet::new(),
    };
    let handler = HandlerMethodDeclaration {
        method_name: "handler".into(),
        return_type_name: "CharSequence".into(),
        return_type_descriptor: Some("Ljava/lang/CharSequence;".into()),
        has_method_level_coerce: true,
        parameters: vec![HandlerParameterDeclaration {
            name: "original".into(),
            type_name: "String".into(),
            type_descriptor: Some("Ljava/lang/String;".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    (site, handler)
}
#[test]
fn absent_host_facts_remain_unknown_and_do_not_create_diagnostics() {
    let service = HandlerSignatureService::new(&Index, None);
    assert_eq!(service.known_reference("java/lang/String"), None);
    assert_eq!(
        service.coerce_reference_knowledge("java/lang/CharSequence", "java/lang/String"),
        None
    );
    let (site, handler) = input();
    assert!(service
        .validate_handler("", &site, &["test/Target".into()], &handler, None)
        .is_empty());
}
#[test]
fn missing_sdk_parameter_descriptor_is_not_a_proven_signature_mismatch() {
    let service = HandlerSignatureService::new(&Index, None);
    let (site, mut handler) = input();
    handler.return_type_descriptor = Some("Ljava/lang/String;".into());
    handler.has_method_level_coerce = false;
    handler.parameters[0].type_name = "StringBuilder".into();
    handler.parameters[0].type_descriptor = None;
    assert!(service
        .validate_handler("", &site, &["test/Target".into()], &handler, None)
        .is_empty());
}
#[test]
fn unavailable_assignability_is_distinct_from_known_incompatibility() {
    let unknown = Facts(None);
    let incompatible = Facts(Some(false));
    let known = HandlerSignatureService::new(&Index, None).with_reference_types(&incompatible);
    let service = HandlerSignatureService::new(&Index, None).with_reference_types(&unknown);
    assert_eq!(
        service.coerce_reference_knowledge("java/lang/CharSequence", "java/lang/String"),
        None
    );
    assert_eq!(
        known.coerce_reference_knowledge("java/lang/CharSequence", "java/lang/String"),
        Some(false)
    );
    let (site, handler) = input();
    assert!(service
        .validate_handler("", &site, &["test/Target".into()], &handler, None)
        .is_empty());
    let issues = known.validate_handler("", &site, &["test/Target".into()], &handler, None);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, diagnostic_codes::WRONG_RETURN_TYPE);
}
#[test]
fn authoritative_assignability_accepts_matching_host_contract() {
    let facts = Facts(Some(true));
    let service = HandlerSignatureService::new(&Index, None).with_reference_types(&facts);
    assert_eq!(
        service.coerce_reference_knowledge("java/lang/CharSequence", "java/lang/String"),
        Some(true)
    );
    let (site, handler) = input();
    assert!(service
        .validate_handler("", &site, &["test/Target".into()], &handler, None)
        .is_empty());
}
#[test]
fn primitive_and_array_decisions_do_not_require_reference_facts() {
    let service = HandlerSignatureService::new(&Index, None);
    let accepted = BTreeSet::new();
    assert_eq!(
        service.type_match_knowledge(Some("B"), "I", true, &accepted),
        Some(true)
    );
    assert_eq!(
        service.type_match_knowledge(Some("J"), "I", true, &accepted),
        Some(false)
    );
    assert_eq!(
        service.type_match_knowledge(Some("[B"), "[I", true, &accepted),
        Some(false)
    );
    assert_eq!(
        service.type_match_knowledge(
            Some("[[Ljava/lang/Object;"),
            "[Ljava/lang/String;",
            true,
            &accepted
        ),
        Some(false)
    );
    assert_eq!(
        service.type_match_knowledge(
            Some("[Ljava/lang/CharSequence;"),
            "[Ljava/lang/String;",
            true,
            &accepted
        ),
        None
    );
}
#[test]
fn java_hex_float_rounding_avoids_intermediate_overflow_and_double_rounding() {
    use mcdev_core::mixinextras_occurrence::{parse_java_f32 as float, parse_java_float as double};
    assert_eq!(double("0x1p999999999999999"), Some(f64::INFINITY));
    assert_eq!(double("0x0p999999999999999"), Some(0.0));
    assert_eq!(
        double("-0x1p-999999999999999").unwrap().to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(double("0x1p-1074").unwrap().to_bits(), 1);
    assert_eq!(double("0x1p-1075").unwrap().to_bits(), 0);
    assert_eq!(double("0x1.00000000000001p-1075").unwrap().to_bits(), 1);
    assert_eq!(
        float("0x1.00000100000000001p0").unwrap().to_bits(),
        0x3f800001
    );
    assert_eq!(double("NaNf"), None);
    assert_eq!(float("Infinityd"), None);
    let huge = format!("0x1{}p-{}", "0".repeat(1000), 4000);
    assert_eq!(double(&huge), Some(1.0));
}
#[test]
fn incomplete_nested_local_rejects_the_unbalanced_parent_handler_annotation() {
    let source = include_str!("fixtures/mixinextras_handler/unbalanced-parent-annotation.java");
    assert!(mcdev_core::mixinextras_handler::HandlerSignatureService::find_sugar_handler_annotation_sites(source).is_empty());
    assert!(
        mcdev_core::mixinextras_handler::HandlerSignatureService::find_annotation_sites(source)
            .is_empty()
    );
    let corrected = source.replace("@Local( }", "@Local() }");
    assert_eq!(mcdev_core::mixinextras_handler::HandlerSignatureService::find_sugar_handler_annotation_sites(&corrected).len(), 1);
}
