use mcdev_companion::{
    host_facts::{
        bridge::FactReader,
        draft::{BinaryAnnotationValue, RawAbi},
        inputs::*,
        FactsError,
    },
    wire::{Evaluation, Fact, HandleKind, Registry},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

#[derive(Default)]
struct Reader {
    responses: VecDeque<Result<Value, FactsError>>,
    queries: Vec<Vec<Value>>,
}
impl Reader {
    fn with(values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            responses: values.into_iter().map(Ok).collect(),
            queries: Vec::new(),
        }
    }
}
impl FactReader for Reader {
    fn read_facts(&mut self, _: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.queries.push(queries.to_vec());
        self.responses.pop_front().expect("unexpected host query")
    }
}
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"MissingSource"}]})
}
fn facts(values: Vec<Value>) -> Value {
    json!({"facts":values})
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn operations(project: bool) -> InputOps {
    let mut host = json!({"capabilities":["source-snapshots-v1","owned-resources-v1","source-facts-v2","source-locations-v1"],
        "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot"],
        "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"],
        "source-facts-v2":["sourceFacts"],"source-locations-v1":["sourceLocation"]}});
    if project {
        host["capabilities"]
            .as_array_mut()
            .unwrap()
            .push(json!("project-inputs-v1"));
        host["supportedOps"]["project-inputs-v1"] = json!(["projectInputs"]);
    }
    InputOps::intersect(&host).unwrap()
}
fn scope(name: &str) -> Value {
    json!({"module":"gradle::|:app","sourceSet":name,"id":format!("scope:{name}")})
}
fn source(id: &str) -> Value {
    json!({"id":id,"displayPath":"/project/Patch.java","documentUri":found(json!("file:///project/Patch.java")),
        "languageId":"java","identity":{"kind":"source","revision":"99"}})
}
fn project_source(id: &str) -> Value {
    let mut value = source(id);
    value["ownerScope"] = found(scope("main"));
    value["memberships"] = found(json!([scope("main"), scope("test")]));
    value["editable"] = found(json!(true));
    value
}
fn project_record() -> Value {
    json!({"kind":"project","id":"project:1","owningSourceSet":found(json!("scope:main")),
        "projectPath":":app","projectDirectory":"/project","buildIdentity":found(json!(":")),"capture":unknown(),"index":unknown()})
}
fn source_set(name: &str) -> Value {
    json!({"kind":"sourceSet","id":format!("scope:{name}"),"project":"project:1","name":name,
        "configurations":unknown(),"compileClasspathCoverage":found(Value::Null),"runtimeCoverage":unknown(),
        "compilerMetadataComplete":true,"unresolved":[],"counts":{"domain":"this-source-set-ordered-entries",
        "compileClasspath":0,"runtimeClasspath":null,"classOutputs":null}})
}
fn project_page(records: Vec<Value>, next: Option<&str>) -> Value {
    facts(vec![
        json!({"records":records,"next":next,"coverage":if next.is_some(){unknown()}else{
        found(json!({"domain":PROJECT_INPUTS_DOMAIN}))}}),
    ])
}

#[test]
fn new_queries_require_both_capability_and_operation() {
    for (capability, operation, op) in [
        ("source-facts-v2", "sourceFacts", InputOp::SourceFacts),
        (
            "source-locations-v1",
            "sourceLocation",
            InputOp::SourceLocation,
        ),
        ("project-inputs-v1", "projectInputs", InputOp::ProjectInputs),
    ] {
        for host in [
            json!({"capabilities":[capability]}),
            json!({"capabilities":[],"supportedOps":{capability:[operation]}}),
        ] {
            assert!(!InputOps::intersect(&host).unwrap().supports(op));
        }
        let ops = InputOps::intersect(
            &json!({"capabilities":[capability],"supportedOps":{capability:[operation,"future"]}}),
        )
        .unwrap();
        assert!(ops.supports(op));
        assert_eq!(ops.accepted()[capability], vec![operation]);
    }
    let mut reader = Reader::default();
    let mut inputs = ScopedInputs::new("evaluation:1", &InputOps::default(), &mut reader).unwrap();
    assert!(matches!(
        inputs.source_facts(None),
        Err(FactsError::Unsupported(_))
    ));
    assert!(matches!(
        inputs.source_location("type:1"),
        Err(FactsError::Unsupported(_))
    ));
    assert!(matches!(
        inputs.project_inputs(),
        Err(FactsError::Unsupported(_))
    ));
    assert!(reader.queries.is_empty());
}

#[test]
fn project_scopes_use_issued_ids_when_raw_module_differs_from_project_path() {
    let resource = json!({"id":"resource:test","relativePath":"main.mixins.json","association":"compileClasspath",
        "classpathOrder":0,"editable":false,"documentUri":unknown(),"documentVersion":null,
        "identity":{"kind":"resource","revision":"8"},"ownerScope":found(scope("main")),
        "membershipScope":{"module":"gradle::|:app","sourceSet":"test"}});
    let mut reader = Reader::with([
        project_page(
            vec![project_record(), source_set("main")],
            Some("project:next"),
        ),
        project_page(
            vec![
                source_set("test"),
                json!({"kind":"root","scope":"scope:main","role":"resource","order":0,"path":"/project/src/main/resources"}),
            ],
            None,
        ),
        facts(vec![
            json!({"sources":[project_source("source:test")],"next":null,"coverage":found(Value::Null)}),
        ]),
        facts(vec![
            json!({"resources":[resource.clone()],"next":null,"coverage":found(json!(true))}),
        ]),
        facts(vec![found(json!([resource.clone()]))]),
    ]);
    let ops = operations(true);
    assert!(!ops.accepted().contains_key("project-class-facts-v1"));
    let mut inputs = ScopedInputs::new("evaluation:1", &ops, &mut reader).unwrap();
    let project = inputs.project_inputs().unwrap();
    assert!(project.coverage.found().is_some());
    assert_eq!(project.source_sets.len(), 2);
    let project_path = project
        .records
        .iter()
        .find_map(|record| match record {
            ProjectInputRecord::Project(project) => Some(project.project_path.as_str()),
            _ => None,
        })
        .unwrap();
    assert_eq!(project_path, ":app");
    let test = &project.source_sets[1];
    let sources = inputs.source_inventory_in(test).unwrap();
    assert_eq!(
        sources.entries[0]
            .descriptor
            .memberships
            .as_ref()
            .unwrap()
            .found()
            .unwrap()[0]
            .module,
        "gradle::|:app"
    );
    assert_eq!(
        sources.entries[0]
            .descriptor
            .owner_scope
            .as_ref()
            .unwrap()
            .found()
            .unwrap()
            .source_set,
        "main"
    );
    let resources = inputs.resource_inventory_in(test).unwrap();
    assert_eq!(
        resources.entries[0]
            .descriptor
            .membership_scope
            .as_ref()
            .unwrap()
            .source_set,
        "test"
    );
    assert!(!resources.entries[0].descriptor.editable);
    assert!(inputs
        .resource_lookup_in("main.mixins.json", None, test)
        .unwrap()
        .found()
        .is_some());
    let mut foreign = ScopedInputs::new("evaluation:2", &ops, Reader::default()).unwrap();
    assert!(matches!(
        foreign.source_inventory_in(test),
        Err(FactsError::Protocol(_))
    ));
    let mut changed = test.clone();
    changed.descriptor.project = "project:foreign".into();
    assert!(matches!(
        inputs.resource_inventory_in(&changed),
        Err(FactsError::Protocol(_))
    ));
    drop(inputs);
    assert_eq!(reader.queries[1][0]["after"], "project:next");
    assert_eq!(reader.queries[2][0]["scope"], "scope:test");
    assert_eq!(reader.queries[3][0]["scope"], "scope:test");
    assert_eq!(reader.queries[4][0]["scope"], "scope:test");
}

#[test]
fn project_coverage_and_scope_references_cannot_be_fabricated() {
    let cases = [
        facts(vec![
            json!({"records":[project_record(),source_set("main")],"next":"next","coverage":found(json!({"domain":PROJECT_INPUTS_DOMAIN}))}),
        ]),
        project_page(
            vec![
                project_record(),
                source_set("main"),
                json!({"kind":"root","scope":"foreign","role":"source","order":0,"path":"/x"}),
            ],
            None,
        ),
        project_page(
            vec![project_record(), source_set("main"), source_set("main")],
            None,
        ),
        project_page(Vec::new(), None),
    ];
    for value in cases {
        let mut inputs = ScopedInputs::new("e", &operations(true), Reader::with([value])).unwrap();
        assert!(matches!(
            inputs.project_inputs(),
            Err(FactsError::Protocol(_))
        ));
    }
    let mut reader = Reader::with([facts(vec![
        json!({"records":[],"next":null,"coverage":unknown()}),
    ])]);
    let mut inputs = ScopedInputs::new("e", &operations(true), &mut reader).unwrap();
    let unknown = inputs.project_inputs().unwrap();
    assert!(unknown.source_sets.is_empty());
    assert!(matches!(unknown.coverage, Fact::Unknown { .. }));
}

#[test]
fn scoped_inventory_still_rejects_other_source_ids_and_resource_membership_names() {
    let mut wrong_source = project_source("source:test");
    wrong_source["memberships"] = found(json!([{
        "module":"gradle::|:app", "sourceSet":"test", "id":"scope:foreign"
    }]));
    let wrong_resource = json!({"id":"resource:test","relativePath":"config.json","association":"source",
        "classpathOrder":0,"editable":true,"documentUri":unknown(),"documentVersion":null,
        "identity":{"kind":"resource","revision":"8"},"ownerScope":found(scope("main")),
        "membershipScope":{"module":"gradle::|:app","sourceSet":"main"}});
    let mut inputs = ScopedInputs::new(
        "e",
        &operations(true),
        Reader::with([
            project_page(
                vec![project_record(), source_set("main"), source_set("test")],
                None,
            ),
            facts(vec![
                json!({"sources":[wrong_source],"next":null,"coverage":found(Value::Null)}),
            ]),
            facts(vec![
                json!({"resources":[wrong_resource],"next":null,"coverage":found(json!(true))}),
            ]),
        ]),
    )
    .unwrap();
    let project = inputs.project_inputs().unwrap();
    let test = &project.source_sets[1];
    assert!(matches!(
        inputs.source_inventory_in(test),
        Err(FactsError::Protocol(_))
    ));
    assert!(matches!(
        inputs.resource_inventory_in(test),
        Err(FactsError::Protocol(_))
    ));
}

#[test]
fn stale_project_input_request_error_and_cursor_cycles_are_preserved() {
    let mut reader = Reader::default();
    reader
        .responses
        .push_back(Err(FactsError::Stale("model changed".into())));
    let mut inputs = ScopedInputs::new("e", &operations(true), &mut reader).unwrap();
    assert!(matches!(inputs.project_inputs(), Err(FactsError::Stale(_))));
    let mut inputs = ScopedInputs::new(
        "e",
        &operations(true),
        Reader::with([
            project_page(vec![project_record()], Some("cursor")),
            project_page(vec![source_set("main")], Some("cursor")),
        ]),
    )
    .unwrap();
    assert!(matches!(
        inputs.project_inputs(),
        Err(FactsError::Protocol(_))
    ));
}

fn detailed_source() -> (String, Value) {
    let text = "@Outer(n=7L, at=@At(\"a\"+\"😀\"), e=Mode.A)\nclass Patch {}".to_owned();
    let range = |needle: &str| {
        let start = text.find(needle).unwrap();
        json!({"start":start,"end":start+needle.len()})
    };
    let literal_a = text.find("\"a\"").unwrap() + 1;
    let modifiers = json!({"visibility":unknown(),"static":found(json!(false)),"final":found(json!(false)),"abstract":found(json!(false))});
    let nested = json!({"typeRef":"ref:At","useSiteTarget":null,"range":range("@At(\"a\"+\"😀\")"),"nameRange":range("At"),
        "arguments":[{"name":null,"range":range("\"a\"+\"😀\""),"value":found(json!({"kind":"mappedString","value":"a😀","segments":[
            {"decoded":{"start":0,"end":1},"source":{"start":literal_a,"end":literal_a+1}},
            {"decoded":{"start":1,"end":3},"source":range("😀")}]}))}]});
    let declaration = json!({"id":"declaration:Patch","owner":null,"range":range("Patch"),"name":"Patch","sourceKind":"Class",
        "fullRange":{"start":0,"end":text.len()},"headerRange":{"start":0,"end":text.find('{').unwrap()},
        "nameRange":range("Patch"),"bodyRange":range("{}"),"modifiers":modifiers,
        "annotations":[{"typeRef":"ref:Outer","useSiteTarget":null,"range":{"start":0,"end":text.find('\n').unwrap()},"nameRange":range("Outer"),
            "arguments":[{"name":"n","range":range("7L"),"value":found(json!({"kind":"integer","type":"long","value":7}))},
                {"name":"at","range":range("@At(\"a\"+\"😀\")"),"value":found(json!({"kind":"annotation","annotation":nested}))},
                {"name":"e","range":range("Mode.A"),"value":found(json!({"kind":"enum","typeRef":"ref:Mode","name":"A"}))}]}]});
    (text, json!({"declarations":[declaration]}))
}

#[test]
fn source_facts_v2_retains_nested_integer_enum_and_utf16_scalar_mapping() {
    let (text, wire) = detailed_source();
    let mut reader = Reader::with([facts(vec![wire.clone()])]);
    let mut inputs = ScopedInputs::new("e", &operations(false), &mut reader).unwrap();
    let result = inputs.source_facts(None).unwrap();
    let mut registry = Registry::default();
    result.validate(&text, &mut registry).unwrap();
    registry.require("ref:Mode", HandleKind::TypeRef).unwrap();
    registry.require("ref:At", HandleKind::TypeRef).unwrap();
    assert_eq!(serde_json::to_value(&result).unwrap(), wire);
    assert_eq!(reader.queries, vec![vec![json!({"op":"sourceFacts"})]]);
    let legacy:Evaluation=serde_json::from_value(json!({"evaluation":"e","owner":"/project/Owner.java","source":"class Owner {}","configuration":{},
        "declarations":[{"id":"decl:Owner","owner":null,"range":{"start":6,"end":11},"name":"Owner","annotations":[]}]})).unwrap();
    legacy.validate(&mut Registry::default()).unwrap();
    assert!(legacy.declarations[0].parameters.is_none());
    assert!(serde_json::to_value(&legacy.declarations[0])
        .unwrap()
        .get("parameters")
        .is_none());
}

// db0f080 optional parameters: DTO admission tests do not claim live MC coverage.
fn parameter_document(kotlin: bool) -> (String, Value) {
    let first = if kotlin {
        "@Local(name = \"speed\") `when`: String"
    } else {
        "@Local(name = \"speed\") final String value"
    };
    let second = if kotlin {
        "`猫🙂`: Int"
    } else {
        "int count"
    };
    let method = if kotlin { "fun" } else { "void" };
    let text = format!("class Owner {{ {method} handler({first}, {second}) {{}} }}");
    let range = |needle: &str| {
        let start = text.find(needle).unwrap();
        json!({"start":start,"end":start+needle.len()})
    };
    let annotation = json!({"typeRef":"snapshot:annotation:Local","useSiteTarget":null,
        "range":range("@Local(name = \"speed\")"),"nameRange":range("Local"),
        "arguments":[{"name":"name","range":range("\"speed\""),
            "value":found(json!({"kind":"string","value":"speed","contentRange":range("speed")}))}]});
    let body_start = text.find("{}").unwrap();
    let start = text.find(&format!("{method} handler")).unwrap();
    let declaration = json!({"id":"draft:handler","owner":null,"range":range("handler"),"name":"handler","annotations":[],
    "sourceKind":"Function","fullRange":{"start":start,"end":body_start+2},
    "headerRange":{"start":start,"end":body_start},"nameRange":range("handler"),
    "bodyRange":{"start":body_start,"end":body_start+2},
    "modifiers":{"visibility":unknown(),"static":found(json!(false)),"final":found(json!(false)),"abstract":found(json!(false))},
    "parameters":found(json!([
        {"name":if kotlin {"when"}else{"value"},"range":range(first),"nameRange":range(if kotlin {"`when`"}else{"value"}),
         "typeRef":"snapshot:parameter:String","annotations":[annotation]},
        {"name":if kotlin {"猫🙂"}else{"count"},"range":range(second),"nameRange":range(if kotlin {"`猫🙂`"}else{"count"}),
         "typeRef":"snapshot:parameter:int","annotations":[]}
    ]))});
    (text, json!({"declarations":[declaration]}))
}

#[test]
fn parameter_absence_unknown_and_known_zero_remain_distinct() {
    let text = "class Owner { void handler() {} }";
    let start = text.find("void").unwrap();
    let name = text.find("handler").unwrap();
    let body = text.find("{}").unwrap();
    let declaration = json!({"id":"draft:zero","owner":null,"range":{"start":name,"end":name+7},"name":"handler","annotations":[],
        "sourceKind":"Function","fullRange":{"start":start,"end":body+2},"headerRange":{"start":start,"end":body},
        "nameRange":{"start":name,"end":name+7},"bodyRange":{"start":body,"end":body+2},
        "modifiers":{"visibility":unknown(),"static":found(json!(false)),"final":found(json!(false)),"abstract":found(json!(false))}});
    let absent: SourceFacts =
        serde_json::from_value(json!({"declarations":[declaration.clone()]})).unwrap();
    absent.validate(text, &mut Registry::default()).unwrap();
    assert!(absent.declarations[0].parameters.is_none());
    assert!(serde_json::to_value(absent).unwrap()["declarations"][0]
        .get("parameters")
        .is_none());
    let mut uncertain = declaration.clone();
    uncertain["parameters"] = unknown();
    let uncertain: SourceFacts =
        serde_json::from_value(json!({"declarations":[uncertain]})).unwrap();
    uncertain.validate(text, &mut Registry::default()).unwrap();
    assert!(matches!(
        uncertain.declarations[0].parameters,
        Some(Fact::Unknown { .. })
    ));
    assert_eq!(
        serde_json::to_value(uncertain).unwrap()["declarations"][0]["parameters"],
        unknown()
    );
    let mut zero = declaration.clone();
    zero["parameters"] = found(json!([]));
    let zero: SourceFacts = serde_json::from_value(json!({"declarations":[zero]})).unwrap();
    zero.validate(text, &mut Registry::default()).unwrap();
    assert!(zero.declarations[0]
        .parameters
        .as_ref()
        .unwrap()
        .found()
        .unwrap()
        .is_empty());
    assert_eq!(
        serde_json::to_value(zero).unwrap()["declarations"][0]["parameters"],
        found(json!([]))
    );
    let mut malformed = declaration;
    malformed["parameters"] = Value::Null;
    assert!(serde_json::from_value::<SourceFacts>(json!({"declarations":[malformed]})).is_err());
}

#[test]
fn parameters_retain_java_annotations_and_foreign_type_references() {
    let (text, mut wire) = parameter_document(false);
    wire["source"] = source("source:foreign");
    wire["semanticScope"] = found(Value::Null);
    let facts: SourceFacts = serde_json::from_value(wire.clone()).unwrap();
    let mut registry = Registry::default();
    facts.validate(&text, &mut registry).unwrap();
    registry
        .require("snapshot:parameter:String", HandleKind::TypeRef)
        .unwrap();
    registry
        .require("snapshot:parameter:int", HandleKind::TypeRef)
        .unwrap();
    registry
        .require("snapshot:annotation:Local", HandleKind::TypeRef)
        .unwrap();
    assert!(registry
        .require("snapshot:parameter:String", HandleKind::Member)
        .is_err());
    assert!(registry
        .require("snapshot:parameter:String", HandleKind::Requirement)
        .is_err());
    let parameters = facts.declarations[0]
        .parameters
        .as_ref()
        .unwrap()
        .found()
        .unwrap();
    assert_eq!(parameters[0].name, "value");
    assert_eq!(parameters[1].name, "count");
    assert_eq!(
        parameters[0].range.text(&text).unwrap(),
        "@Local(name = \"speed\") final String value"
    );
    assert_eq!(
        serde_json::to_value(&facts.declarations[0].parameters).unwrap(),
        wire["declarations"][0]["parameters"]
    );
    assert_eq!(facts.source.as_ref().unwrap().id, "source:foreign");
}

#[test]
fn parameters_accept_kotlin_backticks_with_exact_utf8_name_ranges() {
    let (text, wire) = parameter_document(true);
    let facts: SourceFacts = serde_json::from_value(wire.clone()).unwrap();
    facts.validate(&text, &mut Registry::default()).unwrap();
    let parameters = facts.declarations[0]
        .parameters
        .as_ref()
        .unwrap()
        .found()
        .unwrap();
    assert_eq!(parameters[0].name, "when");
    assert_eq!(parameters[0].name_range.text(&text).unwrap(), "`when`");
    assert_eq!(parameters[1].name, "猫🙂");
    assert_eq!(parameters[1].name_range.text(&text).unwrap(), "`猫🙂`");
    assert_eq!(
        serde_json::to_value(&facts.declarations[0].parameters).unwrap(),
        wire["declarations"][0]["parameters"]
    );
    let mut broken = wire;
    broken["declarations"][0]["parameters"]["value"][1]["nameRange"]["end"] =
        json!(parameters[1].name_range.start + 2);
    let broken: SourceFacts = serde_json::from_value(broken).unwrap();
    assert!(broken.validate(&text, &mut Registry::default()).is_err());
}

#[test]
fn parameters_reject_bad_knowledge_ranges_order_names_and_annotation_facts() {
    let (text, wire) = parameter_document(false);
    let mut wrong_knowledge = wire.clone();
    wrong_knowledge["declarations"][0]["parameters"] = json!({"state":"definitelyAbsent"});
    let mut unknown_reasons = wire.clone();
    unknown_reasons["declarations"][0]["parameters"] = json!({"state":"unknown","reasons":[]});
    let mut not_callable = wire.clone();
    not_callable["declarations"][0]["sourceKind"] = json!("Property");
    not_callable["declarations"][0]["parameters"] = found(json!([]));
    let mut reordered = wire.clone();
    reordered["declarations"][0]["parameters"]["value"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    let mut overlap = wire.clone();
    overlap["declarations"][0]["parameters"]["value"][1]["range"]["start"] = json!(
        wire["declarations"][0]["parameters"]["value"][0]["range"]["end"]
            .as_u64()
            .unwrap()
            - 1
    );
    let mut outside_header = wire.clone();
    outside_header["declarations"][0]["parameters"]["value"][1]["range"]["end"] = json!(text.len());
    let mut wrong_name = wire.clone();
    wrong_name["declarations"][0]["parameters"]["value"][0]["name"] = json!("wrong");
    let mut wrong_name_range = wire.clone();
    wrong_name_range["declarations"][0]["parameters"]["value"][0]["nameRange"] =
        wire["declarations"][0]["nameRange"].clone();
    let mut empty_type = wire.clone();
    empty_type["declarations"][0]["parameters"]["value"][0]["typeRef"] = json!("");
    let mut outside_annotation = wire.clone();
    outside_annotation["declarations"][0]["parameters"]["value"][0]["annotations"][0]["range"]
        ["end"] = wire["declarations"][0]["parameters"]["value"][1]["range"]["end"].clone();
    let mut bad_annotation_fact = wire.clone();
    bad_annotation_fact["declarations"][0]["parameters"]["value"][0]["annotations"][0]
        ["arguments"][0]["value"] = json!({"state":"unknown","reasons":[]});
    for malformed in [
        wrong_knowledge,
        unknown_reasons,
        not_callable,
        reordered,
        overlap,
        outside_header,
        wrong_name,
        wrong_name_range,
        empty_type,
        outside_annotation,
        bad_annotation_fact,
    ] {
        let facts: SourceFacts = serde_json::from_value(malformed).unwrap();
        assert!(facts.validate(&text, &mut Registry::default()).is_err());
    }
    let entries: Vec<_> = (0..256)
        .map(|index| format!("int parameter{index}"))
        .collect();
    let over_limit_text = format!(
        "class Owner {{ void handler({}) {{}} }}",
        entries.join(", ")
    );
    let body = over_limit_text.find("{}").unwrap();
    let mut over_limit = wire;
    over_limit["declarations"][0]["fullRange"]["end"] = json!(body + 2);
    over_limit["declarations"][0]["headerRange"]["end"] = json!(body);
    over_limit["declarations"][0]["bodyRange"] = json!({"start":body,"end":body+2});
    over_limit["declarations"][0]["parameters"] = found(json!(entries.iter().enumerate().map(|(index, entry)| {
        let start = over_limit_text.find(entry).unwrap();
        json!({"name":format!("parameter{index}"),"range":{"start":start,"end":start+entry.len()},
            "nameRange":{"start":start+4,"end":start+entry.len()},"typeRef":"snapshot:parameter:int","annotations":[]})
    }).collect::<Vec<_>>()));
    let over_limit: SourceFacts = serde_json::from_value(over_limit).unwrap();
    assert!(over_limit
        .validate(&over_limit_text, &mut Registry::default())
        .unwrap_err()
        .to_string()
        .contains("parameter normalization limit"));
}

#[test]
fn mapped_strings_reject_split_surrogates_outside_bytes_and_missing_v2_ranges() {
    let (text, wire) = detailed_source();
    let mut split = wire.clone();
    split["declarations"][0]["annotations"][0]["arguments"][1]["value"]["value"]["annotation"]
        ["arguments"][0]["value"]["value"]["segments"][1]["decoded"]["end"] = json!(2);
    let mut outside = wire.clone();
    outside["declarations"][0]["headerRange"]["end"] = json!(1);
    let mut no_range = wire.clone();
    no_range["declarations"][0]
        .as_object_mut()
        .unwrap()
        .remove("fullRange");
    let mut too_large_int = wire.clone();
    too_large_int["declarations"][0]["annotations"][0]["arguments"][0]["value"]["value"] =
        json!({"kind":"integer","type":"int","value":2147483648i64});
    for invalid in [split, outside, no_range, too_large_int] {
        let facts: SourceFacts = serde_json::from_value(invalid).unwrap();
        assert!(facts.validate(&text, &mut Registry::default()).is_err());
    }
}

#[test]
fn foreign_source_facts_keep_semantic_scope_and_reject_other_evaluation_entries() {
    let (text, mut normalized) = detailed_source();
    let descriptor = source("source:1");
    normalized["source"] = descriptor.clone();
    normalized["semanticScope"] = found(Value::Null);
    let mut reader = Reader::with([
        facts(vec![
            json!({"sources":[descriptor],"next":null,"coverage":found(Value::Null)}),
        ]),
        facts(vec![normalized.clone()]),
        facts(vec![
            json!({"source":source("source:1"),"declarations":[],"semanticScope":unknown()}),
        ]),
    ]);
    let ops = operations(false);
    let mut inputs = ScopedInputs::new("e", &ops, &mut reader).unwrap();
    let entry = inputs.source_inventory().unwrap().entries.remove(0);
    let normalized = inputs.source_facts(Some(&entry)).unwrap();
    normalized
        .validate(&text, &mut Registry::default())
        .unwrap();
    assert!(normalized
        .semantic_scope
        .as_ref()
        .unwrap()
        .found()
        .is_some());
    let unavailable = inputs.source_facts(Some(&entry)).unwrap();
    assert!(matches!(
        unavailable.semantic_scope,
        Some(Fact::Unknown { .. })
    ));
    let mut foreign = ScopedInputs::new("other", &ops, Reader::default()).unwrap();
    assert!(matches!(
        foreign.source_facts(Some(&entry)),
        Err(FactsError::Protocol(_))
    ));
    assert_eq!(
        reader.queries[1][0],
        json!({"op":"sourceFacts","source":"source:1"})
    );
}

#[test]
fn source_location_preserves_utf16_revision_hash_and_unknown() {
    let location = json!({"target":"member:1","documentUri":"file:///C%3A/project/Target.kt",
        "range":{"start":{"line":4,"character":7},"end":{"line":4,"character":13}},
        "source":{"identity":{"kind":"source","revision":"opaque-host-revision"},"sha256":digest("😀 Target")}});
    let mut reader = Reader::with([facts(vec![found(location.clone()), unknown()])]);
    let mut inputs = ScopedInputs::new("e", &operations(false), &mut reader).unwrap();
    let result = inputs
        .source_locations(&["member:1".into(), "type:binary".into()])
        .unwrap();
    assert_eq!(
        serde_json::to_value(result[0].found().unwrap()).unwrap(),
        location
    );
    assert!(matches!(result[1], Fact::Unknown { .. }));
    assert_eq!(
        reader.queries[0][0],
        json!({"op":"sourceLocation","target":"member:1"})
    );
}

#[test]
fn source_location_mismatches_and_stale_errors_never_become_empty_success() {
    let valid = json!({"target":"type:1","documentUri":"file:///project/Target.java",
        "range":{"start":{"line":0,"character":6},"end":{"line":0,"character":12}},
        "source":{"identity":{"kind":"source","revision":"2"},"sha256":digest("class Target {}")}});
    let mut wrong_target = valid.clone();
    wrong_target["target"] = json!("type:other");
    let mut wrong_range = valid.clone();
    wrong_range["range"]["start"]["character"] = json!(15);
    let mut wrong_hash = valid.clone();
    wrong_hash["source"]["sha256"] = json!("ff");
    for value in [
        found(wrong_target),
        found(wrong_range),
        found(wrong_hash),
        json!({"state":"definitelyAbsent"}),
    ] {
        let mut inputs =
            ScopedInputs::new("e", &operations(false), Reader::with([facts(vec![value])])).unwrap();
        assert!(matches!(
            inputs.source_location("type:1"),
            Err(FactsError::Protocol(_))
        ));
    }
    let mut reader = Reader::default();
    reader
        .responses
        .push_back(Err(FactsError::Stale("source edited".into())));
    let mut inputs = ScopedInputs::new("e", &operations(false), reader).unwrap();
    assert!(matches!(
        inputs.source_location("type:1"),
        Err(FactsError::Stale(_))
    ));
}

fn abi(annotations: Option<Value>) -> Value {
    let mut value = json!({"identity":{"originId":"compile:1","internalName":"fixture/AtCode","contentSha256":digest("header")},
        "majorVersion":61,"minorVersion":0,"accessFlags":33,"superName":"java/lang/Object","interfaces":[],"signature":null,
        "sourceFile":null,"attributeNames":[],"innerClasses":[],"fields":[],"methods":[{"name":"value","descriptor":"()V",
        "accessFlags":1,"signature":null,"attributeNames":[]}]});
    if let Some(annotations) = annotations {
        value["annotations"] = annotations.clone();
        value["methods"][0]["annotations"] = annotations;
    }
    value
}

#[test]
fn binary_annotations_keep_exact_bits_long_surrogates_and_legacy_shape() {
    let values = json!([
        {"kind":"byte","value":-128},{"kind":"char","value":55357},{"kind":"double","bits":"8000000000000000"},
        {"kind":"float","bits":"7fc00001"},{"kind":"int","value":-2147483648i64},{"kind":"long","value":"-9223372036854775808"},
        {"kind":"short","value":-32768},{"kind":"boolean","value":true},{"kind":"string","value":"INVOKE"},
        {"kind":"string","utf16":[55357,65]},{"kind":"enum","descriptor":"Lfixture/Mode;","name":"A"},
        {"kind":"class","descriptor":"V"},{"kind":"annotation","value":{"descriptor":"Lfixture/Nested;","elements":[]}},
        {"kind":"array","values":[{"kind":"int","value":3}]}
    ]);
    let annotations = json!([{"runtimeVisible":false,"descriptor":"Lfixture/AtCode;","elements":[{"name":"all","value":{"kind":"array","values":values}}]}]);
    let wire = abi(Some(annotations));
    let typed: RawAbi = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), wire);
    assert!(matches!(
        typed.annotations.as_ref().unwrap()[0].elements[0].value,
        BinaryAnnotationValue::Array { .. }
    ));
    let legacy = abi(None);
    let typed: RawAbi = serde_json::from_value(legacy.clone()).unwrap();
    assert!(typed.annotations.is_none());
    assert!(typed.methods[0].annotations.is_none());
    assert_eq!(serde_json::to_value(typed).unwrap(), legacy);
}

#[test]
fn malformed_binary_annotation_values_fail_without_dropping_the_payload() {
    assert!(serde_json::from_value::<RawAbi>(abi(Some(Value::Null))).is_err());
    for value in [
        json!({"kind":"long","value":"9223372036854775808"}),
        json!({"kind":"double","bits":"7FF0000000000000"}),
        json!({"kind":"string","value":"lossy","utf16":[55357]}),
        json!({"kind":"string"}),
        json!({"kind":"enum","descriptor":"I","name":"A"}),
        json!({"kind":"class","descriptor":"()V"}),
        json!({"kind":"char","value":65536}),
    ] {
        let annotations = json!([{"runtimeVisible":true,"descriptor":"Lfixture/A;","elements":[{"name":"value","value":value}]}]);
        assert!(serde_json::from_value::<RawAbi>(abi(Some(annotations))).is_err());
    }
}
