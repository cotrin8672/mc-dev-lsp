use mcdev_companion::{
    host_facts::{bridge::FactReader, inputs::*, FactsError},
    wire::{Declaration, Evaluation, Fact, Registry},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"IncompleteSource"}]})
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn span(source: &str, text: &str) -> Value {
    let start = source.find(text).unwrap();
    json!({"start":start,"end":start+text.len()})
}
fn annotation(source: &str, text: &str, name: &str, reference: &str, args: Value) -> Value {
    let start = source.find(text).unwrap() + text.find(name).unwrap();
    json!({"range":span(source,text),"nameRange":{"start":start,"end":start+name.len()},
        "typeRef":reference,"useSiteTarget":null,"arguments":args})
}
fn fixture() -> (String, Vec<Declaration>, Value) {
    let text = "//😀猫\r\n@Mixin class Patch { @Outer(value=@Inner) void hook(@Local Object arg) {} @Inject(method=\"is\") }".to_owned();
    let mixin = annotation(&text, "@Mixin", "Mixin", "ref-mixin", json!([]));
    let inner = annotation(&text, "@Inner", "Inner", "ref-inner", json!([]));
    let outer = annotation(
        &text,
        "@Outer(value=@Inner)",
        "Outer",
        "ref-outer",
        json!([
            {"name":"value","range":span(&text,"@Inner"),"value":found(json!({"kind":"annotation","annotation":inner}))}
        ]),
    );
    let local = annotation(&text, "@Local", "Local", "ref-local", json!([]));
    let orphan = annotation(
        &text,
        "@Inject(method=\"is\")",
        "Inject",
        "ref-inject",
        json!([
            {"name":"method","range":span(&text,"\"is\""),"value":found(json!({"kind":"string","value":"is","contentRange":span(&text,"is")}))}
        ]),
    );
    let declarations = serde_json::from_value(json!([
        {"id":"class","owner":null,"range":span(&text,"Patch"),"name":"Patch","annotations":[mixin]},
        {"id":"method","owner":"class","range":span(&text,"hook"),"name":"hook","annotations":[outer],
          "parameters":found(json!([{"name":"arg","range":span(&text,"@Local Object arg"),"nameRange":span(&text,"arg"),
            "typeRef":"ref-object","annotations":[local]}]))}
    ])).unwrap();
    let mut owned_mixin = mixin;
    owned_mixin["owner"] = json!("class");
    let mut owned_outer = outer;
    owned_outer["owner"] = json!("method");
    let mut nested = inner;
    nested["owner"] = Value::Null;
    let mut parameter = local;
    parameter["owner"] = Value::Null;
    let mut orphan = orphan;
    orphan["owner"] = Value::Null;
    let wire = json!({"source":{"documentUri":found(json!("file:///project/Patch.java")),
        "identity":{"kind":"source","revision":"opaque-source-generation"},"sha256":digest(&text)},
        "annotations":[owned_mixin,owned_outer,nested,parameter,orphan],"coverage":unknown()});
    (text, declarations, wire)
}
fn registry(text: &str, declarations: &[Declaration]) -> Registry {
    let evaluation: Evaluation = serde_json::from_value(
        json!({"evaluation":"evaluation","owner":"owner","source":text,
        "configuration":{},"declarations":declarations}),
    )
    .unwrap();
    let mut registry = Registry::default();
    evaluation.validate(&mut registry).unwrap();
    registry
}
fn validate(wire: Value, text: &str, declarations: &[Declaration]) -> Result<usize, FactsError> {
    let sites: AnnotationSites = serde_json::from_value(wire).unwrap();
    sites
        .validate(
            text,
            "file:///project/Patch.java",
            None,
            declarations,
            true,
            &mut registry(text, declarations),
        )
        .map(|sites| sites.len())
}
#[derive(Default)]
struct Reader {
    replies: VecDeque<Result<Value, FactsError>>,
    queries: Vec<Value>,
}
impl FactReader for Reader {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        assert_eq!(evaluation, "evaluation");
        self.queries.extend_from_slice(queries);
        self.replies.pop_front().expect("unexpected query")
    }
}
fn ops() -> InputOps {
    InputOps::intersect(&json!({"capabilities":["source-facts-v2","source-snapshots-v1"],
        "supportedOps":{"source-facts-v2":["annotationSites"],"source-snapshots-v1":["sourceInventory"]}})).unwrap()
}

#[test]
fn annotation_sites_are_independent_optional_operations() {
    for host in [
        json!({"capabilities":["source-facts-v2"]}),
        json!({"capabilities":[],"supportedOps":{"source-facts-v2":["annotationSites"]}}),
        json!({"capabilities":["source-facts-v2"],"supportedOps":{"source-facts-v2":["sourceFacts"]}}),
    ] {
        let ops = InputOps::intersect(&host).unwrap();
        assert!(!ops.supports(InputOp::AnnotationSites));
        let mut reader = Reader::default();
        assert!(matches!(
            ScopedInputs::new("evaluation", &ops, &mut reader)
                .unwrap()
                .annotation_sites(None),
            Err(FactsError::Unsupported(_))
        ));
        assert!(reader.queries.is_empty());
    }
    assert!(ops().supports(InputOp::AnnotationSites));
    assert!(!ops().supports(InputOp::SourceFacts));
    assert_eq!(ops().accepted()["source-facts-v2"], vec!["annotationSites"]);
}

#[test]
fn unknown_coverage_keeps_positive_sites_and_reconciles_owned_parameter_and_nested_sites() {
    let (text, declarations, wire) = fixture();
    let mut reader = Reader {
        replies: vec![Ok(json!({"facts":[wire]}))].into(),
        ..Default::default()
    };
    let result = ScopedInputs::new("evaluation", &ops(), &mut reader)
        .unwrap()
        .annotation_sites(None)
        .unwrap();
    assert!(matches!(result.coverage, Fact::Unknown { .. }));
    let only = result
        .validate(
            &text,
            "file:///project/Patch.java",
            None,
            &declarations,
            true,
            &mut registry(&text, &declarations),
        )
        .unwrap();
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].type_ref, "ref-inject");
    assert_eq!(result.source.identity.revision, "opaque-source-generation");
    assert_eq!(reader.queries, vec![json!({"op":"annotationSites"})]);
}

#[test]
fn duplicate_ownership_payload_and_range_contradictions_are_rejected() {
    let (text, declarations, wire) = fixture();
    let mut variants = Vec::new();
    let mut duplicate = wire.clone();
    duplicate["annotations"]
        .as_array_mut()
        .unwrap()
        .push(wire["annotations"][4].clone());
    variants.push(duplicate);
    let mut owner = wire.clone();
    owner["annotations"][0]["owner"] = Value::Null;
    variants.push(owner);
    let mut parameter_owner = wire.clone();
    parameter_owner["annotations"][3]["owner"] = json!("method");
    variants.push(parameter_owner);
    let mut foreign_owner = wire.clone();
    foreign_owner["annotations"][4]["owner"] = json!("other-evaluation-declaration");
    variants.push(foreign_owner);
    let mut payload = wire.clone();
    payload["annotations"][2]["typeRef"] = json!("different-ref");
    variants.push(payload);
    let mut overlap = wire.clone();
    overlap["annotations"][4]["range"] =
        json!({"start":wire["annotations"][3]["range"]["start"],"end":text.len()});
    variants.push(overlap);
    for variant in variants {
        assert!(matches!(
            validate(variant, &text, &declarations),
            Err(FactsError::Protocol(_))
        ));
    }
}

#[test]
fn complete_inventory_must_include_known_sites_and_unknown_uri_never_proves_binding() {
    let (text, declarations, mut wire) = fixture();
    wire["coverage"] = found(Value::Null);
    assert_eq!(validate(wire.clone(), &text, &declarations).unwrap(), 1);
    wire["annotations"].as_array_mut().unwrap().remove(2);
    assert!(matches!(
        validate(wire, &text, &declarations),
        Err(FactsError::Protocol(_))
    ));
    let (_, _, mut wire) = fixture();
    wire["source"]["documentUri"] = unknown();
    let sites: AnnotationSites = serde_json::from_value(wire).unwrap();
    assert!(matches!(sites.source.document_uri, Fact::Unknown { .. }));
    assert!(matches!(
        sites.validate(
            &text,
            "file:///project/Patch.java",
            None,
            &declarations,
            true,
            &mut registry(&text, &declarations)
        ),
        Err(FactsError::Unknown(_))
    ));
}

#[test]
fn malformed_dto_unicode_ranges_and_false_source_footprints_fail() {
    let (text, declarations, wire) = fixture();
    let mut missing = wire.clone();
    missing["annotations"][4]
        .as_object_mut()
        .unwrap()
        .remove("owner");
    assert!(serde_json::from_value::<AnnotationSites>(missing).is_err());
    let mut variants = Vec::new();
    let mut split = wire.clone();
    split["annotations"][4]["nameRange"] = json!({"start":3,"end":4});
    variants.push(split);
    let mut bounds = wire.clone();
    bounds["annotations"][4]["range"]["end"] = json!(text.len() + 1);
    variants.push(bounds);
    let mut bad = wire.clone();
    bad["annotations"][4]["arguments"][0]["value"] = json!({"state":"unknown","reasons":[]});
    variants.push(bad);
    let mut absent = wire.clone();
    absent["coverage"] = json!({"state":"definitelyAbsent"});
    variants.push(absent);
    for variant in variants {
        assert!(matches!(
            validate(variant, &text, &declarations),
            Err(FactsError::Protocol(_))
        ));
    }
    let mut stale = wire.clone();
    stale["source"]["sha256"] = json!("0".repeat(64));
    assert!(matches!(
        validate(stale, &text, &declarations),
        Err(FactsError::Stale(_))
    ));
    let mut wrong_uri = wire;
    wrong_uri["source"]["documentUri"] = found(json!("file:///project/Foreign.java"));
    assert!(matches!(
        validate(wrong_uri, &text, &declarations),
        Err(FactsError::Stale(_))
    ));
}

#[test]
fn foreign_tokens_and_snapshots_remain_reader_and_evaluation_scoped() {
    let (text, declarations, wire) = fixture();
    let descriptor = json!({"id":"source:foreign","displayPath":"/project/Patch.java","documentUri":wire["source"]["documentUri"],
        "languageId":"java","identity":wire["source"]["identity"],"documentVersion":3});
    let mut reader=Reader { replies:vec![Ok(json!({"facts":[{"sources":[descriptor],"next":null,"coverage":found(Value::Null)}]})),
        Ok(json!({"facts":[wire.clone()]}))].into(),..Default::default() };
    let mut inputs = ScopedInputs::new("evaluation", &ops(), &mut reader).unwrap();
    let entry = inputs.source_inventory().unwrap().entries.remove(0);
    let sites = inputs.annotation_sites(Some(&entry)).unwrap();
    assert_eq!(
        sites
            .validate(
                &text,
                "file:///project/Patch.java",
                Some(&entry.descriptor),
                &declarations,
                true,
                &mut registry(&text, &declarations)
            )
            .unwrap()
            .len(),
        1
    );
    let mut forged = entry.clone();
    forged.descriptor.id = "source:forged".into();
    assert!(matches!(
        inputs.annotation_sites(Some(&forged)),
        Err(FactsError::Protocol(_))
    ));
    let mut other = ScopedInputs::new("evaluation", &ops(), Reader::default()).unwrap();
    assert!(matches!(
        other.annotation_sites(Some(&entry)),
        Err(FactsError::Protocol(_))
    ));
    drop(inputs);
    assert_eq!(
        reader.queries[1],
        json!({"op":"annotationSites","source":"source:foreign"})
    );
    let mut wrong = entry.descriptor.clone();
    wrong.identity.revision = "other-generation".into();
    assert!(matches!(
        sites.validate(
            &text,
            "file:///project/Patch.java",
            Some(&wrong),
            &declarations,
            true,
            &mut registry(&text, &declarations)
        ),
        Err(FactsError::Stale(_))
    ));
}

#[test]
fn stale_and_cancellation_are_not_converted_to_empty_site_inventories() {
    for error in [
        FactsError::Stale("changed".into()),
        FactsError::Cancelled("cancelled".into()),
    ] {
        let mut reader = Reader {
            replies: vec![Err(error.clone())].into(),
            ..Default::default()
        };
        let result = ScopedInputs::new("evaluation", &ops(), &mut reader)
            .unwrap()
            .annotation_sites(None);
        match error {
            FactsError::Stale(_) => assert!(matches!(result, Err(FactsError::Stale(_)))),
            FactsError::Cancelled(_) => assert!(matches!(result, Err(FactsError::Cancelled(_)))),
            _ => unreachable!(),
        }
    }
}

#[test]
fn reference_and_type_id_aliases_match_only_positive_same_view_identity_and_provenance() {
    use mcdev_companion::wire::TypeFact;
    use std::collections::BTreeMap;
    let (text, declarations, mut wire) = fixture();
    wire["annotations"][1]["typeRef"] = json!("alias-outer");
    wire["annotations"][1]["arguments"][0]["value"]["value"]["annotation"]["typeRef"] =
        json!("alias-inner");
    wire["annotations"][2]["typeRef"] = json!("alias-inner");
    let sites: AnnotationSites = serde_json::from_value(wire).unwrap();
    let mut types = BTreeMap::new();
    for (reference, fqn) in [
        ("ref-outer", "demo.Outer"),
        ("alias-outer", "demo.Outer"),
        ("ref-inner", "demo.Inner"),
        ("alias-inner", "demo.Inner"),
    ] {
        let ty: TypeFact = serde_json::from_value(
            json!({"id":format!("fresh-type-{reference}"),"qualifiedName":fqn}),
        )
        .unwrap();
        types.insert(reference.to_owned(), Fact::Found { value: ty });
    }
    let check = |types: &BTreeMap<String, Fact<TypeFact>>| {
        sites
            .validate_with_types(
                &text,
                "file:///project/Patch.java",
                None,
                &declarations,
                true,
                &mut registry(&text, &declarations),
                types,
            )
            .map(|sites| sites.len())
    };
    assert_eq!(check(&types).unwrap(), 1);
    let mut unresolved = types.clone();
    unresolved.insert(
        "alias-inner".into(),
        serde_json::from_value(unknown()).unwrap(),
    );
    assert!(matches!(check(&unresolved), Err(FactsError::Unknown(_))));
    let mut mismatch = types.clone();
    if let Fact::Found { value } = mismatch.get_mut("alias-inner").unwrap() {
        value.qualified_name = "other.Inner".into();
    }
    assert!(matches!(check(&mismatch), Err(FactsError::Protocol(_))));
    let context:SourceDescriptor=serde_json::from_value(json!({"id":"source:one","displayPath":"/project/Patch.java",
        "documentUri":found(json!("file:///project/Patch.java")),"languageId":"java","identity":{"kind":"source","revision":"opaque-source-generation"}})).unwrap();
    for fact in types.values_mut() {
        if let Fact::Found { value } = fact {
            value.read_only = true;
            value.context_source = Some(context.clone());
        }
    }
    if let Fact::Found { value } = types.get_mut("alias-inner").unwrap() {
        value.context_source.as_mut().unwrap().id = "source:alias".into();
    }
    assert_eq!(check(&types).unwrap(), 1);
    if let Fact::Found { value } = types.get_mut("alias-inner").unwrap() {
        value.context_source.as_mut().unwrap().identity.revision = "other-generation".into();
    }
    assert!(matches!(check(&types), Err(FactsError::Protocol(_))));
}

#[test]
fn unknown_parameter_metadata_does_not_promote_its_sites_to_unowned_injectors() {
    let (text, mut declarations, wire) = fixture();
    declarations[1].source_kind = Some("Function".into());
    declarations[1].header_range = Some(
        serde_json::from_value(span(
            &text,
            "@Outer(value=@Inner) void hook(@Local Object arg)",
        ))
        .unwrap(),
    );
    declarations[1].parameters = Some(serde_json::from_value(unknown()).unwrap());
    assert_eq!(validate(wire, &text, &declarations).unwrap(), 1);
}

#[test]
fn annotation_source_uri_binding_decodes_drive_colon_spaces_and_japanese_without_filesystem_io() {
    let (text, declarations, wire) = fixture();
    for (actual, client) in [
        (
            "file:///C%3A/unsaved%20folder/%E9%9B%AA.java",
            "file:///C:/unsaved%20folder/%E9%9B%AA.java",
        ),
        (
            "file:///tmp/unsaved%20folder/雪.java",
            "file:///tmp/unsaved folder/%E9%9B%AA.java",
        ),
    ] {
        let mut wire = wire.clone();
        wire["source"]["documentUri"] = found(json!(actual));
        let sites: AnnotationSites = serde_json::from_value(wire).unwrap();
        let expected:SourceDescriptor=serde_json::from_value(json!({"id":"issued-source","displayPath":"/unused/unsaved.java",
            "documentUri":found(json!(client)),"languageId":"java","identity":sites.source.identity})).unwrap();
        assert_eq!(
            sites
                .validate(
                    &text,
                    client,
                    Some(&expected),
                    &declarations,
                    true,
                    &mut registry(&text, &declarations)
                )
                .unwrap()
                .len(),
            1
        );
        let mut other = expected.clone();
        other.identity.revision = "other-snapshot".into();
        assert!(matches!(
            sites.validate(
                &text,
                client,
                Some(&other),
                &declarations,
                true,
                &mut registry(&text, &declarations)
            ),
            Err(FactsError::Stale(_))
        ));
        assert!(matches!(
            sites.validate(
                &text,
                "file:///different/雪.java",
                None,
                &declarations,
                true,
                &mut registry(&text, &declarations)
            ),
            Err(FactsError::Stale(_))
        ));
        for invalid in [
            "file:///tmp/%FF.java",
            "file:///tmp/%GG.java",
            "file:///tmp/%00.java",
            "file:///tmp/a.java?query",
            "https://example/a.java",
        ] {
            assert!(matches!(
                sites.validate(
                    &text,
                    invalid,
                    None,
                    &declarations,
                    true,
                    &mut registry(&text, &declarations)
                ),
                Err(FactsError::Protocol(_))
            ));
        }
    }
    let mut literal_percent = wire;
    literal_percent["source"]["documentUri"] =
        found(json!("file:///tmp/unsaved%2520folder/雪.java"));
    let sites: AnnotationSites = serde_json::from_value(literal_percent).unwrap();
    assert!(matches!(
        sites.validate(
            &text,
            "file:///tmp/unsaved%20folder/雪.java",
            None,
            &declarations,
            true,
            &mut registry(&text, &declarations)
        ),
        Err(FactsError::Stale(_))
    ));
}

#[test]
fn actual_serializer93_keeps_source_ranges_canonical_owner_and_unmeasured_alias_unknown() {
    use mcdev_companion::wire::{validate_declarations, HandleKind, TypeFact};
    use std::collections::BTreeMap;
    const RAW: &[u8] =
        include_bytes!("fixtures/host_inputs/annotation_sites/actual-annotation-sites93.json");
    assert_eq!(RAW.len(), 8601);
    assert_eq!(
        format!("{:x}", Sha256::digest(RAW)),
        "fa2007364b01b5cfc877085e0c7297cc0513bfcdaa17f3243ee5564a7183eb1a"
    );
    let sample: Value = serde_json::from_slice(RAW).unwrap();
    let texts: Value = serde_json::from_slice(include_bytes!(
        "fixtures/host_inputs/annotation_sites/editor-source-texts93.json"
    ))
    .unwrap();
    assert_eq!(
        sample["provenance"]["hostSha256"],
        "46ae70429778a834951ce0c191b08b334863441a82bf20c3394f8075fabfed4e"
    );
    assert_eq!(
        sample["provenance"]["proofSha256"],
        "adc9d13314749b903c370c2564b8ffe8fd00ffcaa2569b53f76bcdf9338a42a2"
    );
    for (phase, expected_bytes) in [("initial", 88), ("edited", 118)] {
        let observed = &sample[phase];
        let text = texts[phase]["source"].as_str().unwrap();
        assert_eq!(text.len(), expected_bytes);
        let client_uri = texts[phase]["documentUri"].as_str().unwrap();
        let sites: AnnotationSites = serde_json::from_value(observed["inventory"].clone()).unwrap();
        assert_eq!(sites.source.sha256, digest(text));
        assert_eq!(
            sites.source.sha256,
            texts[phase]["sha256"].as_str().unwrap()
        );
        assert_ne!(sites.source.document_uri.found().unwrap(), client_uri);
        let declarations: Vec<Declaration> =
            serde_json::from_value(observed["declarations"].clone()).unwrap();
        let mut registry = Registry::default();
        validate_declarations(&declarations, text, &mut registry, true).unwrap();
        let resolved: Fact<TypeFact> =
            serde_json::from_value(observed["resolved"].clone()).unwrap();
        resolved.validate().unwrap();
        let ty = resolved.found().unwrap();
        assert_eq!(ty.qualified_name, "refs.Marker");
        assert!(!ty.read_only && ty.context_source.is_none());
        assert!(
            mcdev_core::mixin_source::MixinAnnotation::from_official_fqn(&ty.qualified_name)
                .is_none(),
            "generic producer fixture grants no MC injector identity"
        );
        registry.register(&ty.id, HandleKind::Type).unwrap();
        let site = &sites.annotations[0];
        assert_eq!(
            site.annotation.range.text(text).unwrap(),
            "@Marker(value=\"is\")"
        );
        assert_eq!(
            site.annotation
                .name_range
                .as_ref()
                .unwrap()
                .text(text)
                .unwrap(),
            "Marker"
        );
        let types = BTreeMap::from([(site.annotation.type_ref.clone(), resolved)]);
        let result = sites.validate_with_types(
            text,
            client_uri,
            None,
            &declarations,
            true,
            &mut registry,
            &types,
        );
        if phase == "initial" {
            assert!(site.owner.is_none());
            assert!(matches!(sites.coverage, Fact::Unknown { .. }));
            assert_eq!(result.unwrap().len(), 1);
            assert_eq!(sites.source.identity.revision, "32");
            assert_eq!(texts[phase]["documentVersion"], 3);
            assert_eq!(sample["beforeSha256"], sites.source.sha256);
        } else {
            let owner = declarations
                .iter()
                .find(|declaration| Some(&declaration.id) == site.owner.as_ref())
                .unwrap();
            assert_eq!(owner.name.as_deref(), Some("injected"));
            assert_eq!(owner.id, "6:1");
            assert_eq!(owner.annotations[0].range, site.annotation.range);
            assert_eq!(owner.annotations[0].name_range, site.annotation.name_range);
            assert_eq!(owner.annotations[0].arguments, site.annotation.arguments);
            assert_eq!(site.annotation.type_ref, "6:3");
            assert_eq!(owner.annotations[0].type_ref, "6:4");
            assert!(
                !types.contains_key("6:4"),
                "do not invent a second actual resolver response"
            );
            assert!(matches!(result, Err(FactsError::Unknown(_))));
            assert!(matches!(sites.coverage, Fact::Found { value: () }));
            assert_eq!(sites.source.identity.revision, "50");
            assert_eq!(texts[phase]["documentVersion"], 4);
            assert_eq!(sample["afterSha256"], sites.source.sha256);
        }
    }
    let stale: Value = serde_json::from_str(sample["stale"]["error"].as_str().unwrap()).unwrap();
    assert_eq!(stale["code"], -32801);
    assert_eq!(
        sample["stale"]["evaluation"],
        sample["initial"]["evaluation"]
    );
    assert_ne!(
        sample["initial"]["evaluation"],
        sample["edited"]["evaluation"]
    );
    assert_eq!(
        sample["provenance"]["accepted"]["evaluation"],
        sample["edited"]["evaluation"]
    );
    assert_eq!(
        sample["provenance"]["accepted"]["source_sha256"],
        sample["afterSha256"]
    );
}

#[test]
fn nonpositive_alias_diagnostic_distinguishes_lookup_and_fact_states_without_dumping_payloads() {
    use mcdev_companion::wire::TypeFact;
    use std::collections::BTreeMap;
    let (text, declarations, mut wire) = fixture();
    wire["annotations"][1]["typeRef"] = json!("alias-outer");
    let sites: AnnotationSites = serde_json::from_value(wire).unwrap();
    let known: TypeFact =
        serde_json::from_value(json!({"id":"known-type","qualifiedName":"demo.Outer"})).unwrap();
    let reasons=(0..64).map(|index|json!({"code":format!("MissingFacts{index}"),"source":"DO_NOT_DUMP_SOURCE".repeat(1000)})).collect::<Vec<_>>();
    for (fact, state) in [
        (None, "notCollected"),
        (Some(Fact::DefinitelyAbsent), "definitelyAbsent"),
        (
            Some(Fact::Unknown {
                reasons: reasons.clone(),
            }),
            "unknown",
        ),
    ] {
        let mut types = BTreeMap::from([(
            "ref-outer".into(),
            Fact::Found {
                value: known.clone(),
            },
        )]);
        if let Some(fact) = fact {
            types.insert("alias-outer".into(), fact);
        }
        let error = sites
            .validate_with_types(
                &text,
                "file:///project/Patch.java",
                None,
                &declarations,
                true,
                &mut registry(&text, &declarations),
                &types,
            )
            .unwrap_err();
        let FactsError::Unknown(message) = error else {
            panic!("alias failure lost its typed Unknown")
        };
        assert!(
            message.starts_with("annotation reference alias has no positive same-view identity")
        );
        assert!(
            message.len() < 8192
                && !message.contains("DO_NOT_DUMP_SOURCE")
                && !message.contains(&text)
        );
        let diagnostic: Value =
            serde_json::from_str(message.split_once("; annotationAlias=").unwrap().1).unwrap();
        assert_eq!(diagnostic["known"]["reference"], "ref-outer");
        assert_eq!(diagnostic["known"]["state"], "found");
        assert_eq!(diagnostic["known"]["qualifiedName"], "demo.Outer");
        assert_eq!(diagnostic["site"]["reference"], "alias-outer");
        assert_eq!(diagnostic["site"]["state"], state);
        assert!(diagnostic["site"].get("qualifiedName").is_none());
        assert_eq!(diagnostic["field"]["kind"], "annotationType");
        assert_eq!(
            diagnostic["field"]["knownRange"],
            span(&text, "@Outer(value=@Inner)")
        );
        assert_eq!(diagnostic["annotation"]["knownOwner"], "method");
        assert_eq!(diagnostic["annotation"]["siteOwner"], "method");
        assert_eq!(diagnostic["annotation"]["sourceSha256"], digest(&text));
        if state == "unknown" {
            assert_eq!(diagnostic["site"]["reasonCodes"][0], "MissingFacts0");
            assert_eq!(diagnostic["site"]["reasonCount"], 64);
            assert_eq!(
                diagnostic["site"]["reasonCodes"].as_array().unwrap().len(),
                8
            );
            assert_eq!(diagnostic["site"]["reasonCodesTruncated"], true);
            assert_eq!(
                diagnostic["site"]["reasonPayloadSha256"]
                    .as_str()
                    .unwrap()
                    .len(),
                64
            );
        }
    }
}
