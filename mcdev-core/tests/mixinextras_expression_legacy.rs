use mcdev_core::{
    bytecode::*, expression_engine as engine, extras_model::*, mixin_source::*,
    mixinextras_expression::*, *,
};
use serde_json::{json, Value};
#[path = "fixtures/mixin_source/replay.rs"]
#[allow(dead_code)]
mod source_replay;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
fn snake(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
fn pascal(s: &str) -> String {
    s.split('_')
        .map(|p| {
            let mut c = p.chars();
            c.next().map_or(String::new(), |f| {
                f.to_ascii_uppercase().to_string() + &c.as_str().to_ascii_lowercase()
            })
        })
        .collect()
}
fn input(value: Value, key: &str) -> Value {
    match value {
        Value::Object(map) => {
            if let (Some(first), Some(last)) = (
                map.get("first").and_then(Value::as_i64),
                map.get("last").and_then(Value::as_i64),
            ) {
                return json!({"start":first,"end":last+1});
            }
            if key == "localSpecs" && map.len() == 1 && map.contains_key("Local") {
                return input(map["Local"].clone(), "");
            }
            if matches!(key, "explicit" | "ambiguousExplicit" | "metadata") {
                return Value::Object(map);
            }
            Value::Object(
                map.into_iter()
                    .map(|(k, v)| (snake(&k), input(v, &k)))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.into_iter().map(|v| input(v, key)).collect()),
        Value::String(s)
            if matches!(
                key,
                "annotation"
                    | "slot"
                    | "parentInjectorAnnotation"
                    | "expressionCompletionPosition"
                    | "kind"
                    | "atShift"
                    | "operationKind"
                    | "occurrenceResultClassification"
            ) =>
        {
            json!(pascal(&s))
        }
        other => other,
    }
}
fn de<T: serde::de::DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone())
        .or_else(|_| serde_json::from_value(input(v.clone(), "")))
        .unwrap()
}
fn constraint(value: &Value) -> engine::TypeConstraint {
    match value["kind"].as_str() {
        Some("IntLike") => engine::TypeConstraint::IntLike,
        Some("Exact") => {
            engine::TypeConstraint::Exact(value["fields"]["descriptor"].as_str().unwrap().into())
        }
        _ if value.get("descriptor").is_some() => {
            engine::TypeConstraint::Exact(value["descriptor"].as_str().unwrap().into())
        }
        _ => panic!("unrecorded constraint variant {value}"),
    }
}
fn candidate(value: &Value) -> engine::MemberCandidate {
    let decorations = value["decorations"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, v)| match key.as_str() {
            "SIMPLE_EXPRESSION_TYPE" => (
                engine::DecorationKey::SimpleExpressionType,
                engine::DecorationValue::TypeConstraint(constraint(&v["constraint"])),
            ),
            "SIMPLE_OPERATION_ARGS" => (
                engine::DecorationKey::SimpleOperationArgs,
                engine::DecorationValue::TypeConstraints(
                    v["constraints"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(constraint)
                        .collect(),
                ),
            ),
            "SIMPLE_OPERATION_PARAM_NAMES" => (
                engine::DecorationKey::SimpleOperationParamNames,
                engine::DecorationValue::ParamNames(de(&v["names"])),
            ),
            "SIMPLE_OPERATION_RETURN_TYPE" => (
                engine::DecorationKey::SimpleOperationReturnType,
                engine::DecorationValue::TypeConstraint(constraint(&v["constraint"])),
            ),
            "IS_STRING_CONCAT_EXPRESSION" => (
                engine::DecorationKey::IsStringConcatExpression,
                engine::DecorationValue::Flag(v["value"].as_bool().unwrap()),
            ),
            _ => panic!("unknown decoration {key}"),
        })
        .collect();
    engine::MemberCandidate {
        kind: if value["kind"] == "FIELD" {
            engine::MemberKind::Field
        } else {
            assert_eq!(value["kind"], "METHOD");
            engine::MemberKind::Method
        },
        owner_internal_name: de(&value["ownerInternalName"]),
        name: de(&value["name"]),
        descriptor: de(&value["descriptor"]),
        is_interface: value["isInterface"].as_bool().unwrap_or(false),
        original_instruction_opcode: value["originalInstructionOpcode"].as_u64().unwrap() as u8,
        original_instruction_index: value["originalInstructionIndex"].as_u64().unwrap() as usize,
        decorations,
    }
}
fn ranges(v: Value, key: &str) -> Value {
    match v {
        Value::Object(map) => {
            if let (Some(first), Some(last)) = (
                map.get("first").and_then(Value::as_i64),
                map.get("last").and_then(Value::as_i64),
            ) {
                return json!({"start":first,"end":last+1});
            }
            if key == "localSpecs" && map.len() == 1 && map.contains_key("Local") {
                return ranges(map["Local"].clone(), "");
            }
            Value::Object(
                map.into_iter()
                    .map(|(k, v)| {
                        let v = ranges(v, &k);
                        (k, v)
                    })
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.into_iter().map(|v| ranges(v, key)).collect()),
        Value::String(s)
            if matches!(
                key,
                "position" | "kind" | "resultClassification" | "targetKind"
            ) =>
        {
            json!(s.replace('_', "").to_lowercase())
        }
        other => other,
    }
}
fn canonical(v: Value) -> Value {
    ranges(source_replay::canonical(v, ""), "")
}
fn normalized_row(row: &Value) -> Value {
    let mut row = row.clone();
    let args = row["args"].clone();
    let source_hint = row.get("sourceHint").and_then(Value::as_str);
    let (basis, origin) = if let Some(source) = source_hint {
        let origin = if row["api"] == "DefinitionsAnnotationParser.parseDefinitionAt"
            && args["source"].as_str() == Some(source)
        {
            args["parsedTextBaseOffset"].as_i64().unwrap_or(0)
        } else {
            0
        };
        (source.to_string(), origin)
    } else if let Some(source) = args.get("source").and_then(Value::as_str) {
        (
            source.to_string(),
            args.get("parsedTextBaseOffset")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        )
    } else if let Some(region) = args.get("handlerRegion").and_then(Value::as_str) {
        (
            region.to_string(),
            args["regionStartOffset"].as_i64().unwrap_or(0),
        )
    } else {
        (
            args["body"].as_str().unwrap_or("").to_string(),
            args["parsedTextBaseOffset"].as_i64().unwrap_or(0),
        )
    };
    fn offset(value: i64, source: &str, origin: i64) -> i64 {
        source_replay::byte(source, value - origin).map_or(value, |v| origin + v as i64)
    }
    fn range(value: &mut Value, source: &str, origin: i64) {
        if let (Some(first), Some(last)) = (value["first"].as_i64(), value["last"].as_i64()) {
            let start = offset(first, source, origin);
            let end = offset(last + 1, source, origin);
            value["first"] = json!(start);
            value["last"] = json!(end - 1);
        }
    }
    fn definitions(v: &mut Value, source: &str, origin: i64, fallback: &str) {
        match v {
            Value::Object(map) if map.contains_key("bodyLength") => {
                let raw = map.get("sourceRange").cloned().unwrap_or(Value::Null);
                let body = if let (Some(first), Some(last)) =
                    (raw["first"].as_i64(), raw["last"].as_i64())
                {
                    source_replay::byte(source, first - origin)
                        .zip(source_replay::byte(source, last + 1 - origin))
                        .and_then(|(a, b)| source.get(a..b))
                        .unwrap_or(fallback)
                } else {
                    fallback
                };
                map.insert("bodyLength".into(), json!(body.len()));
                if let Some(r) = map.get_mut("idContentRange") {
                    range(r, body, 0);
                }
                if let Some(a) = map.get_mut("parseIssues").and_then(Value::as_array_mut) {
                    for issue in a {
                        range(&mut issue["bodyRange"], body, 0);
                    }
                }
                for name in ["sourceRange", "idSourceRange"] {
                    if let Some(r) = map.get_mut(name) {
                        range(r, source, origin);
                    }
                }
            }
            Value::Object(map) => {
                for value in map.values_mut() {
                    definitions(value, source, origin, fallback);
                }
            }
            Value::Array(a) => {
                for value in a {
                    definitions(value, source, origin, fallback);
                }
            }
            _ => {}
        }
    }
    let fallback = args["body"].as_str().unwrap_or("");
    definitions(&mut row["args"], &basis, origin, fallback);
    definitions(&mut row["output"], &basis, origin, fallback);
    if origin == 0 {
        source_replay::normalize_expected_offsets(&mut row["args"], &basis, "");
        source_replay::normalize_expected_offsets(&mut row["output"], &basis, "");
    }
    if let Some(prefix) = args.get("expressionPrefix").and_then(Value::as_str) {
        for key in ["cursorInExpression"] {
            if let Some(n) = args[key].as_i64() {
                row["args"][key] = json!(offset(n, prefix, 0));
            }
        }
        for key in ["tokenStart", "tokenEnd"] {
            if let Some(n) = row["output"][key].as_i64() {
                row["output"][key] = json!(offset(n, prefix, 0));
            }
        }
    }
    if let Some(n) = args.get("offset").and_then(Value::as_i64) {
        row["args"]["offset"] = json!(offset(n, &basis, origin));
    }
    row
}
struct Reads<'a>(&'a [Value]);
struct CapturedRunner {
    reads: Vec<Value>,
}
impl ExpressionMemberMatcherRunner for CapturedRunner {
    fn complete_reachable_members(
        &self,
        bytes: &[u8],
        name: &str,
        descriptor: &str,
        receiver: &str,
        context: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation> {
        let kind = format!("{:?}", context.context_type);
        let mut screaming = String::new();
        for (i, c) in kind.chars().enumerate() {
            if i > 0 && c.is_ascii_uppercase() {
                screaming.push('_');
            }
            screaming.push(c.to_ascii_uppercase());
        }
        let args = json!([
            format!("{:x}", Sha256::digest(bytes)),
            name,
            descriptor,
            receiver,
            screaming
        ]);
        let row = self
            .reads
            .iter()
            .find(|r| {
                r["api"] == "ExpressionMemberMatcherRunner.completeReachableMembers"
                    && r["args"] == args
            })
            .unwrap_or_else(|| panic!("unexpected matcher callback {args}"));
        if row["outputType"].as_str().unwrap().ends_with("$Available") {
            Ok(engine::MemberCompletionResult::Available(
                row["output"]["candidates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(candidate)
                    .collect(),
            ))
        } else {
            Ok(engine::MemberCompletionResult::Unavailable {
                reason: row["output"]["reason"].as_str().unwrap().into(),
                cause: row["output"]["cause"].as_str().map(String::from),
            })
        }
    }
}
fn member_service<'a>(row: &Value, reads: &'a Reads<'_>) -> ExpressionMemberCompletionService<'a> {
    static CACHES: OnceLock<Mutex<BTreeMap<(String, i64), Arc<ExpressionMemberCompletionCache>>>> =
        OnceLock::new();
    let args = &row["args"];
    let key = (
        row["legacyCase"].as_str().unwrap().to_string(),
        args["__cacheInstance"]
            .as_i64()
            .unwrap_or_else(|| args["__instance"].as_i64().unwrap_or(0)),
    );
    let capacity = args["__cacheCapacity"].as_u64().unwrap_or(32) as usize;
    let cache = CACHES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(key)
        .or_insert_with(|| Arc::new(ExpressionMemberCompletionCache::new(capacity)))
        .clone();
    let mut service = ExpressionMemberCompletionService::new(reads, reads);
    service.reachable_members_cache = cache;
    if args["__customRunner"] == true {
        service.matcher_runner = Arc::new(CapturedRunner {
            reads: row["reads"].as_array().unwrap().clone(),
        });
    }
    service
}
fn member_configuration(row: &Value) -> Value {
    if row["api"] == "ExpressionMemberCompletionService.completeMembers" {
        return row.clone();
    }
    static ROWS: OnceLock<Vec<Value>> = OnceLock::new();
    let rows = ROWS.get_or_init(|| {
        include_str!("fixtures/mixinextras_expression/calls.jsonl")
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    });
    rows.iter()
        .find(|r| {
            r["api"] == "ExpressionMemberCompletionService.completeMembers"
                && r["legacyCase"] == row["legacyCase"]
                && r["args"]["__instance"] == row["args"]["__memberInstance"]
        })
        .cloned()
        .unwrap_or_else(|| row.clone())
}
#[derive(Debug)]
struct CancellationMarker;
impl std::fmt::Display for CancellationMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("member completion cancelled")
    }
}
impl std::error::Error for CancellationMarker {}
struct ObservedRunner {
    inner: CapturedRunner,
    count: Arc<std::sync::atomic::AtomicUsize>,
    cancellation: Option<Arc<dyn std::error::Error + Send + Sync>>,
}
impl ExpressionMemberMatcherRunner for ObservedRunner {
    fn complete_reachable_members(
        &self,
        b: &[u8],
        n: &str,
        d: &str,
        r: &str,
        c: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(error) = &self.cancellation {
            return Err(engine::Cancellation(error.clone()));
        }
        self.inner.complete_reachable_members(b, n, d, r, c)
    }
}
fn member_rows(case: &str) -> Vec<Value> {
    include_str!("fixtures/mixinextras_expression/calls.jsonl")
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .filter(|r| {
            r["api"] == "ExpressionMemberCompletionService.completeMembers"
                && r["legacyCase"]
                    .as_str()
                    .unwrap()
                    .ends_with(&format!("::{case}"))
        })
        .map(|r| normalized_row(&r))
        .collect()
}
fn invoke_member(
    row: &Value,
    cache: Arc<ExpressionMemberCompletionCache>,
    runner: Arc<dyn ExpressionMemberMatcherRunner>,
    checker: &dyn engine::CancellationChecker,
) -> Result<ExpressionMemberCompletionServiceResult, engine::Cancellation> {
    let reads = Reads(row["reads"].as_array().unwrap());
    let args = &row["args"];
    let mut service = ExpressionMemberCompletionService::new(&reads, &reads);
    service.reachable_members_cache = cache;
    service.matcher_runner = runner;
    service.complete_members(
        args["source"].as_str().unwrap(),
        &de(&args["context"]),
        &de::<Vec<String>>(&args["mixinTargetOwners"]),
        &de::<Vec<ResolvedMixinExtrasContext>>(&args["resolvedContexts"]),
        checker,
    )
}
fn assert_member_result(row: &Value, result: ExpressionMemberCompletionServiceResult) {
    let expected = match row["outputType"]
        .as_str()
        .unwrap()
        .rsplit('$')
        .next()
        .unwrap()
    {
        "Empty" => ExpressionMemberCompletionServiceResult::Empty,
        "Unavailable" => ExpressionMemberCompletionServiceResult::Unavailable,
        "Available" => ExpressionMemberCompletionServiceResult::Available {
            candidates: row["output"]["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(candidate)
                .collect(),
        },
        other => panic!("unknown service kind {other}"),
    };
    assert_eq!(result, expected);
}
#[test]
fn original_cache_transitions_check_native_state_and_callback_counts_after_every_call() {
    let cases: &[(&str, &[usize])] = &[
        (
            "sharedCacheReusesMatcherAcrossDistinctServiceInstancesWhenOnlyMemberPrefixChanges",
            &[1, 1],
        ),
        (
            "reusesCachedReachableMembersWhenOnlyMemberPrefixChanges",
            &[1, 1],
        ),
        (
            "filtersCachedCandidatesByMemberPrefixWithoutReRunningMatcher",
            &[1, 1, 1],
        ),
        ("cacheMissesWhenReceiverExpressionChanges", &[1, 2]),
        ("cacheMissesWhenClassBytesContentChanges", &[1, 2]),
        ("cacheMissesWhenDefinitionSemanticContentChanges", &[1, 2]),
        (
            "cacheMissesWhenTargetMethodOrContextTypeChanges",
            &[1, 2, 3],
        ),
        (
            "doesNotCacheUnavailableOrCancelledMatcherResults",
            &[1, 2, 1, 2],
        ),
        (
            "evictsOldestCacheEntryAndReRunsMatcherOnCapacityMiss",
            &[1, 2, 3, 4],
        ),
    ];
    for (case, counts) in cases {
        let rows = member_rows(case);
        assert_eq!(rows.len(), counts.len(), "lost original call {case}");
        let mut caches = BTreeMap::new();
        let mut counters = BTreeMap::new();
        for (row, expected_count) in rows.iter().zip(*counts) {
            let id = row["args"]["__cacheInstance"].as_i64().unwrap();
            let cache = caches
                .entry(id)
                .or_insert_with(|| {
                    Arc::new(ExpressionMemberCompletionCache::new(
                        row["args"]["__cacheCapacity"].as_u64().unwrap() as usize,
                    ))
                })
                .clone();
            let counter = counters
                .entry(id)
                .or_insert_with(|| Arc::new(std::sync::atomic::AtomicUsize::new(0)))
                .clone();
            let marker: Arc<dyn std::error::Error + Send + Sync> = Arc::new(CancellationMarker);
            let cancelled = row.get("throwable").is_some();
            let runner = Arc::new(ObservedRunner {
                inner: CapturedRunner {
                    reads: row["reads"].as_array().unwrap().clone(),
                },
                count: counter.clone(),
                cancellation: cancelled.then(|| marker.clone()),
            });
            let result = invoke_member(row, cache, runner, &engine::NO_CANCELLATION);
            if cancelled {
                let error = result.unwrap_err();
                assert!(Arc::ptr_eq(&error.0, &marker));
            } else {
                assert_member_result(row, result.unwrap());
            }
            assert_eq!(
                counter.load(std::sync::atomic::Ordering::SeqCst),
                *expected_count,
                "{case}"
            );
        }
    }
}
#[test]
fn original_cancellation_marker_propagates_by_identity() {
    let rows = member_rows("propagatesCancellation");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.get("throwable").is_some());
    let marker: Arc<dyn std::error::Error + Send + Sync> = Arc::new(CancellationMarker);
    let cancel = marker.clone();
    let checker = move || Err(engine::Cancellation(cancel.clone()));
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runner = Arc::new(ObservedRunner {
        inner: CapturedRunner {
            reads: row["reads"].as_array().unwrap().clone(),
        },
        count: count.clone(),
        cancellation: None,
    });
    let result = invoke_member(
        row,
        Arc::new(ExpressionMemberCompletionCache::default()),
        runner,
        &checker,
    );
    assert!(Arc::ptr_eq(&result.unwrap_err().0, &marker));
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
}
struct TypeBindingRunner {
    count: Arc<std::sync::atomic::AtomicUsize>,
}
#[test]
fn kotlin_definition_context_retains_valid_pool_binding_and_slices_original_ids() {
    let row = include_str!("fixtures/mixinextras_expression/calls.jsonl")
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .find(|r| {
            r["api"] == "ExpressionContextResolver.resolveExpressionContext"
                && r["legacyCase"]
                    .as_str()
                    .unwrap()
                    .ends_with("::aliasImportsReachHandlerRegionAndKeepDefinitionsAndSourceRanges")
        })
        .unwrap();
    let row = normalized_row(&row);
    let source = row["args"]["source"].as_str().unwrap();
    let context =
        ExpressionContextResolver::resolve_expression_context(source, &de(&row["args"]["site"]));
    let built = OfficialExpressionIdentifierPoolBuilder::build(
        &context.definition_index,
        &ClassLiteralTypeNameResolver::FAIL_CLOSED,
    );
    assert!(built.pool.identifier_exists("call"));
    assert_eq!(
        context.values_for_at_id(Some("main")),
        vec!["call()", "field"]
    );
    assert_eq!(context.values_for_at_id(None), vec!["direct"]);
    for definition in &context.definition_index.definitions {
        assert_eq!(
            &source[definition.id_source_range.clone().unwrap()],
            definition.id.as_ref().unwrap()
        );
    }
    assert!(source[context.definition_index.definitions[2]
        .source_range
        .clone()
        .unwrap()]
    .starts_with("type = [String::class]"));
}
struct TrackingBytecode<'a, 'b> {
    reads: &'a Reads<'b>,
    common_calls: std::sync::atomic::AtomicUsize,
}
impl BytecodeIndex for TrackingBytecode<'_, '_> {
    fn get_at_target_candidates(
        &self,
        o: &str,
        n: &str,
        d: Option<&str>,
        v: &str,
    ) -> Vec<AtTargetCandidate> {
        self.reads.get_at_target_candidates(o, n, d, v)
    }
    fn get_return_ordinal_count(&self, o: &str, n: &str, d: Option<&str>) -> usize {
        self.reads.get_return_ordinal_count(o, n, d)
    }
    fn get_class_bytes(&self, o: &str) -> Option<Vec<u8>> {
        self.reads.get_class_bytes(o)
    }
    fn resolve_common_super_class(&self, a: &str, b: &str) -> Option<String> {
        self.common_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.reads.resolve_common_super_class(a, b)
    }
}
#[test]
fn official_matcher_invokes_common_super_class_resolver_for_branch_merge_fixture() {
    let row = include_str!("fixtures/mixinextras_expression/calls.jsonl")
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .find(|r| {
            r["api"] == "ExpressionContextResolver.inferExpressionValueType"
                && r["legacyCase"].as_str().unwrap().ends_with(
                    "::officialMatcherInvokesCommonSuperClassResolverForBranchMergeFixture",
                )
        })
        .unwrap();
    let row = normalized_row(&row);
    let reads = Reads(row["reads"].as_array().unwrap());
    let bytecode = TrackingBytecode {
        reads: &reads,
        common_calls: std::sync::atomic::AtomicUsize::new(0),
    };
    let args = &row["args"];
    let result = ExpressionContextResolver::infer_expression_value_type(
        args["source"].as_str().unwrap(),
        &de(&args["site"]),
        &de(&args["targetMethod"]),
        &de::<Vec<String>>(&args["mixinTargets"]),
        &bytecode,
        &reads,
        None,
    );
    assert_eq!(result.as_deref(), Some("Ljava/lang/Object;"));
    assert!(
        bytecode
            .common_calls
            .load(std::sync::atomic::Ordering::SeqCst)
            > 0
    );
}
#[test]
fn keyword_and_definition_id_completion_does_not_invoke_member_matcher() {
    let row=include_str!("fixtures/mixinextras_expression/calls.jsonl").lines().map(|s|serde_json::from_str::<Value>(s).unwrap()).find(|r|r["api"]=="ExpressionSupport.completeExpressionValue"&&r["legacyCase"].as_str().unwrap().ends_with("::completeExpressionValueKeywordAndDefinitionIdCompletionDoesNotInvokeMemberService")).unwrap();
    let row = normalized_row(&row);
    let reads = Reads(row["reads"].as_array().unwrap());
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut member = ExpressionMemberCompletionService::new(&reads, &reads);
    member.matcher_runner = Arc::new(ObservedRunner {
        inner: CapturedRunner { reads: vec![] },
        count: count.clone(),
        cancellation: None,
    });
    let support = ExpressionSupport::new(Some(&member), None);
    let args = &row["args"];
    let result = support
        .complete_expression_value(
            &de(&args["context"]),
            args["source"].as_str().unwrap(),
            &de::<Vec<ResolvedMixinExtrasContext>>(&args["resolvedContexts"]),
            &de::<Vec<String>>(&args["mixinTargetOwners"]),
            &engine::NO_CANCELLATION,
        )
        .unwrap();
    assert_eq!(
        result
            .iter()
            .map(|i| i.insert_text.as_str())
            .collect::<Vec<_>>(),
        vec!["this", "super", "true", "false", "null", "new", "alpha", "beta"]
    );
    assert!(result
        .iter()
        .all(|i| i.metadata.source != "mixinextras.expressionMember"));
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
}
impl ExpressionMemberMatcherRunner for TypeBindingRunner {
    fn complete_reachable_members(
        &self,
        _: &[u8],
        _: &str,
        _: &str,
        _: &str,
        context: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let one = context
            .identifier_pool
            .types
            .get("T")
            .is_some_and(|types| types.iter().any(|d| d == "Lone/Foo;"));
        Ok(engine::MemberCompletionResult::Available(vec![
            engine::MemberCandidate {
                kind: engine::MemberKind::Field,
                owner_internal_name: "sample/Target".into(),
                name: if one { "one" } else { "two" }.into(),
                descriptor: "I".into(),
                is_interface: false,
                original_instruction_opcode: 180,
                original_instruction_index: 0,
                decorations: Default::default(),
            },
        ]))
    }
}
#[test]
fn changing_import_recomputes_bindings_but_unchanged_import_reuses_candidates() {
    let rows = member_rows("changingImportRecomputesBindingsButUnchangedImportReusesCandidates");
    assert_eq!(rows.len(), 3);
    let cache = Arc::new(ExpressionMemberCompletionCache::default());
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runner = Arc::new(TypeBindingRunner {
        count: count.clone(),
    });
    for (i, row) in rows.iter().enumerate() {
        assert_member_result(
            row,
            invoke_member(row, cache.clone(), runner.clone(), &engine::NO_CANCELLATION).unwrap(),
        );
        assert_eq!(
            count.load(std::sync::atomic::Ordering::SeqCst),
            [1, 2, 2][i]
        );
    }
}
impl Reads<'_> {
    fn get(&self, api: &str, args: Value) -> Option<&Value> {
        self.0
            .iter()
            .find(|r| r["api"] == api && r["args"] == args)
            .map(|r| &r["output"])
    }
    fn classes(&self) -> Vec<ClassIndexEntry> {
        let mut out = BTreeMap::new();
        for r in self.0 {
            let v = &r["output"];
            let items = v
                .as_array()
                .map(|a| a.iter().collect())
                .unwrap_or_else(|| vec![v]);
            for v in items {
                if v.get("simpleName").is_some() && v.get("packageName").is_some() {
                    let c: ClassIndexEntry = de(v);
                    out.insert(c.internal_name.clone(), c);
                }
            }
        }
        out.into_values().collect()
    }
}
impl ClassIndex for Reads<'_> {
    fn find_classes(&self, p: &str, l: usize) -> Vec<ClassIndexEntry> {
        self.get("ClassIndex.findClasses", json!([p, l]))
            .map_or_else(
                || {
                    self.classes()
                        .into_iter()
                        .filter(|c| c.simple_name.starts_with(p))
                        .take(l)
                        .collect()
                },
                de,
            )
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        if let Some(v) = self.get("ClassIndex.findClass", json!([n])) {
            if v.is_null() {
                None
            } else {
                Some(de(v))
            }
        } else {
            self.classes().into_iter().find(|c| c.internal_name == n)
        }
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        if let Some(v) = self.get("ClassIndex.findClassByFqn", json!([n])) {
            if v.is_null() {
                None
            } else {
                Some(de(v))
            }
        } else {
            self.classes().into_iter().find(|c| c.fqn() == n)
        }
    }
    fn get_methods(&self, n: &str) -> Vec<MethodIndexEntry> {
        self.get("ClassIndex.getMethods", json!([n]))
            .map_or(vec![], de)
    }
    fn get_fields(&self, n: &str) -> Vec<FieldIndexEntry> {
        self.get("ClassIndex.getFields", json!([n]))
            .map_or(vec![], de)
    }
}
impl BytecodeIndex for Reads<'_> {
    fn get_at_target_candidates(
        &self,
        o: &str,
        n: &str,
        d: Option<&str>,
        a: &str,
    ) -> Vec<AtTargetCandidate> {
        self.get("BytecodeIndex.getAtTargetCandidates", json!([o, n, d, a]))
            .map_or(vec![], de)
    }
    fn get_return_ordinal_count(&self, o: &str, n: &str, d: Option<&str>) -> usize {
        self.get("BytecodeIndex.getReturnOrdinalCount", json!([o, n, d]))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize
    }
    fn get_class_bytes(&self, o: &str) -> Option<Vec<u8>> {
        self.get("BytecodeIndex.getClassBytes", json!([o]))
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|n| n.as_i64().unwrap() as u8).collect())
    }
    fn resolve_common_super_class(&self, a: &str, b: &str) -> Option<String> {
        self.get("BytecodeIndex.resolveCommonSuperClass", json!([a, b]))
            .and_then(Value::as_str)
            .map(String::from)
    }
}
fn replay(row: &Value) -> Option<Value> {
    let args = &row["args"];
    let api = row["api"].as_str().unwrap();
    if is_source_api(api) {
        return Some(source_replay::replay(row));
    }
    let s = |key: &str| args[key].as_str().unwrap_or("");
    let n = |key: &str| args[key].as_u64().unwrap_or(0) as usize;
    let imports = args
        .get("imports")
        .filter(|v| !v.is_null())
        .map(de::<JavaSourceImports>);
    let body = s("body");
    let reads = Reads(row["reads"].as_array().map_or(&[], |a| a.as_slice()));
    let config = member_configuration(row);
    let member = member_service(&config, &reads);
    let support = ExpressionSupport::new(
        if args["__memberInstance"].is_null() {
            None
        } else {
            Some(&member)
        },
        Some(&reads),
    );
    Some(match api {
        "HandlerSignatureService.findAnnotationSites"=>json!(mcdev_core::mixinextras_handler::HandlerSignatureService::find_annotation_sites(s("source"))),
        "HandlerSignatureService.findSugarHandlerAnnotationSites"=>json!(mcdev_core::mixinextras_handler::HandlerSignatureService::find_sugar_handler_annotation_sites(s("source"))),
        "ExpressionMemberCompletionService.completeMembers" => json!(member
            .complete_members(
                s("source"),
                &de(&args["context"]),
                &de::<Vec<String>>(&args["mixinTargetOwners"]),
                &de::<Vec<ResolvedMixinExtrasContext>>(&args["resolvedContexts"]),
                &engine::NO_CANCELLATION
            )
            .unwrap()),
        "ExpressionSupport.completeAtValue" => {
            json!(support.complete_at_value(&de(&args["context"])))
        }
        "ExpressionSupport.completeExpressionAnnotations" => {
            json!(support.complete_expression_annotations(&de(&args["context"])))
        }
        "ExpressionSupport.completeExpressionValue" => json!(support
            .complete_expression_value(
                &de(&args["context"]),
                s("source"),
                &de::<Vec<ResolvedMixinExtrasContext>>(&args["resolvedContexts"]),
                &de::<Vec<String>>(&args["mixinTargetOwners"]),
                &engine::NO_CANCELLATION
            )
            .unwrap()),
        "ExpressionSupport.completeFeatureAnnotations" => {
            json!(support.complete_feature_annotations(s("source"), n("line"), n("character")))
        }
        "ExpressionSupport.completeFeatureAnnotationsWithRange" => json!(
            support.complete_feature_annotations_with_range(s("source"), n("line"), n("character"))
        ),
        "ExpressionSupport.isExpressionAtValue" => json!(
            ExpressionSupport::is_expression_at_value(args["atValue"].as_str())
        ),
        "DefinitionAnnotationParser.parse" => {
            json!(DefinitionAnnotationParser::parse(body, imports.as_ref()))
        }
        "DefinitionsAnnotationParser.parse" => json!(DefinitionsAnnotationParser::parse(
            body,
            n("parsedTextBaseOffset"),
            imports.as_ref()
        )),
        "DefinitionsAnnotationParser.parseDefinitionAt" => {
            json!(DefinitionsAnnotationParser::parse_definition_at(
                s("source"),
                n("definitionAtOffset"),
                n("parsedTextBaseOffset"),
                imports.as_ref()
            ))
        }
        "ExpressionAnnotationParser.parse" => {
            json!(ExpressionAnnotationParser::parse(body, imports.as_ref()))
        }
        "ExpressionsAnnotationParser.parse" => {
            json!(ExpressionsAnnotationParser::parse(body, imports.as_ref()))
        }
        "ExpressionCompletionLexer.lex" => {
            let source = s("source");
            let source = if source.is_empty() {
                s("expressionPrefix")
            } else {
                source
            };
            let cursor = args
                .get("cursor")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| args["cursorInExpression"].as_u64().unwrap_or(0));
            let lex = ExpressionCompletionLexer::lex(source, cursor as usize);
            json!({"position":format!("{:?}",lex.position),"tokenStart":lex.token_start,"tokenEnd":lex.token_end})
        }
        "ExpressionReceiverParser.parse" => json!(ExpressionReceiverParser::parse(
            s("expressionPrefix"),
            n("cursorInExpression")
        )),
        "ExpressionReceiverParser.isValidReceiverExpression" => json!(
            ExpressionReceiverParser::is_valid_receiver_expression(s("receiverExpression"))
        ),
        "ExpressionContextResolver.parseHandlerAnnotations" => {
            let region = s("handlerRegion");
            let imp = imports.unwrap_or_else(|| JavaTypeDescriptorResolver::imports_for(region));
            json!(ExpressionContextResolver::parse_handler_annotations(
                region,
                n("regionStartOffset"),
                &imp
            ))
        }
        "ExpressionContextResolver.resolveExpressionContext" => json!(
            ExpressionContextResolver::resolve_expression_context(s("source"), &de(&args["site"]))
        ),
        "ExpressionContextResolver.resolveExpressionContextForSite" => json!(
            ExpressionContextResolver::resolve_expression_context_for_site(
                s("source"),
                &de(&args["site"]),
                &de::<Vec<ResolvedMixinExtrasContext>>(&args["resolvedContexts"])
            )
        ),
        "ExpressionContextResolver.findEnclosingSite" => json!(
            ExpressionContextResolver::find_enclosing_site(s("source"), n("offset"))
        ),
        "ExpressionContextResolver.handlerRegion" => json!(
            ExpressionContextResolver::handler_region(s("source"), &de(&args["site"]))
        ),
        "ExpressionContextResolver.inferFromExpression" => {
            json!(ExpressionContextResolver::infer_from_expression(
                s("expression"),
                s("ownerInternalName"),
                &de(&args["targetMethod"]),
                &reads,
                &reads
            ))
        }
        api if api.starts_with("ExpressionContextResolver.infer") => {
            let site: MixinExtrasAnnotationSite = de(&args["site"]);
            let method: MethodIndexEntry = de(&args["targetMethod"]);
            let owners: Vec<String> = de(&args["mixinTargets"]);
            let resolved = args
                .get("resolvedContext")
                .filter(|v| !v.is_null())
                .map(de::<ExpressionContext>);
            match api {
                "ExpressionContextResolver.inferExpressionValueType" => {
                    json!(ExpressionContextResolver::infer_expression_value_type(
                        s("source"),
                        &site,
                        &method,
                        &owners,
                        &reads,
                        &reads,
                        resolved.as_ref()
                    ))
                }
                "ExpressionContextResolver.inferExpressionValueTypeInfo" => {
                    json!(ExpressionContextResolver::infer_expression_value_type_info(
                        s("source"),
                        &site,
                        &method,
                        &owners,
                        &reads,
                        &reads,
                        resolved.as_ref()
                    ))
                }
                "ExpressionContextResolver.inferSimpleOperationLayout" => {
                    json!(ExpressionContextResolver::infer_simple_operation_layout(
                        s("source"),
                        &site,
                        &method,
                        &owners,
                        &reads,
                        &reads,
                        resolved.as_ref()
                    ))
                }
                "ExpressionContextResolver.inferModifyReceiverLayout" => {
                    json!(ExpressionContextResolver::infer_modify_receiver_layout(
                        s("source"),
                        &site,
                        &method,
                        &owners,
                        &reads,
                        &reads,
                        resolved.as_ref()
                    ))
                }
                "ExpressionContextResolver.inferWrapWithConditionLayouts" => json!(
                    ExpressionContextResolver::infer_wrap_with_condition_layouts(
                        s("source"),
                        &site,
                        &method,
                        &owners,
                        &reads,
                        &reads,
                        resolved.as_ref()
                    )
                ),
                _ => return None,
            }
        }
        "ExpressionDefinitionEditPlanner.plan" => json!(ExpressionDefinitionEditPlanner::plan(
            &de(&args["context"]),
            args["expressionAnnotationOffset"].as_i64().unwrap() as isize,
            s("source"),
            &de(&args["definitionIndex"]),
            &candidate(&args["candidate"])
        )),
        "ExpressionDefinitionEditPlanner.canonicalExactSelector" => {
            json!(ExpressionDefinitionEditPlanner::canonical_exact_selector(
                &candidate(&args["candidate"])
            ))
        }
        "ExpressionDefinitionEditPlanner.findReusableDefinitionId" => {
            let kind = if args["kind"] == "FIELD" {
                engine::MemberKind::Field
            } else {
                engine::MemberKind::Method
            };
            json!(
                ExpressionDefinitionEditPlanner::find_reusable_definition_id(
                    &de(&args["definitionIndex"]),
                    kind,
                    s("exactSelector")
                )
            )
        }
        "ExpressionDefinitionEditPlanner.allocateDefinitionId" => {
            json!(ExpressionDefinitionEditPlanner::allocate_definition_id(
                &de(&args["definitionIndex"]),
                s("memberName")
            ))
        }
        _ => return None,
    })
}
fn is_source_api(api: &str) -> bool {
    [
        "AnnotationContextExtractor.",
        "JavaTypeDescriptorResolver.",
        "MixinMemberDeclarationParser.",
        "MixinSemanticModelParser.",
        "MixinTargetResolver.",
        "SemanticCompletionContextExtractor.",
        "JavaStringContentDecoder.",
    ]
    .iter()
    .any(|prefix| api.starts_with(prefix))
}
#[test]
fn every_original_expression_case_and_full_output() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("tests/fixtures/mixinextras_expression/calls.jsonl");
    let text = std::fs::read_to_string(path).unwrap();
    let mut count = 0;
    let mut failures = vec![];
    let manifest: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/mixinextras_expression/cases.json")).unwrap();
    assert_eq!(manifest.len(), 316);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut cancellations = vec![];
    for (line, row) in text.lines().enumerate() {
        let row: Value = serde_json::from_str(row).unwrap();
        let source_api = is_source_api(row["api"].as_str().unwrap());
        let row: Value = if source_api {
            row
        } else {
            normalized_row(&row)
        };
        if row.get("throwable").is_some() {
            cancellations.push(row["legacyCase"].as_str().unwrap().to_string());
            continue;
        }
        *seen
            .entry(row["legacyCase"].as_str().unwrap().to_string())
            .or_default() += 1;
        let actual = std::panic::catch_unwind(|| replay(&row));
        let actual = match actual {
            Ok(Some(v)) => v,
            Ok(None) => panic!("unsupported capture API {}", row["api"]),
            Err(_) => {
                failures.push(
                    json!({"line":line+1,"case":row["legacyCase"],"api":row["api"],"panic":true}),
                );
                continue;
            }
        };
        count += 1;
        let kinds_match = !source_api || source_replay::kinds_match(&row, &actual);
        let mut expected = if source_api {
            source_replay::expected_output(&row)
        } else {
            row["output"].clone()
        };
        if row["api"] == "ExpressionMemberCompletionService.completeMembers"
            && expected.get("candidates").is_some()
        {
            expected["candidates"] = json!(expected["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(candidate)
                .collect::<Vec<_>>());
        }
        let kind = row["outputType"]
            .as_str()
            .and_then(|s| s.rsplit('$').next())
            .filter(|s| matches!(*s, "Available" | "Unavailable" | "Empty"));
        if let Some(kind) = kind.filter(|_| !source_api) {
            expected = if kind == "Empty"
                || kind == "Unavailable"
                    && row["api"] == "ExpressionMemberCompletionService.completeMembers"
            {
                json!(kind)
            } else {
                json!({kind:expected})
            };
        }
        let actual = canonical(actual);
        let expected = canonical(expected);
        if actual != expected || !kinds_match {
            failures.push(json!({"line":line+1,"case":row["legacyCase"],"api":row["api"],"actual":actual,"expected":expected}));
        }
    }
    assert_eq!(cancellations.len(), 2);
    assert!(cancellations
        .iter()
        .all(|id| id.ends_with("::propagatesCancellation")
            || id.ends_with("::doesNotCacheUnavailableOrCancelledMatcherResults")));
    for case in &manifest {
        assert_eq!(case["status"], "passed");
        assert_eq!(
            seen.get(case["legacyCase"].as_str().unwrap())
                .copied()
                .unwrap_or(0),
            case["calls"].as_u64().unwrap() as usize,
            "lost original calls {}",
            case["legacyCase"]
        );
    }
    let log = root.join("../build/rust-test-logs/expression-source-failures.json");
    std::fs::write(&log, serde_json::to_string_pretty(&failures).unwrap()).unwrap();
    assert!(count > 0);
    assert!(
        failures.is_empty(),
        "{} full-output failures / {count} supported calls; first={}; log={}",
        failures.len(),
        failures
            .first()
            .map_or(String::new(), |v| v["case"].to_string()
                + " "
                + v["api"].as_str().unwrap_or("")),
        log.display()
    );
}
