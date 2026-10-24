//! Full public service outputs and applied documents from the frozen 133-case oracle.
use mcdev_core::{
    bytecode::*, extras_model::*, mixinextras_actions::*, mixinextras_diagnostics::*,
    mixinextras_handler::*, mixinextras_operation::*, *,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn snake(name: &str) -> String {
    let mut result = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            result.push('_');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect()
}
fn model(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let output = if key == "handlerParameterDescriptors" {
                        value.clone()
                    } else if [
                        "annotation",
                        "kind",
                        "operationKind",
                        "occurrenceResultClassification",
                    ]
                    .contains(&key.as_str())
                        && value.is_string()
                    {
                        json!(pascal(value.as_str().unwrap()))
                    } else {
                        model(value)
                    };
                    let key = if key.chars().next().is_some_and(char::is_uppercase) {
                        key.clone()
                    } else {
                        snake(key)
                    };
                    (key, output)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(model).collect()),
        other => other.clone(),
    }
}
fn decode<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(model(value)).unwrap_or_else(|e| panic!("typed fixture {e}: {value}"))
}
fn output<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap()
}
struct Reads<'a> {
    rows: &'a [Value],
    calls: std::sync::Mutex<Vec<Value>>,
}
fn handler_service<'a>(reads: &'a Reads<'a>, args: &Value) -> HandlerSignatureService<'a> {
    let has_bytecode = args["hasBytecodeIndex"]
        .as_bool()
        .expect("original constructor capability required");
    HandlerSignatureService::new(reads, has_bytecode.then_some(reads as &dyn BytecodeIndex))
        .with_reference_types(reads)
}
impl HandlerReferenceTypeResolver for Reads<'_> {
    fn is_known_reference(&self, name: &str) -> Option<bool> {
        reference_facts()["known"][name].as_bool()
    }
    fn is_assignable(&self, from: &str, expected: &str) -> Option<bool> {
        reference_facts()["assignable"][format!("{from}|{expected}")].as_bool()
    }
}
fn reference_facts() -> &'static Value {
    static FACTS: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    FACTS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "fixtures/mixinextras_services/host-reference-facts.json"
        ))
        .unwrap()
    })
}
fn utf16_offset(source: &str, units: usize) -> Option<usize> {
    let mut current = 0;
    for (byte, ch) in source.char_indices() {
        if current == units {
            return Some(byte);
        }
        current += ch.len_utf16();
    }
    if current == units {
        Some(source.len())
    } else {
        None
    }
}
impl Reads<'_> {
    fn get(&self, api: &str, args: Value) -> &Value {
        let mut calls = self.calls.lock().unwrap();
        let occurrence = calls
            .iter()
            .filter(|r| r["api"] == api && r["args"] == args)
            .count();
        calls.push(json!({"api":api,"args":args}));
        drop(calls);
        &self
            .rows
            .iter()
            .filter(|r| r["api"] == api && r["args"] == args)
            .nth(occurrence)
            .unwrap_or_else(|| panic!("uncaptured read {api} {args}"))["output"]
    }
    fn assert_trace(&self) {
        let actual = self.calls.lock().unwrap();
        let expected: Vec<_> = self
            .rows
            .iter()
            .map(|r| json!({"api":r["api"],"args":r["args"]}))
            .collect();
        assert!(
            *actual == expected,
            "callback trace mismatch actual={} expected={}",
            serde_json::to_string(&*actual).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
    }
}
impl ClassIndex for Reads<'_> {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        serde_json::from_value(
            self.get("ClassIndex.findClasses", json!([prefix, limit]))
                .clone(),
        )
        .unwrap()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.findClass", json!([name])).clone()).unwrap()
    }
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.findClassByFqn", json!([fqn])).clone()).unwrap()
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.getMethods", json!([owner])).clone()).unwrap()
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        serde_json::from_value(self.get("ClassIndex.getFields", json!([owner])).clone()).unwrap()
    }
}
impl BytecodeIndex for Reads<'_> {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        at_value: &str,
    ) -> Vec<AtTargetCandidate> {
        decode(self.get(
            "BytecodeIndex.getAtTargetCandidates",
            json!([owner, name, descriptor, at_value]),
        ))
    }
    fn get_return_ordinal_count(&self, owner: &str, name: &str, descriptor: Option<&str>) -> usize {
        self.get(
            "BytecodeIndex.getReturnOrdinalCount",
            json!([owner, name, descriptor]),
        )
        .as_u64()
        .unwrap() as usize
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        let value = self.get("BytecodeIndex.getClassBytes", json!([owner]));
        (!value.is_null()).then(|| {
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as u8)
                .collect()
        })
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.get(
            "BytecodeIndex.resolveCommonSuperClass",
            json!([left, right]),
        )
        .as_str()
        .map(str::to_owned)
    }
}
impl OperationCallArgumentTypeResolver for Reads<'_> {
    fn resolve_type(&self, expression: &str) -> Option<String> {
        self.get(
            "OperationCallArgumentTypeResolver.resolveType",
            json!([expression]),
        )
        .as_str()
        .map(str::to_owned)
    }
}
fn fixes(value: &Value, source: &str) -> Vec<McFix> {
    let mut value = value.clone();
    for fix in value.as_array_mut().unwrap() {
        normalize_fix(fix, source);
    }
    serde_json::from_value(value).unwrap()
}
fn normalize_fix(value: &mut Value, source: &str) {
    if let Some(edits) = value["edits"].as_array_mut() {
        for edit in edits {
            for key in ["startOffset", "endOffset"] {
                if let Some(units) = edit[key].as_u64() {
                    if let Some(byte) = utf16_offset(source, units as usize) {
                        edit[key] = json!(byte);
                    }
                }
            }
        }
    }
}
fn source_for(row: &Value) -> &str {
    row["args"]["source"]
        .as_str()
        .or_else(|| row["args"]["request"]["source"].as_str())
        .or_else(|| row["args"]["currentSource"].as_str())
        .unwrap_or("")
}
fn replay_service(row: &Value) -> (Value, Value) {
    let api = row["api"].as_str().unwrap();
    let args = &row["args"];
    let source = source_for(row);
    let reads = Reads {
        rows: row["reads"].as_array().unwrap(),
        calls: std::sync::Mutex::new(vec![]),
    };
    let targets: Vec<String> = args.get("mixinTargets").map(decode).unwrap_or_default();
    let contexts: Vec<ResolvedMixinExtrasContext> =
        args.get("resolvedContexts").map(decode).unwrap_or_default();
    let actual = match api {
        "MixinExtrasCodeActionService.completeHandler" => {
            let Some(start) = args["annotationStartOffset"]
                .as_u64()
                .and_then(|n| utf16_offset(source, n as usize))
            else {
                return (json!([]), json!([]));
            };
            let cursor = args["cursorOffset"]
                .as_u64()
                .and_then(|n| utf16_offset(source, n as usize));
            let actual = MixinExtrasCodeActionService::new(&reads)
                .with_signature_service(handler_service(&reads, args))
                .complete_handler(source, start, &targets, &contexts, cursor);
            reads.assert_trace();
            let mut expected = row["output"].clone();
            for item in expected.as_array_mut().unwrap() {
                if let Some(edits) = item["additionalEdits"].as_array_mut() {
                    for edit in edits {
                        for key in ["startOffset", "endOffset"] {
                            if let Some(units) = edit[key].as_u64() {
                                edit[key] = json!(utf16_offset(source, units as usize).unwrap());
                            }
                        }
                    }
                }
            }
            let expected: Vec<McCompletionItem> = serde_json::from_value(expected).unwrap();
            return (output(actual), output(expected));
        }
        "MixinExtrasCodeActionService.applyHandlerFix" => {
            let mut fix = args["fix"].clone();
            normalize_fix(&mut fix, source);
            let fix: McFix = serde_json::from_value(fix).unwrap();
            let actual = MixinExtrasCodeActionService::new(&reads)
                .with_signature_service(handler_service(&reads, args))
                .apply_handler_fix(&fix, source);
            reads.assert_trace();
            let mut expected = row["output"].clone();
            normalize_fix(&mut expected, source);
            let expected: McFix = serde_json::from_value(expected).unwrap();
            return (output(actual), output(expected));
        }
        "MixinExtrasDiagnosticsService.analyze" => {
            let request = &args["request"];
            let input = MixinExtrasDiagnosticRequest {
                source: source.into(),
                document_uri: request["documentUri"].as_str().unwrap().into(),
                resolved_contexts: decode(&request["resolvedContexts"]),
            };
            let service = MixinExtrasDiagnosticsService::new(&reads, &reads)
                .with_signature_service(handler_service(&reads, args));
            let actual = service.analyze(&input);
            reads.assert_trace();
            let expected: Vec<McDiagnostic> =
                serde_json::from_value(row["output"].clone()).unwrap();
            return (output(actual), output(expected));
        }
        "MixinExtrasCodeActionService.deduplicateFixes" => {
            MixinExtrasCodeActionService::deduplicate_fixes(&fixes(&args["fixes"], source))
        }
        "MixinExtrasCodeActionService.fixesForDiagnostics" => {
            let diagnostics =
                serde_json::from_value::<Vec<McDiagnostic>>(args["diagnostics"].clone()).unwrap();
            MixinExtrasCodeActionService::new(&reads)
                .with_signature_service(handler_service(&reads, args))
                .fixes_for_diagnostics(
                    &diagnostics,
                    args["documentUri"].as_str().unwrap(),
                    source,
                    &contexts,
                )
        }
        "MixinExtrasCodeActionService.generateHandlerFixes" => {
            let site = decode(&args["site"]);
            MixinExtrasCodeActionService::new(&reads)
                .with_signature_service(handler_service(&reads, args))
                .generate_handler_fixes(
                    args["documentUri"].as_str().unwrap(),
                    source,
                    &site,
                    &targets,
                    &contexts,
                )
        }
        "MixinExtrasCodeActionService.generateHandlerFixesAtPosition" => {
            let line = args["line"].as_u64();
            let character = args["character"].as_u64();
            match (line, character) {
                (Some(line), Some(character)) => MixinExtrasCodeActionService::new(&reads)
                    .with_signature_service(handler_service(&reads, args))
                    .generate_handler_fixes_at_position(
                        args["documentUri"].as_str().unwrap(),
                        source,
                        line as usize,
                        character as usize,
                        &targets,
                        &contexts,
                    ),
                _ => vec![],
            }
        }
        other => panic!("unimplemented service API {other}"),
    };
    reads.assert_trace();
    let expected = fixes(&row["output"], source);
    for (actual, expected) in actual.iter().zip(&expected) {
        if let (
            McFix::WorkspaceEditFix { edits: a, .. },
            McFix::WorkspaceEditFix { edits: e, .. },
        ) = (actual, expected)
        {
            assert_eq!(
                apply_text_edits(source, a),
                apply_text_edits(source, e),
                "actual applied document"
            );
        }
    }
    (output(actual), output(expected))
}
#[test]
fn additional_public_completion_and_legacy_materialization_outputs_match() {
    let calls = include_str!("fixtures/mixinextras_services/caller-calls.jsonl");
    let mut count = 0;
    for text in calls.lines() {
        let row: Value = serde_json::from_str(text).unwrap();
        if !row["api"].as_str().unwrap().starts_with("MixinExtras") {
            continue;
        }
        count += 1;
        let (actual, expected) = replay_service(&row);
        assert_eq!(actual, expected, "{} {}", row["legacyCase"], row["api"]);
    }
    assert_eq!(count, 8);
}
#[test]
fn every_service_public_output_and_applied_document_matches_the_frozen_oracle() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mixinextras_services");
    let calls = std::fs::read_to_string(path.join("services-calls.jsonl")).unwrap();
    let mut failures = vec![];
    let mut cases = BTreeSet::new();
    let mut count = 0;
    for (line, text) in calls.lines().enumerate() {
        let row: Value = serde_json::from_str(text).unwrap();
        if !row["api"].as_str().unwrap().starts_with("MixinExtras") {
            continue;
        }
        count += 1;
        cases.insert(row["legacyCase"].as_str().unwrap().to_string());
        match std::panic::catch_unwind(||replay_service(&row)){Ok((actual,expected))if actual==expected=>{},Ok((actual,expected))=>failures.push(json!({"line":line+1,"case":row["legacyCase"],"api":row["api"],"actual":actual,"expected":expected})),Err(error)=>failures.push(json!({"line":line+1,"case":row["legacyCase"],"api":row["api"],"panic":error.downcast_ref::<String>().cloned().or_else(||error.downcast_ref::<&str>().map(|s|s.to_string())).unwrap_or_default()}))}
    }
    let log = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/rust-test-logs/extras-service-failures.json");
    std::fs::write(&log, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
    assert!(
        failures.is_empty(),
        "{} failures across {count} full service outputs/{} cases; {}",
        failures.len(),
        cases.len(),
        log.display()
    );
    assert_eq!(count, 249);
    assert_eq!(cases.len(), 128);
    let manifest: Vec<Value> = serde_json::from_str(include_str!(
        "fixtures/mixinextras_services/services-cases.json"
    ))
    .unwrap();
    assert_eq!(manifest.len(), 133);
    let direct = [
        "selectorFallsBackToUniqueHandlerOverlap",
        "selectorIgnoresHandlerRangesThatOnlyTouchAtEndpoint",
        "selectorPrefersUniqueAnnotationContainment",
        "selectorReturnsNullWhenAnnotationMatchesAreAmbiguous",
        "selectorReturnsNullWhenHandlerOverlapIsAmbiguous",
    ];
    for case in manifest {
        assert_eq!(case["status"], "passed");
        let id = case["legacyCase"].as_str().unwrap();
        assert!(
            cases.contains(id) || direct.iter().any(|name| id.ends_with(name)),
            "unexecuted exact legacycase {id}"
        );
    }
}
fn range(a: (usize, usize), b: (usize, usize)) -> McTextRange {
    McTextRange {
        start: McTextPosition {
            line: a.0,
            character: a.1,
        },
        end: McTextPosition {
            line: b.0,
            character: b.1,
        },
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
            parameters: vec![],
            range,
            is_static: false,
            has_method_level_coerce: false,
        }),
        expand_zero_conditions: BTreeSet::new(),
    }
}
fn context(handler_range: McTextRange) -> ResolvedMixinExtrasContext {
    ResolvedMixinExtrasContext {
        handler_range,
        context: ExpressionContext::default(),
    }
}
/// MixinExtrasDiagnosticsServiceTest::selectorFallsBackToUniqueHandlerOverlap
#[test]
fn selector_falls_back_to_unique_handler_overlap() {
    let site = site(range((1, 0), (1, 10)), Some(range((3, 4), (3, 40))));
    let matches = [context(range((3, 0), (3, 50)))];
    assert_eq!(
        select_resolved_mixin_extras_context(&site, &matches),
        Some(&matches[0])
    );
}
/// MixinExtrasDiagnosticsServiceTest::selectorIgnoresHandlerRangesThatOnlyTouchAtEndpoint
#[test]
fn selector_ignores_handler_ranges_that_only_touch_at_endpoint() {
    let site = site(range((0, 0), (0, 1)), Some(range((3, 4), (3, 40))));
    let start = context(range((3, 0), (3, 4)));
    let end = context(range((3, 40), (3, 50)));
    assert!(select_resolved_mixin_extras_context(&site, &[start.clone()]).is_none());
    assert!(select_resolved_mixin_extras_context(&site, &[end.clone()]).is_none());
    assert!(select_resolved_mixin_extras_context(&site, &[start, end]).is_none());
}
/// MixinExtrasDiagnosticsServiceTest::selectorPrefersUniqueAnnotationContainment
#[test]
fn selector_prefers_unique_annotation_containment() {
    let site = site(range((2, 4), (2, 20)), Some(range((3, 4), (3, 40))));
    let matches = [
        context(range((2, 0), (4, 0))),
        context(range((3, 0), (4, 0))),
    ];
    assert_eq!(
        select_resolved_mixin_extras_context(&site, &matches),
        Some(&matches[0])
    );
}
/// MixinExtrasDiagnosticsServiceTest::selectorReturnsNullWhenAnnotationMatchesAreAmbiguous
#[test]
fn selector_returns_null_when_annotation_matches_are_ambiguous() {
    let site = site(range((2, 4), (2, 20)), None);
    let matches = [
        context(range((2, 0), (4, 0))),
        context(range((2, 2), (4, 2))),
    ];
    assert!(select_resolved_mixin_extras_context(&site, &matches).is_none());
}
/// MixinExtrasDiagnosticsServiceTest::selectorReturnsNullWhenHandlerOverlapIsAmbiguous
#[test]
fn selector_returns_null_when_handler_overlap_is_ambiguous() {
    let site = site(range((0, 0), (0, 1)), Some(range((3, 4), (3, 40))));
    let matches = [
        context(range((3, 0), (3, 20))),
        context(range((3, 10), (3, 50))),
    ];
    assert!(select_resolved_mixin_extras_context(&site, &matches).is_none());
}
fn apply_snapshot(source: &str, current: &str, edits: &[McTextEdit]) -> Option<String> {
    if source != current {
        return None;
    }
    apply_text_edits(current, edits).ok()
}
#[test]
fn checked_application_rejects_stale_snapshot_invalid_boundaries_and_overlaps() {
    let edit = |start, end, text: &str| McTextEdit {
        start_offset: start,
        end_offset: end,
        new_text: text.into(),
    };
    assert_eq!(
        apply_snapshot("abc", "abc", &[edit(1, 2, "X")]),
        Some("aXc".into())
    );
    assert_eq!(apply_snapshot("abc", "abd", &[edit(1, 2, "X")]), None);
    assert_eq!(
        apply_snapshot("abc", "abc", &[edit(0, 2, "X"), edit(1, 3, "Y")]),
        None
    );
    assert_eq!(apply_snapshot("😀", "😀", &[edit(1, 2, "X")]), None);
    assert_eq!(apply_snapshot("abc", "abc", &[edit(0, 10, "X")]), None);
}
#[test]
fn expression_cancellation_propagates_through_diagnostics() {
    use mcdev_core::expression_engine::{with_cancellation, Cancellation};
    let row = include_str!("fixtures/mixinextras_services/services-calls.jsonl")
        .lines()
        .map(|text| serde_json::from_str::<Value>(text).unwrap())
        .find(|r| {
            r["api"] == "MixinExtrasDiagnosticsService.analyze"
                && r["reads"].as_array().unwrap().iter().any(|read| {
                    read["api"] == "BytecodeIndex.getClassBytes"
                        && read["output"]
                            .as_array()
                            .is_some_and(|bytes| !bytes.is_empty())
                })
                && r["args"]["request"]["source"]
                    .as_str()
                    .unwrap()
                    .contains("MIXINEXTRAS:EXPRESSION")
        })
        .unwrap();
    let request = &row["args"]["request"];
    let reads = Reads {
        rows: row["reads"].as_array().unwrap(),
        calls: std::sync::Mutex::new(vec![]),
    };
    let request = MixinExtrasDiagnosticRequest {
        source: request["source"].as_str().unwrap().into(),
        document_uri: request["documentUri"].as_str().unwrap().into(),
        resolved_contexts: decode(&request["resolvedContexts"]),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_cancellation(
            std::sync::Arc::new(|| {
                Err(Cancellation(std::sync::Arc::new(std::io::Error::other(
                    "cancelled",
                ))))
            }),
            || {
                MixinExtrasDiagnosticsService::new(&reads, &reads)
                    .with_signature_service(handler_service(&reads, &row["args"]))
                    .analyze(&request)
            },
        )
    }));
    let error = result.expect_err("cancellation must not become empty diagnostics");
    assert!(
        error.is::<Cancellation>(),
        "typed cancellation must survive"
    );
}
