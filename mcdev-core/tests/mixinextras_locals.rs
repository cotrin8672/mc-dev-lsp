#[allow(dead_code)]
#[path = "fixtures/mixinextras_locals/replay.rs"]
mod replay;
use mcdev_core::{bytecode::*, extras_model::*, mixin_source::*, mixinextras_locals::*, *};
use std::collections::BTreeSet;
struct PanicIndex;
impl ClassIndex for PanicIndex {
    fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
        panic!("unexpected class lookup")
    }
    fn find_class(&self, _: &str) -> Option<ClassIndexEntry> {
        panic!("unexpected class lookup")
    }
    fn find_class_by_fqn(&self, _: &str) -> Option<ClassIndexEntry> {
        panic!("unexpected class lookup")
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        panic!("unexpected methods lookup")
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        panic!("unexpected fields lookup")
    }
}
struct Code {
    bytes: Option<Vec<u8>>,
}
impl BytecodeIndex for Code {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("unexpected candidate lookup")
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!("unexpected return lookup")
    }
    fn get_class_bytes(&self, _: &str) -> Option<Vec<u8>> {
        self.bytes.clone()
    }
}
fn point(indices: Option<BTreeSet<i32>>) -> LocalCaptureValidationPoint {
    LocalCaptureValidationPoint {
        owner: "Missing".into(),
        target_method: MethodIndexEntry {
            name: "run".into(),
            descriptor: "()V".into(),
            is_static: true,
            readable_signature: "run()".into(),
        },
        site: MixinExtrasAnnotationSite {
            annotation: MixinExtrasAnnotation::ModifyExpressionValue,
            method_attribute: "run".into(),
            at_value: Some("MIXINEXTRAS:EXPRESSION".into()),
            at_target: None,
            at_args: vec![],
            at_id: None,
            at_ordinal: None,
            at_shift: AtShiftSpec::Before,
            has_constant_selector: false,
            annotation_range: McTextRange::default(),
            handler_method: None,
            expand_zero_conditions: BTreeSet::new(),
        },
        expression_instruction_indices: indices,
    }
}
fn parameter() -> HandlerParameterDeclaration {
    HandlerParameterDeclaration {
        name: "local".into(),
        type_name: "int".into(),
        is_sugar: true,
        sugar_spec: Some(HandlerParameterSugarSpec::Local(LocalSugarSpec::default())),
        ..Default::default()
    }
}
#[test]
fn share_service_matches_scoped_ids_and_explicit_namespaces() {
    replay::replay_share_service(
        "routesShareIdCompletionAcrossMatchingHandlersAndExplicitNamespaces",
    );
}
#[test]
fn share_service_matches_java_escaped_values() {
    replay::replay_share_service("routesShareIdCompletionWithJavaEscapedValues");
}
#[test]
fn share_service_matches_additional_sources_and_iteration_count() {
    replay::replay_share_service("shareIdCompletionIncludesMatchingIdsFromAdditionalSources");
}
#[test]
fn share_early_guards_do_not_read_index_or_advance_additional_sources() {
    let service = ShareCompletionService::new(&PanicIndex);
    for (annotation, slot, attribute) in [
        (MixinAnnotation::Inject, AnnotationSlot::Value, None),
        (MixinAnnotation::Share, AnnotationSlot::Method, None),
        (
            MixinAnnotation::Share,
            AnnotationSlot::Value,
            Some("namespace"),
        ),
    ] {
        let mut context = AnnotationContext::new(annotation, slot, "");
        context.attribute_name = attribute.map(str::to_owned);
        let extra = std::iter::from_fn(|| -> Option<&str> {
            panic!("additional source advanced before guard")
        });
        assert!(service
            .complete("class Test {}", &context, extra)
            .is_empty());
    }
}
#[test]
fn share_without_current_handler_keeps_additional_sources_lazy() {
    let source="import org.spongepowered.asm.mixin.Mixin; @Mixin(Target.class) class Test { void run() {} }";
    let mut context = AnnotationContext::new(MixinAnnotation::Share, AnnotationSlot::Value, "");
    context.annotation_start_offset = source.find("void").unwrap();
    let extra = std::iter::from_fn(|| -> Option<&str> {
        panic!("additional source advanced without current key")
    });
    assert!(ShareCompletionService::new(&PanicIndex)
        .complete(source, &context, extra)
        .is_empty());
}
#[test]
fn validation_distinguishes_unknown_empty_occurrences_and_corrupt_extraction() {
    let code = Code {
        bytes: Some(vec![1]),
    };
    let service = LocalCaptureValidationService::new(&PanicIndex, &code);
    let parameter = parameter();
    let results = service
        .validate(
            "class Mixin {}",
            &parameter,
            &[
                point(None),
                point(Some(BTreeSet::new())),
                point(Some(BTreeSet::from([0]))),
            ],
        )
        .unwrap();
    assert!(
        matches!(&results[0],LocalCaptureValidationResult::Unavailable{snapshots,..}if snapshots.is_empty())
    );
    assert!(
        matches!(&results[1],LocalCaptureValidationResult::NotFound{snapshots,..}if snapshots.is_empty())
    );
    assert!(
        matches!(&results[2],LocalCaptureValidationResult::Unavailable{snapshots,..}if snapshots.is_empty())
    );
    let no_bytes = Code { bytes: None };
    assert!(matches!(
        &LocalCaptureValidationService::new(&PanicIndex, &no_bytes)
            .validate(
                "class Mixin {}",
                &parameter,
                &[point(Some(BTreeSet::new()))]
            )
            .unwrap()[0],
        LocalCaptureValidationResult::Unavailable { .. }
    ));
}

#[test]
fn normalized_local_validation_keeps_checked_handler_types_without_source_lookup() {
    use mcdev_core::{host_source::HostFact, mixinextras_handler::HandlerSignatureService};
    use sha2::{Digest, Sha256};
    let source = "class Untyped {}";
    let code = Code {
        bytes: Some(vec![1]),
    };
    let mut parameter = parameter();
    parameter.type_descriptor = Some("I".into());
    let mut point = point(Some(BTreeSet::new()));
    point.site.handler_method = Some(HandlerMethodDeclaration {
        method_name: "handler".into(),
        return_type_name: "not.Indexed".into(),
        return_type_descriptor: Some("Lnot/Indexed;".into()),
        parameters: vec![parameter.clone()],
        ..Default::default()
    });
    let facts = HostHandlerSource {
        document_uri: "file:///Untyped.java".into(),
        sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        qualified_name: "Untyped".into(),
        targets: vec!["Missing".into()],
        sites: HostFact::Known(vec![point.site.clone()]),
        slice_selections: vec![],
    };
    let signature = HandlerSignatureService::new(&PanicIndex, Some(&code)).with_host_source(facts);
    let results = LocalCaptureValidationService::new(&PanicIndex, &code)
        .with_signature_service(signature)
        .validate(source, &parameter, &[point])
        .unwrap();
    assert!(matches!(
        &results[0],
        LocalCaptureValidationResult::NotFound { .. }
    ));
}
#[test]
fn validation_non_local_sugar_does_not_resolve_unrelated_parameter_types() {
    let code = Code { bytes: None };
    let parameter = HandlerParameterDeclaration {
        type_name: "UnresolvedType".into(),
        sugar_spec: Some(HandlerParameterSugarSpec::Share(ShareSugarSpec::default())),
        ..Default::default()
    };
    assert!(matches!(
        &LocalCaptureValidationService::new(&PanicIndex, &code)
            .validate("class Test {}", &parameter, &[point(None)])
            .unwrap()[0],
        LocalCaptureValidationResult::Unavailable { .. }
    ));
}
#[test]
fn local_error_messages_preserve_stable_details() {
    assert_eq!(
        LocalCaptureError::MissingClassBytes.message(),
        "class bytes missing"
    );
    assert_eq!(
        LocalCaptureError::EmptyClassBytes.message(),
        "class bytes are empty"
    );
    assert_eq!(
        LocalCaptureError::MethodNotFound {
            method_name: "run".into(),
            method_descriptor: "()V".into()
        }
        .message(),
        "method not found: run()V"
    );
    assert_eq!(
        LocalCaptureError::InvalidInstructionIndex {
            instruction_occurrence_index: -1,
            instruction_count: 8
        }
        .message(),
        "invalid instruction occurrence index -1 (instruction count=8)"
    );
    assert_eq!(
        LocalCaptureError::CorruptClassBytes {
            detail: "bad constant pool".into()
        }
        .message(),
        "class bytes corrupt: bad constant pool"
    );
    assert_eq!(
        LocalCaptureError::Analysis {
            detail: "stack underflow".into()
        }
        .message(),
        "bytecode analysis failed: stack underflow"
    );
}
#[test]
fn constant_hint_preserves_isolated_utf16_units() {
    let mut candidate = AtTargetCandidate::new(AtTargetKind::Constant, "", "", "");
    candidate.constant_value = Some(ConstantValue::StringUtf16Value(vec![
        0xd800,
        b'"' as u16,
        0xdc00,
    ]));
    assert_eq!(
        mcdev_core::mixin_completion::ConstantAtHintFormatter::hint_for(&candidate),
        Some("@Constant(stringValue = \"\\uD800\\\"\\uDC00\")".into())
    );
}

#[test]
fn empty_indices_skip_analysis_after_class_and_method_validation() {
    let row = include_str!("fixtures/mixinextras_locals/legacy_calls.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| {
            row["api"] == "LocalCaptureExtractor.extract"
                && row["legacyCase"]
                    .as_str()
                    .unwrap()
                    .ends_with("analysisFailureFailsClosed")
        })
        .unwrap();
    let bytes = row["args"]["classBytes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_i64().unwrap() as u8)
        .collect::<Vec<_>>();
    assert_eq!(
        LocalCaptureExtractor::extract(Some(&bytes), "broken", "()V", &BTreeSet::new()).unwrap(),
        LocalCaptureResult::Success { snapshots: vec![] }
    );
    assert!(matches!(
        LocalCaptureExtractor::extract(Some(&bytes), "missing", "()V", &BTreeSet::new()).unwrap(),
        LocalCaptureResult::Failure {
            error: LocalCaptureError::MethodNotFound { .. }
        }
    ));
    assert!(
        matches!(LocalCaptureExtractor::extract(Some(&bytes),"broken","()V",&BTreeSet::from([-1])).unwrap(),LocalCaptureResult::Failure{error:LocalCaptureError::InvalidInstructionIndex{instruction_occurrence_index:-1,instruction_count}}if instruction_count>0)
    );
}
