use mcdev_core::{extras_model::*, mixin_source::*, McTextPosition, McTextRange};
fn range(l: usize, a: usize, r: usize, b: usize) -> McTextRange {
    McTextRange {
        start: McTextPosition {
            line: l,
            character: a,
        },
        end: McTextPosition {
            line: r,
            character: b,
        },
    }
}
fn resolved(handler_range: McTextRange) -> ResolvedMixinExtrasContext {
    ResolvedMixinExtrasContext {
        handler_range,
        context: ExpressionContext::default(),
    }
}
fn site(
    annotation_range: McTextRange,
    handler_range: Option<McTextRange>,
) -> MixinExtrasAnnotationSite {
    MixinExtrasAnnotationSite {
        annotation: MixinExtrasAnnotation::ModifyExpressionValue,
        method_attribute: "draw(Ljava/lang/String;FF)V".into(),
        at_value: Some("MIXINEXTRAS:EXPRESSION".into()),
        at_target: None,
        at_args: vec![],
        at_id: None,
        at_ordinal: None,
        at_shift: AtShiftSpec::Before,
        has_constant_selector: false,
        annotation_range,
        handler_method: handler_range.map(|range| HandlerMethodDeclaration {
            method_name: "mcdevHandler".into(),
            return_type_name: "int".into(),
            return_type_descriptor: Some("I".into()),
            range,
            ..Default::default()
        }),
        expand_zero_conditions: Default::default(),
    }
}
#[test]
fn selector_prefers_unique_annotation_containment() {
    let site = site(range(2, 4, 2, 20), Some(range(3, 4, 3, 40)));
    let a = resolved(range(2, 0, 4, 0));
    let b = resolved(range(3, 0, 4, 0));
    let contexts = [a.clone(), b];
    assert_eq!(
        select_resolved_mixin_extras_context(&site, &contexts),
        Some(&a)
    );
}
#[test]
fn selector_falls_back_to_unique_handler_overlap() {
    let site = site(range(1, 0, 1, 10), Some(range(3, 4, 3, 40)));
    let a = resolved(range(3, 0, 3, 50));
    let contexts = [a.clone()];
    assert_eq!(
        select_resolved_mixin_extras_context(&site, &contexts),
        Some(&a)
    );
}
#[test]
fn selector_returns_null_when_annotation_matches_are_ambiguous() {
    let site = site(range(2, 4, 2, 20), None);
    assert!(select_resolved_mixin_extras_context(
        &site,
        &[resolved(range(2, 0, 4, 0)), resolved(range(2, 2, 4, 2))]
    )
    .is_none());
}
#[test]
fn selector_returns_null_when_handler_overlap_is_ambiguous() {
    let site = site(range(0, 0, 0, 1), Some(range(3, 4, 3, 40)));
    assert!(select_resolved_mixin_extras_context(
        &site,
        &[resolved(range(3, 0, 3, 20)), resolved(range(3, 10, 3, 50))]
    )
    .is_none());
}
#[test]
fn retains_explicit_resolved_mixin_extras_contexts() {
    let resolved = resolved(range(1, 4, 1, 20));
    let model = MixinClassModel {
        resolved_mixin_extras_contexts: vec![resolved.clone()],
        ..Default::default()
    };
    assert_eq!(model.resolved_mixin_extras_contexts, vec![resolved]);
    let json = serde_json::to_string(&model).unwrap();
    assert_eq!(
        serde_json::from_str::<MixinClassModel>(&json).unwrap(),
        model
    );
}
#[test]
fn definition_source_and_id_ranges_keep_half_open_offsets() {
    let d = MixinExtrasDefinition {
        id: Some("field".into()),
        body_length: 31,
        ..Default::default()
    }
    .attach_id_content_range(Some(5..10))
    .with_id_source_range_from_handler_region(100, 20);
    assert_eq!(d.source_range, Some(120..151));
    assert_eq!(d.id_source_range, Some(125..130));
    let issue = DefinitionAnnotationParseIssue {
        attribute: "field".into(),
        raw_value: "invalid".into(),
        message: "invalid reference".into(),
        body_range: 13..20,
    };
    let d = d.with_parse_issues(vec![issue.clone()]);
    assert_eq!(d.parse_issues, vec![issue]);
    let index = MixinExtrasDefinitionIndex {
        definitions: vec![d.clone(), MixinExtrasDefinition::default()],
    };
    assert_eq!(index.definitions_with_id("field"), vec![&d]);
}
#[test]
fn expression_values_select_id_without_fallback() {
    let context = ExpressionContext {
        expression_index: MixinExtrasExpressionIndex {
            expressions: vec![
                MixinExtrasExpression {
                    values_utf16: None,
                    id: "".into(),
                    values: vec!["default".into()],
                },
                MixinExtrasExpression {
                    values_utf16: None,
                    id: "named".into(),
                    values: vec!["one".into(), "two".into()],
                },
            ],
        },
        definition_index: Default::default(),
    };
    assert_eq!(context.values_for_at_id(None), vec!["default"]);
    assert_eq!(context.values_for_at_id(Some("named")), vec!["one", "two"]);
    assert!(context.values_for_at_id(Some("missing")).is_empty());
}
