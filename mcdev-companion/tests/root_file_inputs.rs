//! Working-draft DTO tests, not a published serializer or live MC acceptance proof.
use mcdev_companion::{
    host_facts::{bridge::FactReader, inputs::*, FactsError},
    wire::Fact,
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
    fn with(responses: impl IntoIterator<Item = Value>) -> Self {
        Self {
            responses: responses.into_iter().map(Ok).collect(),
            queries: Vec::new(),
        }
    }
}
impl FactReader for Reader {
    fn read_facts(&mut self, _: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.queries.push(queries.to_vec());
        self.responses
            .pop_front()
            .expect("unexpected root-file host read")
    }
}
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
}
fn facts(values: Vec<Value>) -> Value {
    json!({"facts":values})
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn operations() -> InputOps {
    InputOps::intersect(&json!({"capabilities":["semantic-v1","root-files-v1"],
        "supportedOps":{"root-files-v1":["rootFileInventory","rootFileSnapshot"]}}))
    .unwrap()
}
fn root() -> Value {
    json!({"identity":"root:1","uri":"file:///workspace","rootKind":"workspace","revision":"42"})
}
fn file(token: &str, relative: &str) -> Value {
    json!({"file":token,"uri":format!("file:///workspace/{relative}"),"relativePath":relative})
}
fn page(filter: &RootFileFilter, files: Vec<Value>, next: Option<&str>, complete: bool) -> Value {
    json!({"root":root(),"filter":filter,"files":files,"next":next,
        "coverage":if complete {found(json!(true))} else {unknown()},"problems":[]})
}
fn chunk(
    token: &str,
    relative: &str,
    body: &str,
    offset: usize,
    text: &str,
    next: Option<&str>,
) -> Value {
    found(
        json!({"file":token,"root":root(),"uri":format!("file:///workspace/{relative}"),"relativePath":relative,
        "identity":{"kind":"rootFile","revision":"900","sha256":digest(body)},"documentVersion":7,
        "size":body.len(),"offset":offset,"text":text,"next":next}),
    )
}

#[test]
fn root_file_negotiation_requires_capability_and_each_operation_without_other_authority() {
    for host in [
        json!({"capabilities":["semantic-v1"]}),
        json!({"capabilities":["root-files-v1"]}),
        json!({"supportedOps":{"root-files-v1":["rootFileInventory","rootFileSnapshot"]}}),
    ] {
        let ops = InputOps::intersect(&host).unwrap();
        assert!(!ops.supports(InputOp::RootFileInventory));
        assert!(!ops.supports(InputOp::RootFileSnapshot));
    }
    let ops = operations();
    assert_eq!(
        ops.accepted()["root-files-v1"],
        vec!["rootFileInventory", "rootFileSnapshot"]
    );
    let mut reader = Reader::default();
    let mut inputs = ScopedInputs::new("e", &ops, &mut reader).unwrap();
    assert!(matches!(
        inputs.resource_inventory(),
        Err(FactsError::Unsupported(_))
    ));
    assert!(matches!(
        inputs.source_inventory(),
        Err(FactsError::Unsupported(_))
    ));
    assert!(matches!(
        inputs.runtime_class("fixture/Target"),
        Err(FactsError::Unsupported(_))
    ));
    assert!(reader.queries.is_empty());
    let mut inputs = ScopedInputs::new("e", &InputOps::default(), Reader::default()).unwrap();
    assert!(matches!(
        inputs.root_file_inventory(&RootFileFilter::default()),
        Err(FactsError::Unsupported(_))
    ));
    assert!(matches!(
        inputs.root_file_snapshots(&[]),
        Err(FactsError::Unsupported(_))
    ));
}

#[test]
fn root_file_filters_are_literal_case_insensitive_and_have_no_fixed_exclusions() {
    for bad in [
        "",
        ".",
        "..",
        "../elsewhere",
        "a/b",
        "a\\b",
        "C:root",
        "*.json",
        "a?",
        "[ab]",
        "line\n",
    ] {
        let mut filter = RootFileFilter::default();
        filter.basenames.insert(bad.into());
        assert!(filter.validate().is_err(), "{bad}");
    }
    let mut filter = RootFileFilter::default();
    filter.extensions.insert(".json".into());
    assert!(filter.validate().is_err());
    filter = RootFileFilter::default();
    filter.basenames.insert("é".repeat(129));
    assert!(filter.validate().is_err());
    filter.basenames.clear();
    filter.basenames.insert("é".repeat(128));
    filter.validate().unwrap();
    filter.basenames = (0..129).map(|value| value.to_string()).collect();
    assert!(filter.validate().is_err());
    filter = RootFileFilter::default();
    filter.basenames.insert("CONFIG.JSON".into());
    filter.extensions.insert("JsOn5".into());
    filter.excluded_directories.insert("BuIlD".into());
    let mut reader = Reader::with([facts(vec![page(
        &filter,
        vec![
            file("a", "config.json"),
            file("b", ".gradle/cache/mappings.JSON5"),
        ],
        None,
        true,
    )])]);
    let mut inputs = ScopedInputs::new("e", &operations(), &mut reader).unwrap();
    let inventory = inputs.root_file_inventory(&filter).unwrap();
    assert_eq!(inventory.entries.len(), 2);
    assert_eq!(
        reader.queries[0][0]["filter"]["basenames"],
        json!(["CONFIG.JSON"])
    );
    for relative in [
        "BUILD/config.json",
        "build/deep/config.json",
        "elsewhere/not-selected.txt",
    ] {
        let mut inputs = ScopedInputs::new(
            "e",
            &operations(),
            Reader::with([facts(vec![page(
                &filter,
                vec![file("a", relative)],
                None,
                true,
            )])]),
        )
        .unwrap();
        assert!(matches!(
            inputs.root_file_inventory(&filter),
            Err(FactsError::Protocol(_))
        ));
    }
    let mut all = RootFileFilter::default();
    all.excluded_directories.insert("BUILD".into());
    let mut inputs = ScopedInputs::new(
        "e",
        &operations(),
        Reader::with([facts(vec![page(
            &all,
            vec![file("a", "BUILD"), file("b", ".git/config")],
            None,
            true,
        )])]),
    )
    .unwrap();
    assert_eq!(
        inputs.root_file_inventory(&all).unwrap().entries.len(),
        2,
        "exclusions match directory segments, not file basenames"
    );
}

#[test]
fn root_file_pagination_keeps_exact_filter_root_domain_and_terminal_coverage() {
    let filter = RootFileFilter::default();
    let mut reader = Reader::with([
        facts(vec![page(
            &filter,
            vec![file("a", "one/config.json")],
            Some("cursor:1"),
            false,
        )]),
        facts(vec![page(
            &filter,
            vec![file("b", "two/config.json")],
            None,
            true,
        )]),
    ]);
    let mut inputs = ScopedInputs::new("e", &operations(), &mut reader).unwrap();
    let inventory = inputs.root_file_inventory(&filter).unwrap();
    assert_eq!(inventory.entries.len(), 2);
    assert_eq!(inventory.coverage, Fact::Found { value: true });
    assert_eq!(inventory.root.as_ref().unwrap().revision, "42");
    assert_eq!(inventory.entries[0].root, inventory.entries[1].root);
    assert_eq!(reader.queries[1][0]["after"], "cursor:1");
    for query in reader.queries.iter().flatten() {
        assert_eq!(query.as_object().unwrap().len(), 3);
        assert!(
            query.get("root").is_none()
                && query.get("path").is_none()
                && query.get("scope").is_none()
        );
    }
}

#[test]
fn root_file_unavailable_root_and_body_never_become_successful_empty_values() {
    let filter = RootFileFilter::default();
    let mut unavailable = page(&filter, Vec::new(), None, false);
    unavailable["root"] = Value::Null;
    unavailable["problems"] = json!(["root-file-domain-unavailable"]);
    let mut inputs =
        ScopedInputs::new("e", &operations(), Reader::with([facts(vec![unavailable])])).unwrap();
    let inventory = inputs.root_file_inventory(&filter).unwrap();
    assert!(inventory.root.is_none());
    assert!(inventory.entries.is_empty());
    assert!(matches!(inventory.coverage, Fact::Unknown { .. }));
    let mut inputs = ScopedInputs::new(
        "e",
        &operations(),
        Reader::with([
            facts(vec![page(
                &filter,
                vec![file("a", "config.json")],
                None,
                true,
            )]),
            facts(vec![unknown()]),
        ]),
    )
    .unwrap();
    let entry = inputs
        .root_file_inventory(&filter)
        .unwrap()
        .entries
        .remove(0);
    assert!(matches!(
        inputs.root_file_snapshot(&entry).unwrap(),
        Fact::Unknown { .. }
    ));
    let mut invalid = page(&filter, Vec::new(), None, true);
    invalid["root"] = Value::Null;
    let mut inputs =
        ScopedInputs::new("e", &operations(), Reader::with([facts(vec![invalid])])).unwrap();
    assert!(inputs.root_file_inventory(&filter).is_err());
}

#[test]
fn root_file_page_mismatches_cycles_duplicates_and_false_coverage_are_rejected() {
    let filter = RootFileFilter::default();
    let first = page(&filter, vec![file("a", "a.json")], Some("cursor"), false);
    let last = page(&filter, vec![file("b", "b.json")], None, true);
    let mut changed_root = last.clone();
    changed_root["root"]["identity"] = json!("root:other");
    let mut changed_revision = last.clone();
    changed_revision["root"]["revision"] = json!("43");
    let mut changed_filter = last.clone();
    changed_filter["filter"]["extensions"] = json!(["json"]);
    let mut lost_root = last.clone();
    lost_root["root"] = Value::Null;
    lost_root["coverage"] = unknown();
    lost_root["files"] = json!([]);
    let mut cycle = last.clone();
    cycle["next"] = json!("cursor");
    cycle["coverage"] = unknown();
    let duplicate = page(&filter, vec![file("a", "other.json")], None, true);
    let duplicate_uri = page(&filter, vec![file("b", "a.json")], None, true);
    let mut problem = last.clone();
    problem["problems"] = json!(["unreadable-root-file-tree"]);
    for value in [
        changed_root,
        changed_revision,
        changed_filter,
        lost_root,
        cycle,
        duplicate,
        duplicate_uri,
        problem,
    ] {
        let mut inputs = ScopedInputs::new(
            "e",
            &operations(),
            Reader::with([facts(vec![first.clone()]), facts(vec![value])]),
        )
        .unwrap();
        assert!(matches!(
            inputs.root_file_inventory(&filter),
            Err(FactsError::Protocol(_))
        ));
    }
    let mut early_complete = first.clone();
    early_complete["coverage"] = found(json!(true));
    let mut no_progress = first.clone();
    no_progress["files"] = json!([]);
    let mut wrong_revision = last.clone();
    wrong_revision["root"]["revision"] = json!(42);
    let mut opaque_revision = last.clone();
    opaque_revision["root"]["revision"] = json!("not-a-decimal");
    let mut false_coverage = last.clone();
    false_coverage["coverage"] = found(json!(false));
    let too_many = page(
        &filter,
        (0..257)
            .map(|i| file(&i.to_string(), &format!("{i}.json")))
            .collect(),
        None,
        true,
    );
    for value in [
        early_complete,
        no_progress,
        wrong_revision,
        opaque_revision,
        false_coverage,
        too_many,
    ] {
        let mut inputs =
            ScopedInputs::new("e", &operations(), Reader::with([facts(vec![value])])).unwrap();
        assert!(matches!(
            inputs.root_file_inventory(&filter),
            Err(FactsError::Protocol(_))
        ));
    }
}

#[test]
fn root_file_tokens_are_private_to_the_issuing_evaluation_filter_and_unchanged_entry() {
    let all = RootFileFilter::default();
    let mut selected = RootFileFilter::default();
    selected.extensions.insert("json".into());
    let mut reader = Reader::with([
        facts(vec![page(&all, vec![file("a", "config.json")], None, true)]),
        facts(vec![page(
            &selected,
            vec![file("b", "config.json")],
            None,
            true,
        )]),
    ]);
    let ops = operations();
    let mut inputs = ScopedInputs::new("e", &ops, &mut reader).unwrap();
    let entry = inputs.root_file_inventory(&all).unwrap().entries.remove(0);
    let mut changed = entry.clone();
    changed.descriptor.uri = "file:///elsewhere/config.json".into();
    assert!(matches!(
        inputs.root_file_snapshot(&changed),
        Err(FactsError::Protocol(_))
    ));
    let mut changed = entry.clone();
    changed.root.revision = "43".into();
    assert!(matches!(
        inputs.root_file_snapshot(&changed),
        Err(FactsError::Protocol(_))
    ));
    let mut changed = entry.clone();
    changed.filter = selected.clone();
    assert!(matches!(
        inputs.root_file_snapshot(&changed),
        Err(FactsError::Protocol(_))
    ));
    let mut foreign = ScopedInputs::new("other", &ops, Reader::default()).unwrap();
    assert!(matches!(
        foreign.root_file_snapshot(&entry),
        Err(FactsError::Protocol(_))
    ));
    assert!(
        matches!(
            inputs.root_file_inventory(&selected),
            Err(FactsError::Protocol(_))
        ),
        "one domain identity cannot be rebound to another exact filter"
    );
    assert_eq!(
        reader.queries.len(),
        2,
        "foreign/modified entries are rejected before any snapshot request"
    );
}

#[test]
fn root_file_snapshot_reassembles_scalar_safe_utf8_bytes_and_distinct_overlay_versions() {
    let filter = RootFileFilter::default();
    let first = "a".repeat(96 * 1024 - 1);
    let last = "😀猫";
    let body = format!("{first}{last}");
    let mut reader = Reader::with([
        facts(vec![page(
            &filter,
            vec![file("a", "config.json"), file("b", "other.json")],
            None,
            true,
        )]),
        facts(vec![
            chunk("a", "config.json", &body, 0, &first, Some("text:1")),
            unknown(),
        ]),
        facts(vec![chunk(
            "a",
            "config.json",
            &body,
            first.len(),
            last,
            None,
        )]),
    ]);
    let mut inputs = ScopedInputs::new("e", &operations(), &mut reader).unwrap();
    let entries = inputs.root_file_inventory(&filter).unwrap().entries;
    let snapshots = inputs.root_file_snapshots(&entries).unwrap();
    let content = snapshots[0].found().unwrap();
    assert_eq!(content.text, body);
    assert_eq!(content.size, body.len());
    assert_eq!(content.root.revision, "42");
    assert_eq!(content.identity.revision, "900");
    assert_eq!(content.document_version, Some(7));
    assert_eq!(content.identity.sha256, digest(&body));
    assert!(matches!(snapshots[1], Fact::Unknown { .. }));
    assert_eq!(
        reader.queries[2],
        vec![json!({"op":"rootFileSnapshot","file":"a","after":"text:1"})]
    );
}

#[test]
fn root_file_stream_requires_unchanged_identity_offsets_full_hash_and_terminal_length() {
    let filter = RootFileFilter::default();
    let body = "a😀猫";
    let first = chunk("a", "a.json", body, 0, "a", Some("cursor"));
    let last = chunk("a", "a.json", body, 1, "😀猫", None);
    let mut wrong_offset = last.clone();
    wrong_offset["value"]["offset"] = json!(2);
    let mut wrong_hash = last.clone();
    wrong_hash["value"]["identity"]["sha256"] = json!(digest("else"));
    let mut wrong_identity = last.clone();
    wrong_identity["value"]["identity"]["revision"] = json!("901");
    let mut wrong_kind = last.clone();
    wrong_kind["value"]["identity"]["kind"] = json!("resource");
    let mut wrong_root = last.clone();
    wrong_root["value"]["root"]["revision"] = json!("43");
    let mut wrong_file = last.clone();
    wrong_file["value"]["file"] = json!("other");
    let mut wrong_version = last.clone();
    wrong_version["value"]["documentVersion"] = json!(8);
    let mut overflow_version = last.clone();
    overflow_version["value"]["documentVersion"] = json!(2147483648u64);
    let mut wrong_size = last.clone();
    wrong_size["value"]["size"] = json!(body.len() + 1);
    let mut premature = last.clone();
    premature["value"]["text"] = json!("😀");
    let mut cycle = last.clone();
    cycle["value"]["next"] = json!("cursor");
    cycle["value"]["size"] = json!(body.len() + 1);
    for invalid in [
        wrong_offset,
        wrong_hash,
        wrong_identity,
        wrong_kind,
        wrong_root,
        wrong_file,
        wrong_version,
        overflow_version,
        wrong_size,
        premature,
        cycle,
        json!({"state":"definitelyAbsent"}),
    ] {
        let mut inputs = ScopedInputs::new(
            "e",
            &operations(),
            Reader::with([
                facts(vec![page(&filter, vec![file("a", "a.json")], None, true)]),
                facts(vec![first.clone()]),
                facts(vec![invalid]),
            ]),
        )
        .unwrap();
        let entry = inputs
            .root_file_inventory(&filter)
            .unwrap()
            .entries
            .remove(0);
        assert!(matches!(
            inputs.root_file_snapshot(&entry),
            Err(FactsError::Protocol(_))
        ));
    }
    let mut inputs = ScopedInputs::new(
        "e",
        &operations(),
        Reader::with([
            facts(vec![page(&filter, vec![file("a", "a.json")], None, true)]),
            facts(vec![chunk("a", "a.json", body, 0, "not the body", None)]),
        ]),
    )
    .unwrap();
    let entry = inputs
        .root_file_inventory(&filter)
        .unwrap()
        .entries
        .remove(0);
    assert!(inputs.root_file_snapshot(&entry).is_err());
}

#[test]
fn root_file_stream_preserves_unknown_after_partial_and_accepts_proved_empty_utf8() {
    let filter = RootFileFilter::default();
    let mut inputs = ScopedInputs::new(
        "e",
        &operations(),
        Reader::with([
            facts(vec![page(
                &filter,
                vec![file("a", "a.json"), file("b", "empty.json")],
                None,
                true,
            )]),
            facts(vec![
                chunk("a", "a.json", "abc", 0, "a", Some("cursor")),
                chunk("b", "empty.json", "", 0, "", None),
            ]),
            facts(vec![unknown()]),
        ]),
    )
    .unwrap();
    let entries = inputs.root_file_inventory(&filter).unwrap().entries;
    let content = inputs.root_file_snapshots(&entries).unwrap();
    assert!(matches!(content[0], Fact::Unknown { .. }));
    assert_eq!(content[1].found().unwrap().text, "");
    assert_eq!(content[1].found().unwrap().identity.sha256, digest(""));
}

#[test]
fn root_file_stale_errors_and_uri_escapes_are_not_reinterpreted_as_known_empty() {
    let filter = RootFileFilter::default();
    let mut reader = Reader::default();
    reader
        .responses
        .push_back(Err(FactsError::Stale("root changed".into())));
    let mut inputs = ScopedInputs::new("e", &operations(), reader).unwrap();
    assert!(matches!(
        inputs.root_file_inventory(&filter),
        Err(FactsError::Stale(_))
    ));
    for (relative, uri) in [
        ("../escape.json", "file:///escape.json"),
        ("config.json", "file:///foreign/config.json"),
        ("config.json", "https://example.com/config.json"),
    ] {
        let descriptor = json!({"file":"a","relativePath":relative,"uri":uri});
        let mut inputs = ScopedInputs::new(
            "e",
            &operations(),
            Reader::with([facts(vec![page(&filter, vec![descriptor], None, true)])]),
        )
        .unwrap();
        assert!(inputs.root_file_inventory(&filter).is_err());
    }
}
