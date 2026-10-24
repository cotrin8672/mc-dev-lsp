//! Exact legacy Local/Share inputs and whole typed results, executed without a JVM.
#[allow(dead_code)]
#[path = "../mixin_completion/replay.rs"]
mod completion_replay;
#[allow(dead_code)]
#[path = "../expression_engine_replay.rs"]
mod engine_replay;
#[path = "../mixin_source/replay.rs"]
mod source_replay;
use mcdev_core::{
    bytecode::{AtTargetCandidate, BytecodeIndex},
    expression_engine::*,
    extras_model::*,
    mixin_completion::*,
    mixinextras_locals::*,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::Arc};
fn parse<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("decode {e}: {v}"))
}
fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap()
}
fn snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase())
        } else {
            out.push(c)
        }
    }
    out
}
fn pascal(s: &str) -> String {
    s.split('_')
        .map(|s| {
            let mut c = s.chars();
            c.next()
                .map(|h| h.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        })
        .collect()
}
fn tagged(s: &str) -> bool {
    [
        "Local",
        "Share",
        "Cancellable",
        "Before",
        "After",
        "By",
        "Unresolved",
        "Resolved",
        "NotFound",
        "Ambiguous",
        "Unavailable",
        "Success",
        "Failure",
        "CorruptClassBytes",
        "MethodNotFound",
        "Analysis",
        "InvalidInstructionIndex",
    ]
    .contains(&s)
}
fn model(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(model).collect()),
        Value::Object(m) => {
            if let Some(variant) = m.get("variant").and_then(Value::as_str) {
                let mut m = m.clone();
                m.remove("variant");
                return json!({variant:model(&Value::Object(m))});
            }
            Value::Object(
                m.iter()
                    .map(|(k, v)| {
                        let value = if k == "targetMethod" {
                            v.clone()
                        } else if k == "annotation" {
                            json!(pascal(v.as_str().unwrap()))
                        } else {
                            model(v)
                        };
                        (if tagged(k) { k.clone() } else { snake(k) }, value)
                    })
                    .collect(),
            )
        }
        _ => v.clone(),
    }
}
fn canonical(v: Value, key: &str) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.into_iter().map(|v| canonical(v, key)).collect()),
        Value::Object(m) => Value::Object(
            m.into_iter()
                .map(|(k, v)| {
                    let key = if tagged(&k) { k } else { snake(&k) };
                    let v = if key == "annotation" {
                        v.as_str().map(|s| json!(pascal(s))).unwrap_or(v)
                    } else {
                        canonical(v, &key)
                    };
                    (key, v)
                })
                .collect(),
        ),
        _ => v,
    }
}
fn local_spec(v: &Value) -> LocalSugarSpec {
    let value = model(v);
    parse(value.get("Local").unwrap_or(&value))
}
fn engine_shape(v: &Value) -> Value {
    match v {
        Value::Object(m) if m.len() == 1 && m.contains_key("Local") => engine_shape(&m["Local"]),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), engine_shape(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(engine_shape).collect()),
        _ => v.clone(),
    }
}
struct RecordedBytecode<'a> {
    reads: &'a [Value],
}
impl RecordedBytecode<'_> {
    fn read(&self, api: &str, args: Value) -> &Value {
        &self
            .reads
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .unwrap_or_else(|| panic!("unrecorded {api} {args}"))["output"]
    }
}
impl BytecodeIndex for RecordedBytecode<'_> {
    fn get_at_target_candidates(
        &self,
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        at: &str,
    ) -> Vec<AtTargetCandidate> {
        self.read(
            "BytecodeIndex.getAtTargetCandidates",
            json!([owner, name, descriptor, at]),
        )
        .as_array()
        .unwrap()
        .iter()
        .map(completion_replay::candidate)
        .collect()
    }
    fn get_return_ordinal_count(&self, owner: &str, name: &str, descriptor: Option<&str>) -> usize {
        self.read(
            "BytecodeIndex.getReturnOrdinalCount",
            json!([owner, name, descriptor]),
        )
        .as_u64()
        .unwrap() as usize
    }
    fn get_class_bytes(&self, owner: &str) -> Option<Vec<u8>> {
        byte_array(self.read("BytecodeIndex.getClassBytes", json!([owner])))
    }
    fn resolve_common_super_class(&self, left: &str, right: &str) -> Option<String> {
        self.read(
            "BytecodeIndex.resolveCommonSuperClass",
            json!([left, right]),
        )
        .as_str()
        .map(str::to_owned)
    }
}
fn byte_array(v: &Value) -> Option<Vec<u8>> {
    v.as_array()
        .map(|a| a.iter().map(|v| v.as_i64().unwrap() as u8).collect())
}
fn extraction(row: &Value) -> Value {
    let a = &row["args"];
    let bytes = byte_array(&a["classBytes"]);
    let indices = parse::<BTreeSet<i32>>(&a["instructionOccurrenceIndices"]);
    let result = LocalCaptureExtractor::extract(
        bytes.as_deref(),
        text(a, "methodName"),
        text(a, "methodDescriptor"),
        &indices,
    )
    .unwrap();
    json!(result)
}
fn error_kind(v: &Value) -> Option<&str> {
    let error = v.get("Failure")?.get("error")?;
    error
        .as_str()
        .or_else(|| error.as_object()?.keys().next().map(String::as_str))
}
fn normalize_backend_error(mut v: Value) -> Value {
    if let Some(kind) = error_kind(&v).map(str::to_owned) {
        if ["CorruptClassBytes", "Analysis"].contains(&kind.as_str()) {
            v["Failure"]["error"][kind]["detail"] = json!("backend diagnostic detail");
        }
    }
    v
}
fn replay(row: &Value) -> Value {
    let a = &row["args"];
    let reads = row["reads"].as_array().unwrap();
    let index = completion_replay::RecordedIndex { reads };
    let bytecode = RecordedBytecode { reads };
    match text(row, "api") {
        "LocalCaptureExtractor.extract" => extraction(row),
        "LocalDiscriminatorResolver.resolve" => json!(LocalDiscriminatorResolver::resolve(
            &local_spec(&a["spec"]),
            text(a, "targetDescriptor"),
            &parse::<Vec<LocalCaptureCandidate>>(&model(&a["candidates"]))
        )),
        "LocalCaptureTypeResolver.resolveParameter" => json!(LocalCaptureTypeResolver::new(&index)
            .resolve_parameter(
                text(a, "source"),
                &parse::<HandlerParameterDeclaration>(&model(&a["parameter"]))
            )),
        "LocalCaptureTypeResolver.resolveDefinition" => {
            json!(LocalCaptureTypeResolver::new(&index)
                .resolve_definition(text(a, "source"), &local_spec(&a["spec"])))
        }
        "LocalCompletionService.complete" => json!(LocalCompletionService::new(&index).complete(
            text(a, "source"),
            &parse::<HandlerParameterDeclaration>(&model(&a["parameter"])),
            &parse::<Vec<LocalCaptureValidationResult>>(&model(&a["results"])),
            text(a, "attributeName"),
            text(a, "partialPrefix")
        )),
        "LocalCaptureValidationService.validate" => {
            json!(LocalCaptureValidationService::new(&index, &bytecode)
                .validate(
                    text(a, "source"),
                    &parse::<HandlerParameterDeclaration>(&model(&a["parameter"])),
                    &parse::<Vec<LocalCaptureValidationPoint>>(&model(&a["points"]))
                )
                .unwrap())
        }
        "MixinExtrasCompletionService.complete" => json!(MixinExtrasCompletionService::new(
            &index, &bytecode
        )
        .complete(
            &completion_replay::context(&a["context"]),
            &parse::<MixinCompletionOptions>(&a["options"])
        )),
        "MixinExtrasCompletionService.completeAtTargetForWrapOperation" => {
            json!(MixinExtrasCompletionService::new(&index, &bytecode)
                .complete_at_target_for_wrap_operation(
                    &parse::<Vec<String>>(&a["mixinTargets"]),
                    text(a, "injectMethodName"),
                    a["injectMethodDescriptor"].as_str(),
                    &completion_replay::context(&a["context"])
                ))
        }
        "ShareCompletionService.complete" => {
            let sources = reads
                .iter()
                .filter(|r| r["api"] == "ShareSources.next")
                .map(|r| r["output"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            let count = std::cell::Cell::new(0);
            let expected = sources.len();
            let result = ShareCompletionService::new(&index).complete(
                text(a, "source"),
                &completion_replay::context(&a["context"]),
                sources.into_iter().inspect(|_| count.set(count.get() + 1)),
            );
            assert_eq!(
                count.get(),
                expected,
                "Share additional source iteration count"
            );
            json!(result)
        }
        unknown => panic!("unknown local API {unknown}"),
    }
}
#[derive(Debug)]
struct Marker;
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("original local capture cancellation")
    }
}
impl std::error::Error for Marker {}
fn cancellation(row: &Value) {
    let a = &row["args"];
    let bytes = byte_array(&a["classBytes"]);
    let indices = parse::<BTreeSet<i32>>(&a["instructionOccurrenceIndices"]);
    let marker: Arc<dyn std::error::Error + Send + Sync> = Arc::new(Marker);
    struct Checker(Arc<dyn std::error::Error + Send + Sync>);
    impl CancellationChecker for Checker {
        fn check_cancelled(&self) -> Result<(), Cancellation> {
            Ok(())
        }
        fn check_phase(&self, phase: CancellationPhase) -> Result<(), Cancellation> {
            if phase == CancellationPhase::LocalAnalysis {
                Err(Cancellation(self.0.clone()))
            } else {
                Ok(())
            }
        }
    }
    let result = LocalCaptureExtractor::extract_with_cancellation(
        bytes.as_deref(),
        text(a, "methodName"),
        text(a, "methodDescriptor"),
        &indices,
        &Checker(marker.clone()),
    );
    let Err(error) = result else {
        panic!("cancellation swallowed")
    };
    assert!(Arc::ptr_eq(&marker, &error.0));
    assert!(matches!(
        LocalCaptureExtractor::extract(
            bytes.as_deref(),
            text(a, "methodName"),
            text(a, "methodDescriptor"),
            &indices
        )
        .unwrap(),
        LocalCaptureResult::Success { .. }
    ));
}
pub fn replay_case(case: &str) {
    let rows = include_str!("legacy_calls.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|r| r["legacyCase"] == case)
        .collect::<Vec<_>>();
    assert!(!rows.is_empty(), "uncaptured {case}");
    for (i, row) in rows.iter().enumerate() {
        let api = text(row, "api");
        if api == "OfficialExpressionMatcher.match" {
            engine_replay::replay_match_request(&engine_shape(row))
                .unwrap_or_else(|e| panic!("{case}#{i} {api}: {e}"));
            continue;
        }
        if api == "OfficialExpressionIdentifierPoolBuilder.build" {
            engine_replay::replay_build_request(&engine_shape(row))
                .unwrap_or_else(|e| panic!("{case}#{i} {api}: {e}"));
            continue;
        }
        let module = api.split('.').next().unwrap();
        if [
            "AnnotationContextExtractor",
            "JavaTypeDescriptorResolver",
            "MixinMemberDeclarationParser",
            "MixinSemanticModelParser",
            "MixinTargetResolver",
            "SemanticCompletionContextExtractor",
            "JavaStringContentDecoder",
        ]
        .contains(&module)
        {
            let actual = source_replay::replay(row);
            assert!(
                source_replay::kinds_match(row, &actual),
                "{case}#{i} {api} variant"
            );
            assert_eq!(
                source_replay::canonical(actual, ""),
                source_replay::canonical(source_replay::expected_output(row), ""),
                "{case}#{i} {api}"
            );
            continue;
        }
        if ["InjectMethodCompletionService", "AtTargetCompletionService"].contains(&module) {
            continue;
        }
        if row.get("throwable").is_some() {
            assert_eq!(api, "LocalCaptureExtractor.extract");
            cancellation(row);
            continue;
        }
        let actual = replay(row);
        let expected = model(&row["output"]);
        let expected = normalize_backend_error(expected);
        let actual = normalize_backend_error(actual);
        assert_eq!(
            canonical(actual, ""),
            canonical(expected, ""),
            "{case}#{i} {api}"
        );
    }
}

/// Callee evidence only: these do not mark the enclosing Facade cases ported.
#[allow(dead_code)]
pub fn replay_share_service(case_name: &str) {
    let rows = include_str!("legacy_calls.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|r| {
            r["legacyCase"].as_str().unwrap().ends_with(case_name)
                && r["api"] == "ShareCompletionService.complete"
        })
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    for row in rows {
        assert_eq!(
            canonical(replay(&row), ""),
            canonical(model(&row["output"]), ""),
            "Share service result {case_name}"
        );
    }
}

#[allow(dead_code)]
pub fn replay_additional_case(case_name: &str) {
    let rows = include_str!("additional_calls.jsonl")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|row| row["legacyCase"].as_str().unwrap().ends_with(case_name))
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row["api"], "LocalCaptureExtractor.extract");
        assert_eq!(
            canonical(extraction(row), ""),
            canonical(model(&row["output"]), ""),
            "{case_name}#{i} frame"
        );
    }
}
