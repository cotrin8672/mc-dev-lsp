use mcdev_companion::{
    host_facts::{
        bridge::FactReader,
        inputs::{InputOp, InputOps, ResourceAssociation, RuntimeAuthority, ScopedInputs},
        FactsError,
    },
    transport::read_frame,
    wire::Fact,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::{BufReader, Cursor},
};

#[derive(Default)]
struct Reader {
    responses: VecDeque<Result<Value, FactsError>>,
    requests: Vec<(String, Vec<Value>)>,
}

impl Reader {
    fn returning(values: impl IntoIterator<Item = Value>) -> Self {
        Self {
            responses: values.into_iter().map(Ok).collect(),
            requests: Vec::new(),
        }
    }
}

impl FactReader for Reader {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.requests
            .push((evaluation.to_owned(), queries.to_vec()));
        self.responses.pop_front().expect("unexpected host read")
    }
}

fn ops() -> InputOps {
    InputOps::intersect(&json!({
        "capabilities":["source-snapshots-v1","owned-resources-v1","runtime-class-bytes-v1","jvm-catalog-v1"],
        "supportedOps":{
            "source-snapshots-v1":["sourceInventory","sourceSnapshot"],
            "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"],
            "runtime-class-bytes-v1":["runtimeClass","classBytes"],
            "jvm-catalog-v1":["jvmCatalog","jvmClass"]
        }
    })).unwrap()
}

fn facts(values: Vec<Value>) -> Value {
    json!({"facts":values})
}
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
}
fn digest(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn source(id: &str, path: &str) -> Value {
    json!({"id":id,"displayPath":path,"documentUri":unknown(),"languageId":"java",
        "identity":{"kind":"source","revision":"3"}})
}
fn resource(id: &str) -> Value {
    json!({"id":id,"relativePath":"config.json","association":"source","classpathOrder":0,
        "editable":true,"documentUri":unknown(),"documentVersion":7,
        "identity":{"kind":"resource","revision":"7"}})
}
fn source_page(sources: Vec<Value>, next: Option<&str>, complete: bool) -> Value {
    facts(vec![
        json!({"sources":sources,"next":next,"coverage":if complete {found(Value::Null)} else {unknown()}}),
    ])
}
fn resource_page(resources: Vec<Value>, next: Option<&str>, complete: bool) -> Value {
    facts(vec![
        json!({"resources":resources,"next":next,"coverage":if complete {found(json!(true))} else {unknown()}}),
    ])
}
fn resource_chunk(offset: usize, encoded: &str, next: Option<&str>) -> Value {
    found(
        json!({"identity":{"kind":"resource","revision":"7","sha256":digest(b"abcde")},
        "documentVersion":7,"size":5,"offset":offset,"base64":encoded,"utf8":true,
        "text":found(json!("abcde")),"next":next}),
    )
}
fn runtime_descriptor(authority: &str) -> Value {
    let header = authority == "sdk-api-header";
    json!({"id":"runtime:1","internalName":"test/Type","originId":"jar:1",
        "contentSha256":if header {digest(b"header")} else {digest(b"abcde")},
        "byteLength":if header {0} else {5},"authority":authority,"codeAvailable":!header,
        "runtimeOnly":!header,"classHeader":unknown(),"hierarchy":unknown(),
        "runtimeAbiCoverage":if header {unknown()} else {found(Value::Null)},"sourceCurrent":unknown()})
}
fn class_chunk(offset: usize, encoded: &str, next: Option<&str>) -> Value {
    found(
        json!({"classIdentity":"runtime:1","offset":offset,"byteLength":5,
        "bytesBase64":encoded,"next":next}),
    )
}
fn catalog_entry(id: &str, origin: &str, order: usize) -> Value {
    json!({"entry":id,"internalName":"test/Type","originId":origin,"association":"compileClasspath",
        "classpathOrder":order,"authority":"compiled-input"})
}
fn catalog_page(entries: Vec<Value>, next: Option<&str>, complete: bool) -> Value {
    facts(vec![
        json!({"view":"selectedPhysicalJvmInputs","classes":entries,"next":next,
        "coverage":if complete {found(Value::Null)} else {unknown()},
        "problems":[],"sourceCurrent":unknown()}),
    ])
}
fn abi(origin: &str) -> Value {
    json!({"identity":{"originId":origin,"internalName":"test/Type","contentSha256":digest(origin.as_bytes())},
        "majorVersion":61,"minorVersion":0,"accessFlags":33,"superName":"java/lang/Object",
        "interfaces":[],"signature":null,"sourceFile":"Type.java","attributeNames":[],
        "innerClasses":[],"fields":[],"methods":[{"name":"bridge","descriptor":"()V",
            "accessFlags":4161,"signature":null,"attributeNames":[]}]})
}
fn catalog_class(entry: &str, origin: &str) -> Value {
    found(
        json!({"entry":entry,"authority":"compiled-input","abi":abi(origin),"sourceCurrent":unknown()}),
    )
}
fn fixture(data: &[u8]) -> (Value, Value) {
    let mut reader = BufReader::new(Cursor::new(data));
    let first = read_frame(&mut reader).unwrap().unwrap();
    let second = read_frame(&mut reader).unwrap().unwrap();
    assert!(read_frame(&mut reader).unwrap().is_none());
    (first["result"].clone(), second["result"].clone())
}

#[test]
fn negotiation_is_per_capability_and_operation() {
    let missing = InputOps::intersect(&json!({"capabilities":["semantic-v1"],
        "supportedOps":{"source-snapshots-v1":["sourceInventory"]}}))
    .unwrap();
    assert!(!missing.supports(InputOp::SourceInventory));
    let partial = InputOps::intersect(&json!({"capabilities":["source-snapshots-v1"],
        "supportedOps":{"source-snapshots-v1":["sourceInventory","futureOperation"]}}))
    .unwrap();
    assert!(partial.supports(InputOp::SourceInventory));
    assert!(!partial.supports(InputOp::SourceSnapshot));
    assert_eq!(
        partial.accepted()["source-snapshots-v1"],
        vec!["sourceInventory"]
    );
    assert!(
        InputOps::intersect(&json!({"capabilities":["source-snapshots-v1"],
        "supportedOps":{"source-snapshots-v1":[0]}}))
        .is_err()
    );
    let mut reader = Reader::default();
    let mut scoped = ScopedInputs::new("42", &partial, &mut reader).unwrap();
    assert!(matches!(
        scoped.resource_inventory(),
        Err(FactsError::Unsupported(_))
    ));
    assert!(reader.requests.is_empty());
}

#[test]
fn source_serializer_fixture_preserves_unknown_coverage_and_unicode_bytes() {
    let (inventory, snapshot) = fixture(include_bytes!(
        "fixtures/host_inputs/source-snapshots-v1.frames"
    ));
    let mut reader = Reader::returning([inventory, snapshot]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.source_inventory().unwrap();
    assert!(matches!(inventory.coverage, Fact::Unknown { .. }));
    assert_eq!(inventory.entries.len(), 2);
    let snapshots = scoped.source_snapshots(&inventory.entries[..1]).unwrap();
    let Fact::Found { value } = &snapshots[0] else {
        panic!("missing fixture snapshot")
    };
    assert_eq!(value.text, "// 未保存 Java 😀\n");
    assert_eq!(value.descriptor.document_version, None);
    assert_eq!(value.sha256, digest(value.text.as_bytes()));
    assert_eq!(
        reader.requests[1].1,
        vec![json!({"op":"sourceSnapshot","source":"42:0"})]
    );
}

#[test]
fn resource_serializer_fixture_preserves_exact_unsaved_content() {
    let (inventory, snapshot) = fixture(include_bytes!(
        "fixtures/host_inputs/owned-resources-v1.frames"
    ));
    let mut reader = Reader::returning([inventory, snapshot]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.resource_inventory().unwrap();
    assert_eq!(inventory.coverage, Fact::Found { value: json!(true) });
    let Fact::Found { value } = scoped.resource_snapshot(&inventory.entries[0]).unwrap() else {
        panic!("missing fixture resource")
    };
    assert_eq!(value.bytes, "{\"雪\":\"😀\"}\n".as_bytes());
    assert_eq!(value.document_version, Some(7));
    assert_eq!(
        value.text,
        Fact::Found {
            value: "{\"雪\":\"😀\"}\n".into()
        }
    );
}

#[test]
fn source_inventory_follows_pages_and_batches_snapshots() {
    let a = source("s:1", "A.java");
    let b = source("s:2", "B.java");
    let mut av = a.clone();
    av["text"] = json!("雪😀");
    av["sha256"] = json!(digest("雪😀".as_bytes()));
    let mut bv = b.clone();
    bv["text"] = json!("");
    bv["sha256"] = json!(digest(b""));
    let mut reader = Reader::returning([
        source_page(vec![a], Some("source-page:2"), false),
        source_page(vec![b], None, true),
        facts(vec![found(av), found(bv)]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.source_inventory().unwrap();
    assert_eq!(inventory.coverage, Fact::Found { value: Value::Null });
    let snapshots = scoped.source_snapshots(&inventory.entries).unwrap();
    assert!(matches!(&snapshots[0], Fact::Found { value } if value.text == "雪😀"));
    assert!(matches!(&snapshots[1], Fact::Found { value } if value.text.is_empty()));
    assert_eq!(reader.requests.len(), 3);
    assert_eq!(reader.requests[1].1[0]["after"], "source-page:2");
    assert_eq!(reader.requests[2].1.len(), 2);
}

#[test]
fn source_snapshot_batch_splits_only_the_documented_budget_error() {
    let sources = vec![source("s:1", "A.java"), source("s:2", "B.java")];
    let mut a = sources[0].clone();
    a["text"] = json!("a");
    a["sha256"] = json!(digest(b"a"));
    let mut b = sources[1].clone();
    b["text"] = json!("b");
    b["sha256"] = json!(digest(b"b"));
    let mut reader = Reader::default();
    reader
        .responses
        .push_back(Ok(source_page(sources, None, true)));
    reader.responses.push_back(Err(FactsError::Protocol(
        "extension response batch too large; split queries".into(),
    )));
    reader.responses.push_back(Ok(facts(vec![found(a)])));
    reader.responses.push_back(Ok(facts(vec![found(b)])));
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.source_inventory().unwrap();
    assert_eq!(
        scoped.source_snapshots(&inventory.entries).unwrap().len(),
        2
    );
    assert_eq!(reader.requests[1].1.len(), 2);
    assert_eq!(reader.requests[2].1.len(), 1);
    assert_eq!(reader.requests[3].1.len(), 1);
}

#[test]
fn resource_stream_checks_all_chunks_and_preserves_unknown() {
    let mut reader = Reader::returning([
        resource_page(vec![resource("r:1")], Some("page:2"), false),
        resource_page(vec![], None, true),
        facts(vec![resource_chunk(0, "YWJj", Some("cursor:1"))]),
        facts(vec![resource_chunk(3, "ZGU=", None)]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.resource_inventory().unwrap();
    let Fact::Found { value } = scoped.resource_snapshot(&inventory.entries[0]).unwrap() else {
        panic!("missing bytes")
    };
    assert_eq!(value.bytes, b"abcde");
    assert_eq!(reader.requests[3].1[0]["after"], "cursor:1");

    let mut reader = Reader::returning([
        resource_page(vec![resource("r:1")], None, true),
        facts(vec![unknown()]),
    ]);
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.resource_inventory().unwrap();
    assert!(matches!(
        scoped.resource_snapshot(&inventory.entries[0]).unwrap(),
        Fact::Unknown { .. }
    ));
}

#[test]
fn resource_snapshots_batch_many_inputs_and_their_continuations() {
    let resources: Vec<_> = (0..70)
        .map(|index| {
            let mut item = resource(&format!("r:{index}"));
            item["relativePath"] = json!(format!("config{index}.json"));
            item
        })
        .collect();
    let mut empty = resource_chunk(0, "", None);
    empty["value"]["size"] = json!(0);
    empty["value"]["identity"]["sha256"] = json!(digest(b""));
    empty["value"]["text"] = found(json!(""));
    let mut reader =
        Reader::returning([resource_page(resources, None, true), facts(vec![empty; 70])]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let entries = scoped.resource_inventory().unwrap().entries;
    let results = scoped.resource_snapshots(&entries).unwrap();
    assert_eq!(results.len(), 70);
    assert!(results
        .iter()
        .all(|result| matches!(result, Fact::Found { value } if value.bytes.is_empty())));
    assert_eq!(reader.requests.len(), 2);
    assert_eq!(reader.requests[1].1.len(), 70);

    let mut reader = Reader::returning([
        resource_page(vec![resource("r:1"), resource("r:2")], None, true),
        facts(vec![
            resource_chunk(0, "YWJj", Some("cursor:1")),
            resource_chunk(0, "YWJj", Some("cursor:2")),
        ]),
        facts(vec![
            resource_chunk(3, "ZGU=", None),
            resource_chunk(3, "ZGU=", None),
        ]),
    ]);
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let entries = scoped.resource_inventory().unwrap().entries;
    let results = scoped.resource_snapshots(&entries).unwrap();
    assert!(results
        .iter()
        .all(|result| matches!(result, Fact::Found { value } if value.bytes == b"abcde")));
    assert_eq!(reader.requests[1].1.len(), 2);
    assert_eq!(reader.requests[2].1.len(), 2);
    assert_eq!(reader.requests[2].1[0]["after"], "cursor:1");
    assert_eq!(reader.requests[2].1[1]["after"], "cursor:2");
}

#[test]
fn resource_snapshot_batch_retries_only_documented_response_budget() {
    let mut reader = Reader::default();
    reader.responses.push_back(Ok(resource_page(
        vec![resource("r:1"), resource("r:2")],
        None,
        true,
    )));
    reader.responses.push_back(Err(FactsError::Protocol(
        "extension response batch too large; split queries".into(),
    )));
    reader
        .responses
        .push_back(Ok(facts(vec![resource_chunk(0, "YWJjZGU=", None)])));
    reader
        .responses
        .push_back(Ok(facts(vec![resource_chunk(0, "YWJjZGU=", None)])));
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let entries = scoped.resource_inventory().unwrap().entries;
    let results = scoped.resource_snapshots(&entries).unwrap();
    assert!(results
        .iter()
        .all(|result| matches!(result, Fact::Found { value } if value.bytes == b"abcde")));
    assert_eq!(reader.requests[1].1.len(), 2);
    assert_eq!(reader.requests[2].1.len(), 1);
    assert_eq!(reader.requests[3].1.len(), 1);
}

#[test]
fn resource_stream_rejects_corruption_and_noncanonical_base64() {
    let mut wrong_offset = resource_chunk(3, "ZGU=", None);
    wrong_offset["value"]["offset"] = json!(2);
    let mut wrong_revision = resource_chunk(3, "ZGU=", None);
    wrong_revision["value"]["identity"]["revision"] = json!("different");
    let mut wrong_digest = resource_chunk(3, "ZGU=", None);
    wrong_digest["value"]["identity"]["sha256"] = json!(digest(b"wrong"));
    let mut wrong_version = resource_chunk(3, "ZGU=", None);
    wrong_version["value"]["documentVersion"] = json!(8);
    let mut wrong_text = resource_chunk(3, "ZGU=", None);
    wrong_text["value"]["text"] = found(json!("other"));
    for second in [
        wrong_offset,
        wrong_revision,
        wrong_digest,
        wrong_version,
        wrong_text,
        resource_chunk(3, "ZGV=", None),
        resource_chunk(3, "ZGU=", Some("cursor:1")),
    ] {
        let mut reader = Reader::returning([
            resource_page(vec![resource("r:1")], None, true),
            facts(vec![resource_chunk(0, "YWJj", Some("cursor:1"))]),
            facts(vec![second]),
        ]);
        let operations = ops();
        let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
        let inventory = scoped.resource_inventory().unwrap();
        assert!(scoped.resource_snapshot(&inventory.entries[0]).is_err());
    }
    let mut invalid_digest = resource_chunk(0, "YWJjZGU=", None);
    invalid_digest["value"]["identity"]["sha256"] = json!(digest(b"wrong"));
    let mut reader = Reader::returning([
        resource_page(vec![resource("r:1")], None, true),
        facts(vec![invalid_digest]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.resource_inventory().unwrap();
    assert!(scoped.resource_snapshot(&inventory.entries[0]).is_err());
}

#[test]
fn resource_empty_binary_and_negative_lookup_stay_distinct() {
    let mut empty = resource_chunk(0, "", None);
    empty["value"]["size"] = json!(0);
    empty["value"]["identity"]["sha256"] = json!(digest(b""));
    empty["value"]["text"] = found(json!(""));
    let mut binary = resource_chunk(0, "/w==", None);
    binary["value"]["size"] = json!(1);
    binary["value"]["identity"]["sha256"] = json!(digest(&[255]));
    binary["value"]["utf8"] = json!(false);
    binary["value"]["text"] = json!({"state":"definitelyAbsent"});
    let mut reader = Reader::returning([
        resource_page(vec![resource("r:1")], None, true),
        facts(vec![empty]),
        facts(vec![binary]),
        facts(vec![json!({"state":"definitelyAbsent"})]),
        facts(vec![unknown()]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let inventory = scoped.resource_inventory().unwrap();
    assert!(
        matches!(scoped.resource_snapshot(&inventory.entries[0]).unwrap(), Fact::Found { value } if value.bytes.is_empty() && value.text == Fact::Found {value:String::new()})
    );
    assert!(
        matches!(scoped.resource_snapshot(&inventory.entries[0]).unwrap(), Fact::Found { value } if value.bytes == [255] && !value.utf8)
    );
    assert!(matches!(
        scoped
            .resource_lookup("missing.json", Some(ResourceAssociation::Source))
            .unwrap(),
        Fact::DefinitelyAbsent
    ));
    assert!(matches!(
        scoped.resource_lookup("unknown.json", None).unwrap(),
        Fact::Unknown { .. }
    ));
    assert!(scoped.resource_lookup("../escape", None).is_err());
    assert!(scoped.resource_lookup("C:/escape", None).is_err());
}

#[test]
fn catalog_keeps_duplicate_origins_and_batches_distinct_abi() {
    let mut reader = Reader::returning([
        catalog_page(
            vec![catalog_entry("catalog@1", "jar:v1", 0)],
            Some("catalog-page:2"),
            false,
        ),
        catalog_page(vec![catalog_entry("catalog@2", "jar:v2", 1)], None, true),
        facts(vec![
            catalog_class("catalog@1", "jar:v1"),
            catalog_class("catalog@2", "jar:v2"),
        ]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let catalog = scoped.jvm_catalog("test/").unwrap();
    assert_eq!(catalog.coverage, Fact::Found { value: Value::Null });
    assert!(matches!(catalog.source_current, Fact::Unknown { .. }));
    assert_eq!(catalog.entries.len(), 2);
    assert_eq!(
        catalog.entries[0].descriptor.internal_name,
        catalog.entries[1].descriptor.internal_name
    );
    assert_ne!(
        catalog.entries[0].descriptor.origin_id,
        catalog.entries[1].descriptor.origin_id
    );
    let classes = scoped.jvm_classes(&catalog.entries).unwrap();
    assert!(
        matches!(&classes[0], Fact::Found { value } if value.abi.methods[0].access_flags == 4161)
    );
    assert!(
        matches!(&classes[1], Fact::Found { value } if value.abi.identity.origin_id == "jar:v2")
    );
    assert_eq!(reader.requests[2].1.len(), 2);
}

#[test]
fn runtime_chunks_are_verified_and_sdk_header_cannot_grant_code() {
    let mut reader = Reader::returning([
        facts(vec![found(runtime_descriptor("runtime-bytecode"))]),
        facts(vec![class_chunk(0, "YWJj", Some("cursor:1"))]),
        facts(vec![class_chunk(3, "ZGU=", None)]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let Fact::Found { value } = scoped.runtime_class("test/Type").unwrap() else {
        panic!("missing class")
    };
    assert!(
        matches!(scoped.class_bytes(&value).unwrap(), Fact::Found { value } if value == b"abcde")
    );

    let mut reader = Reader::returning([
        facts(vec![found(runtime_descriptor("sdk-api-header"))]),
        facts(vec![unknown()]),
    ]);
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let Fact::Found { value } = scoped.runtime_class("test/Type").unwrap() else {
        panic!("missing header")
    };
    assert_eq!(value.descriptor.authority, RuntimeAuthority::SdkApiHeader);
    assert!(matches!(
        scoped.class_bytes(&value).unwrap(),
        Fact::Unknown { .. }
    ));

    let mut reader = Reader::returning([
        facts(vec![found(runtime_descriptor("sdk-api-header"))]),
        facts(vec![class_chunk(0, "YWJjZGU=", None)]),
    ]);
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let Fact::Found { value } = scoped.runtime_class("test/Type").unwrap() else {
        panic!("missing header")
    };
    assert!(scoped.class_bytes(&value).is_err());
}

#[test]
fn class_stream_rejects_identity_length_offset_and_digest_errors() {
    for (field, bad_value) in [
        ("classIdentity", json!("foreign")),
        ("offset", json!(1)),
        ("byteLength", json!(6)),
        ("bytesBase64", json!("YR==")),
    ] {
        let mut chunk = class_chunk(0, "YWJjZGU=", None);
        chunk["value"][field] = bad_value;
        let mut reader = Reader::returning([
            facts(vec![found(runtime_descriptor("runtime-bytecode"))]),
            facts(vec![chunk]),
        ]);
        let operations = ops();
        let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
        let Fact::Found { value } = scoped.runtime_class("test/Type").unwrap() else {
            panic!("missing class")
        };
        assert!(scoped.class_bytes(&value).is_err(), "{field}");
    }
    let mut descriptor = runtime_descriptor("runtime-bytecode");
    descriptor["contentSha256"] = json!(digest(b"wrong"));
    let mut reader = Reader::returning([
        facts(vec![found(descriptor)]),
        facts(vec![class_chunk(0, "YWJjZGU=", None)]),
    ]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let Fact::Found { value } = scoped.runtime_class("test/Type").unwrap() else {
        panic!("missing class")
    };
    assert!(scoped.class_bytes(&value).is_err());
}

#[test]
fn issued_handles_cannot_cross_evaluation_scopes_or_be_modified() {
    let mut reader = Reader::returning([source_page(vec![source("s:1", "A.java")], None, true)]);
    let operations = ops();
    let entry = {
        let mut scoped = ScopedInputs::new("same-evaluation", &operations, &mut reader).unwrap();
        scoped.source_inventory().unwrap().entries.remove(0)
    };
    let mut other = Reader::default();
    let mut scoped = ScopedInputs::new("same-evaluation", &operations, &mut other).unwrap();
    assert!(scoped.source_snapshots(&[entry]).is_err());
    assert!(other.requests.is_empty());

    let mut reader = Reader::returning([resource_page(vec![resource("r:1")], None, true)]);
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let mut entry = scoped.resource_inventory().unwrap().entries.remove(0);
    entry.descriptor.relative_path = "other.json".into();
    assert!(scoped.resource_snapshot(&entry).is_err());
    assert_eq!(reader.requests.len(), 1);
}

#[test]
fn unknown_coverage_is_never_successful_absence_and_bad_pages_fail() {
    let mut reader = Reader::returning([catalog_page(vec![], None, false)]);
    let operations = ops();
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    let catalog = scoped.jvm_catalog("missing/").unwrap();
    assert!(catalog.entries.is_empty());
    assert!(matches!(catalog.coverage, Fact::Unknown { .. }));
    let mut malformed = catalog_page(vec![], Some("next"), true);
    malformed["facts"][0]["coverage"] = found(Value::Null);
    let mut reader = Reader::returning([malformed]);
    let mut scoped = ScopedInputs::new("42", &operations, &mut reader).unwrap();
    assert!(scoped.jvm_catalog("missing/").is_err());
}
