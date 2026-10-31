use super::*;
use mcdev_core::{bytecode::*, mixinextras_occurrence::InjectionPointOccurrenceResolver};

fn selector_host(kind: &str, text: &str, args: impl FnOnce(&str) -> Value) -> Host {
    let mut host = fixture(false);
    replace_type(
        &mut host,
        "@Inject(method=\"tick()V\", at=@At(\"HEAD\"))",
        text,
    );
    let reference = match kind {
        "ModifyConstant" => "modifyconstant",
        "WrapOperation" => "wrapoperation",
        _ => panic!(),
    };
    host.declarations[1]["annotations"] = json!([annotation(
        &host.source,
        text,
        kind,
        reference,
        args(&host.source)
    )]);
    host
}
fn constant_value(source: &str, text: &str, name: &str, value: Value) -> Value {
    json!({"kind":"annotation","annotation":annotation(source,text,"Constant","constant",json!([scalar(source,name,text.split_once('=').unwrap().1.trim_end_matches(')'),value)]))})
}
fn scoped(source: &str, scope: &str, name: &str, spelling: &str, mut value: Value) -> Value {
    let start = source.find(scope).unwrap() + scope.find(spelling).unwrap();
    if value["kind"] == "string" {
        value["contentRange"] = json!({"start":start+1,"end":start+spelling.len()-1});
    }
    json!({"name":name,"range":{"start":start,"end":start+spelling.len()},"value":found(value)})
}
#[test]
fn typed_constant_selectors_match_the_original_complete_site_outputs() {
    for (name, spelling, value) in [
        (
            "intValue",
            "1",
            json!({"kind":"integer","type":"int","value":1}),
        ),
        (
            "longValue",
            "9223372036854775807L",
            json!({"kind":"integer","type":"long","value":i64::MAX}),
        ),
        (
            "longValue",
            "2",
            json!({"kind":"integer","type":"int","value":2}),
        ),
        (
            "floatValue",
            "3",
            json!({"kind":"integer","type":"int","value":3}),
        ),
        (
            "doubleValue",
            "4",
            json!({"kind":"integer","type":"int","value":4}),
        ),
        ("stringValue", "\"hello\"", Value::Null),
        ("nullValue", "true", json!({"kind":"boolean","value":true})),
    ] {
        let constant = format!("@Constant({name}={spelling})");
        let text = format!("@ModifyConstant(method=\"tick()V\", constant={constant})");
        let host = selector_host("ModifyConstant", &text, |source| {
            json!([
                scalar(
                    source,
                    "method",
                    "\"tick()V\"",
                    string_value(source, "tick()V")
                ),
                scalar(
                    source,
                    "constant",
                    &constant,
                    constant_value(
                        source,
                        &constant,
                        name,
                        if value.is_null() {
                            string_value(source, "hello")
                        } else {
                            value
                        }
                    )
                ),
            ])
        });
        let facts = parse(&host, false).model.normalized_handler_source.unwrap();
        let mut actual = facts.checked_sites(&host.source).unwrap().to_vec();
        let mut expected =
            HandlerSignatureService::find_sugar_handler_annotation_sites(&host.source);
        for site in actual.iter_mut().chain(expected.iter_mut()) {
            site.handler_method = None;
        }
        assert_eq!(actual, expected, "{name}");
    }
}
#[test]
fn typed_expand_zero_conditions_keep_enum_identity_and_ordinal() {
    let c =
        "@Constant(intValue=0, ordinal=1, expandZeroConditions=Constant.Condition.LESS_THAN_ZERO)";
    let text = format!("@ModifyConstant(method=\"tick()V\", constant={c})");
    let host = selector_host("ModifyConstant", &text, |source| {
        json!([
            scalar(
                source,
                "method",
                "\"tick()V\"",
                string_value(source, "tick()V")
            ),
            scalar(
                source,
                "constant",
                c,
                json!({"kind":"annotation","annotation":annotation(source,c,"Constant","constant",json!([
                    scalar(source,"intValue","0",json!({"kind":"integer","type":"int","value":0})),
                    scalar(source,"ordinal","1",json!({"kind":"integer","type":"int","value":1})),
                    scalar(source,"expandZeroConditions","Constant.Condition.LESS_THAN_ZERO",json!({"kind":"enum","typeRef":"condition","name":"LESS_THAN_ZERO"})),
                ]))})
            ),
        ])
    });
    let facts = parse(&host, false).model.normalized_handler_source.unwrap();
    let mut actual = facts.checked_sites(&host.source).unwrap().to_vec();
    let mut expected = HandlerSignatureService::find_sugar_handler_annotation_sites(&host.source);
    for site in actual.iter_mut().chain(expected.iter_mut()) {
        site.handler_method = None
    }
    assert_eq!(actual, expected);
}
#[test]
fn wrap_operation_constant_presence_reaches_the_original_selector_model() {
    let c = "@Constant(intValue=1)";
    let text = format!("@WrapOperation(method=\"tick()V\", constant={c})");
    let host = selector_host("WrapOperation", &text, |source| {
        json!([
            scalar(
                source,
                "method",
                "\"tick()V\"",
                string_value(source, "tick()V")
            ),
            scalar(
                source,
                "constant",
                c,
                constant_value(
                    source,
                    c,
                    "intValue",
                    json!({"kind":"integer","type":"int","value":1})
                )
            ),
        ])
    });
    let facts = parse(&host, false).model.normalized_handler_source.unwrap();
    let mut actual = facts.checked_sites(&host.source).unwrap().to_vec();
    let mut expected = HandlerSignatureService::find_sugar_handler_annotation_sites(&host.source);
    for site in actual.iter_mut().chain(expected.iter_mut()) {
        site.handler_method = None
    }
    assert_eq!(actual, expected);
    assert!(actual[0].has_constant_selector);
}
struct Code;
impl BytecodeIndex for Code {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        match value {
            "CONSTANT" => [1, 4, 7]
                .into_iter()
                .map(|index| {
                    let mut candidate =
                        AtTargetCandidate::new(AtTargetKind::Constant, "fixture/Target", "", "I");
                    candidate.instruction_occurrence_index = index;
                    candidate.constant_value = Some(ConstantValue::IntValue(1));
                    candidate
                })
                .collect(),
            "INVOKE" => {
                let mut candidate =
                    AtTargetCandidate::new(AtTargetKind::Invoke, "fixture/Target", "start", "()V");
                candidate.instruction_occurrence_index = 3;
                vec![candidate]
            }
            "RETURN" => {
                let mut candidate =
                    AtTargetCandidate::new(AtTargetKind::Return, "fixture/Target", "", "V");
                candidate.instruction_occurrence_index = 6;
                vec![candidate]
            }
            other => panic!("unexpected selection {other}"),
        }
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!()
    }
    fn get_class_bytes(&self, _: &str) -> Option<Vec<u8>> {
        panic!("selector projection grants no Code authority")
    }
}
#[test]
fn typed_slice_boundaries_filter_constant_candidates_before_ordinal_selection() {
    let from =
        "@At(value=\"INVOKE\", target=\"Lfixture/Target;start()V\", ordinal=0, id=\"entry\")";
    let to = "@At(\"RETURN\")";
    let slice = format!("@Slice(id=\"scope\", from={from}, to={to})");
    let c = "@Constant(intValue=1, ordinal=0, slice=\"scope\")";
    let text = format!("@ModifyConstant(method=\"tick()V\", constant={c}, slice={slice})");
    let host = selector_host("ModifyConstant", &text, |source| {
        let from = annotation(
            source,
            from,
            "At",
            "at",
            json!([
                scoped(
                    source,
                    from,
                    "value",
                    "\"INVOKE\"",
                    string_value(source, "INVOKE")
                ),
                scoped(
                    source,
                    from,
                    "target",
                    "\"Lfixture/Target;start()V\"",
                    string_value(source, "Lfixture/Target;start()V")
                ),
                scoped(
                    source,
                    from,
                    "ordinal",
                    "0",
                    json!({"kind":"integer","type":"int","value":0})
                ),
                scoped(
                    source,
                    from,
                    "id",
                    "\"entry\"",
                    string_value(source, "entry")
                ),
            ]),
        );
        let to = annotation(
            source,
            to,
            "At",
            "at",
            json!([scalar(
                source,
                "value",
                "\"RETURN\"",
                string_value(source, "RETURN")
            )]),
        );
        json!([
            scalar(
                source,
                "method",
                "\"tick()V\"",
                string_value(source, "tick()V")
            ),
            scalar(
                source,
                "constant",
                c,
                json!({"kind":"annotation","annotation":annotation(source,c,"Constant","constant",json!([
                    scoped(source,c,"intValue","1",json!({"kind":"integer","type":"int","value":1})),
                    scoped(source,c,"ordinal","0",json!({"kind":"integer","type":"int","value":0})),
                    scoped(source,c,"slice","\"scope\"",string_value(source,"scope")),
                ]))})
            ),
            scalar(
                source,
                "slice",
                &slice,
                json!({"kind":"annotation","annotation":annotation(source,&slice,"Slice","slice",json!([
                    scoped(source,&slice,"id","\"scope\"",string_value(source,"scope")),
                    scalar(source,"from",&host_text(&from,source),json!({"kind":"annotation","annotation":from})),
                    scalar(source,"to","@At(\"RETURN\")",json!({"kind":"annotation","annotation":to})),
                ]))})
            ),
        ])
    });
    let model = parse(&host, false).model;
    assert!(
        model.injectors[0].at_selectors.is_empty(),
        "slice boundaries are not top-level injection points"
    );
    let facts = model.normalized_handler_source.unwrap();
    let sites = facts.checked_sites(&host.source).unwrap();
    let selection = &facts.slice_selections[0];
    assert_eq!(selection.slice.id, "scope");
    assert_eq!(
        selection.slice.from.as_ref().unwrap().id.as_deref(),
        Some("entry")
    );
    let method = Index.get_methods("fixture/Target").remove(0);
    assert_eq!(
        InjectionPointOccurrenceResolver::new(&Code)
            .resolve_in_slice(
                "fixture/Target",
                &method,
                &sites[0],
                None,
                Some(&selection.slice)
            )
            .unwrap(),
        [4].into()
    );
}
fn host_text(annotation: &Value, source: &str) -> String {
    let range = &annotation["range"];
    source[range["start"].as_u64().unwrap() as usize..range["end"].as_u64().unwrap() as usize]
        .into()
}
#[test]
fn unknown_float_constant_facts_do_not_turn_into_empty_or_integer_selectors() {
    let c = "@Constant(floatValue=1.0F)";
    let text = format!("@ModifyConstant(method=\"tick()V\", constant={c})");
    let host = selector_host("ModifyConstant", &text, |source| {
        let mut c = annotation(source, c, "Constant", "constant", json!([]));
        c["arguments"] =
            json!([{"name":"floatValue","range":span(source,"1.0F"),"value":unknown()}]);
        json!([
            scalar(
                source,
                "method",
                "\"tick()V\"",
                string_value(source, "tick()V")
            ),
            scalar(
                source,
                "constant",
                "@Constant(floatValue=1.0F)",
                json!({"kind":"annotation","annotation":c})
            )
        ])
    });
    assert!(matches!(answer(&host, false), HostFact::Unavailable { .. }));
}
