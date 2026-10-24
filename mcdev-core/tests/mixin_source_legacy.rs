use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
#[path = "fixtures/mixin_source/replay.rs"]
mod replay;
use replay::*;
#[test]
fn navigation_source_ranges_and_type_literals_match_original() {
    for line in include_str!("fixtures/mixin_source/navigation-regressions.jsonl").lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        let actual = replay(&row);
        assert!(kinds_match(&row, &actual));
        assert_eq!(
            canonical(actual, ""),
            expected_output(&row),
            "{} {}",
            row["legacyCase"],
            row["api"]
        );
    }
}
#[test]
fn normalization_preserves_import_alias_and_metadata_dictionary_keys() {
    let value = json!({"explicit":{"Target_alias":"pkg.Target"},"ambiguous_explicit":{"Target_alias":["a.Target","b.Target"]},"metadata":{"custom_key":"value"}});
    let expected = json!({"explicit":{"Target_alias":"pkg.Target"},"ambiguousExplicit":{"Target_alias":["a.Target","b.Target"]},"metadata":{"custom_key":"value"}});
    assert_eq!(canonical(value, ""), expected);
    assert_ne!(
        canonical(json!({"explicit":{"Target_alias":"pkg.Target"}}), ""),
        canonical(json!({"explicit":{"TargetAlias":"pkg.Target"}}), "")
    );
}
#[test]
fn every_original_source_case_and_call_replays_whole_results() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mixin_source/source-calls.jsonl");
    let input =
        fs::read_to_string(&path).expect("run one-time fixture generator and retain fixture");
    let manifest: Vec<Value> = serde_json::from_str(
        &fs::read_to_string(path.with_file_name("source-cases.json")).unwrap(),
    )
    .unwrap();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = vec![];
    for (line, row) in input.lines().enumerate() {
        let row: Value = serde_json::from_str(row).unwrap();
        let id = row["legacyCase"].as_str().unwrap().to_string();
        *seen.entry(id.clone()).or_default() += 1;
        let expected = expected_output(&row);
        let actual = std::panic::catch_unwind(|| replay(&row));
        match actual {
            Ok(actual) => {
                let kinds_match = kinds_match(&row, &actual);
                let actual_kind = output_kind(&actual).map(str::to_string);
                let a = canonical(actual, "");
                let e = canonical(expected, "");
                if a != e || !kinds_match {
                    failures.push(json!({"line":line+1,"legacyCase":id,"api":row["api"],"expected":e,"actual":a,"expectedKind":expected_kind(&row),"actualKind":actual_kind}));
                }
            }
            Err(_) => {
                failures.push(json!({"line":line+1,"legacyCase":id,"api":row["api"],"panic":true}))
            }
        }
    }
    let zero_call:BTreeSet<_>=BTreeSet::from(["mcdev-core/src/test/kotlin/io/github/mcdev/core/mixin/MixinSemanticModelParserTest.kt::retainsExplicitResolvedMixinExtrasContexts"]);
    for case in &manifest {
        assert_eq!(case["status"], "passed");
        let id = case["legacyCase"].as_str().unwrap();
        let expected = case["calls"].as_u64().unwrap() as usize;
        if expected == 0 {
            assert!(zero_call.contains(id), "uncaptured legacy case: {id}");
        } else {
            assert_eq!(seen.get(id), Some(&expected), "lost calls from {id}");
        }
    }
    let log = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/rust-test-logs/mixin-source-golden-failures.json");
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&log, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
    assert!(failures.is_empty(),"{} full-output mismatches across {} original cases / {} calls; first={}; full bounded failure data={}",failures.len(),manifest.len(),input.lines().count(),failures.first().map_or(String::new(),|v|v["legacyCase"].as_str().unwrap_or("").to_string()+" "+v["api"].as_str().unwrap_or("")),log.display());
}
