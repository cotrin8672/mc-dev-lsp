//! Whole-result definition/reference replay from original inputs and host/index facts.
#[allow(dead_code)]
#[path = "../mixin_completion/replay.rs"]
mod completion_replay;
#[allow(dead_code)]
#[path = "../mixin_source/replay.rs"]
mod source_replay;
use mcdev_core::{
    bytecode::{AtTargetCandidate, BytecodeIndex},
    mixin_navigation::*,
    mixin_source::MixinClassModel,
    *,
};
use serde_json::{json, Value};
struct NoBytecode;
impl BytecodeIndex for NoBytecode {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("navigation must use its supplied class-index facts")
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!("unexpected bytecode query")
    }
}
pub fn replay_case(case: &str) {
    let rows = include_str!("legacy_calls.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row["legacyCase"] == case)
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "uncaptured {case}");
    for (i, row) in rows.iter().enumerate() {
        let a = &row["args"];
        let api = row["api"].as_str().unwrap();
        let index = completion_replay::RecordedIndex {
            reads: row["reads"].as_array().unwrap(),
        };
        let service = MixinDefinitionService::new(&index, &NoBytecode);
        let source = a["source"].as_str().unwrap_or("");
        let model = a
            .get("semanticModel")
            .filter(|v| !v.is_null())
            .map(source_replay::deserialize::<MixinClassModel>);
        let actual = match api {
            "MixinDefinitionService.definitionsAt" => json!(service.definitions_at(
                source,
                a["line"].as_u64().unwrap() as usize,
                a["character"].as_u64().unwrap() as usize,
                model.as_ref(),
                a["documentUri"].as_str()
            )),
            "MixinDefinitionService.definitionsAtOffset" => json!(service.definitions_at_offset(
                source,
                source_replay::byte(source, a["offset"].as_i64().unwrap()).unwrap(),
                model.as_ref(),
                a["documentUri"].as_str()
            )),
            "MixinReferenceService.findReferences" => {
                json!(MixinReferenceService::find_references(
                    &serde_json::from_value::<McDefinitionTarget>(a["target"].clone()).unwrap(),
                    &serde_json::from_value::<Vec<SourceScanEntry>>(a["sources"].clone()).unwrap()
                ))
            }
            "AtTargetParser.parse" => json!(AtTargetParser::parse(a["target"].as_str().unwrap())),
            _ => {
                let result = source_replay::replay(row);
                assert!(
                    source_replay::kinds_match(row, &result),
                    "{case}#{i} {api} result variant"
                );
                assert_eq!(
                    source_replay::canonical(result, ""),
                    source_replay::canonical(source_replay::expected_output(row), ""),
                    "{case}#{i} {api}"
                );
                continue;
            }
        };
        assert_eq!(actual, row["output"], "{case}#{i} {api}");
    }
}
