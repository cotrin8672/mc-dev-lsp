mod host;
mod http;
mod stdio;
use host::{parse, RecordedHost};
use mcdev_companion::commands::*;
use mcdev_core::{
    expression_engine::{Cancellation, NO_CANCELLATION},
    *,
};
use mcdev_protocol::{convert::CompletionItemConverter, *};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};

fn rows() -> &'static Vec<Value> {
    static ROWS: OnceLock<Vec<Value>> = OnceLock::new();
    ROWS.get_or_init(|| {
        include_str!("legacy_calls.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    })
}
fn wire(value: Value) -> Value {
    let mut value = if value.is_object() {
        Value::Object(ProtocolPayloadDecoder.encode_to_map(&value).unwrap())
    } else {
        value
    };
    if let Some(caps) = value.get_mut("capabilities").and_then(Value::as_array_mut) {
        let unique = caps.iter().map(Value::to_string).collect::<BTreeSet<_>>();
        assert_eq!(caps.len(), unique.len());
        caps.sort_by_key(Value::to_string);
    }
    fn json_numbers(value: &mut Value) {
        match value {
            Value::Object(m) => m.values_mut().for_each(json_numbers),
            Value::Array(a) => a.iter_mut().for_each(json_numbers),
            Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                if f.fract() == 0.0 && f.abs() <= (1u64 << 53) as f64 {
                    *value = json!(f as i64);
                }
            }
            _ => (),
        }
    }
    // Gson Map<String,Any> encodes integral protocol values as 1.0.
    // JSON has one number type; preserve exact values without string coercion.
    json_numbers(&mut value);
    value
}
fn command(api: &str, args: &Value) -> String {
    if let Some(command) = args.get("command").or_else(|| args.get("commandId")) {
        return command.as_str().unwrap().into();
    }
    match api.split('.').next().unwrap() {
        "McdevCompletionHandler" => McdevCommands::COMPLETION,
        "McdevDiagnosticsHandler" => McdevCommands::DIAGNOSTICS,
        "McdevDefinitionHandler" => McdevCommands::DEFINITION,
        "McdevReferencesHandler" => McdevCommands::REFERENCES,
        "McdevHoverHandler" => McdevCommands::HOVER,
        "McdevCodeActionHandler" => McdevCommands::CODE_ACTION,
        "McdevInfoHandler" => McdevCommands::INFO,
        "McdevReindexHandler" => McdevCommands::REINDEX,
        "McdevProjectContextHandler" if api.ends_with(".reload") => {
            McdevCommands::RELOAD_PROJECT_CONTEXT
        }
        "McdevProjectContextHandler" => McdevCommands::DUMP_CONTEXT,
        "McdevCommandDispatcher" if api.ends_with(".executeProjectAwareCompletion") => {
            McdevCommands::COMPLETION
        }
        other => panic!("unmapped caller API {other}"),
    }
    .into()
}
fn arguments(args: &Value) -> Vec<Value> {
    if let Some(arguments) = args.get("arguments") {
        return parse(arguments);
    }
    if let Some(request) = args.get("request") {
        return vec![request.clone()];
    }
    if let Some(context) = args.get("context") {
        return vec![json!({"context":context})];
    }
    panic!("unmapped caller arguments {args}")
}
fn compare(row: &Value, index: usize, actual: Value) {
    static TIMINGS: OnceLock<Vec<Value>> = OnceLock::new();
    let timings = TIMINGS.get_or_init(|| {
        parse(&serde_json::from_str::<Value>(include_str!("timing_assertions.json")).unwrap())
    });
    let mut expected = wire(row["output"].clone());
    let mut actual = wire(actual);
    let deltas: Vec<Value> = serde_json::from_str(include_str!("explicit_deltas.json")).unwrap();
    for delta in deltas.iter().filter(|d| d["observation"] == index) {
        assert_eq!(delta["legacyCase"], row["legacyCase"]);
        assert_eq!(delta["api"], row["api"]);
        let pointer = delta["field"].as_str().unwrap();
        assert_eq!(expected.pointer(pointer).unwrap(), &delta["old"]);
        assert_eq!(actual.pointer(pointer).unwrap(), &delta["new"]);
        let source = row["args"]["arguments"][0]["context"]["bufferText"]
            .as_str()
            .unwrap();
        assert_eq!(apply_edits(source, &delta["old"]), delta["after"]);
        assert_eq!(apply_edits(source, &delta["new"]), delta["after"]);
        *expected.pointer_mut(pointer).unwrap() = delta["new"].clone();
    }
    for timing in timings.iter().filter(|t| t["observation"] == index) {
        assert_eq!(timing["legacyCase"], row["legacyCase"]);
        assert_eq!(timing["api"], row["api"]);
        let pointer = timing["field"].as_str().unwrap();
        assert_eq!(
            expected.pointer(pointer).unwrap(),
            &timing["originalMeasurement"]
        );
        assert!(
            actual
                .pointer(pointer)
                .and_then(Value::as_i64)
                .is_some_and(|n| n >= 0),
            "measured duration must be a nonnegative integer: {pointer}"
        );
        *actual.pointer_mut(pointer).unwrap() = timing["originalMeasurement"].clone();
    }
    if actual != expected {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../build/command-replay-differences");
        std::fs::create_dir_all(&dir).unwrap();
        let name = format!(
            "{}-{index}.json",
            row["legacyCase"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .replace("::", "-")
        );
        let path = dir.join(name);
        std::fs::write(&path,serde_json::to_string_pretty(&json!({"case":row["legacyCase"],"api":row["api"],"args":row["args"],"expected":expected,"actual":actual})).unwrap()).unwrap();
        panic!(
            "{} {} observation{index} differs: {}",
            row["legacyCase"],
            row["api"],
            path.display()
        );
    }
}
fn apply_edits(source: &str, edits: &Value) -> String {
    let mut edits = edits
        .as_array()
        .unwrap()
        .iter()
        .map(|edit| {
            let offset = |position: &Value| {
                position_to_offset(
                    source,
                    McTextPosition {
                        line: position["line"].as_u64().unwrap() as usize,
                        character: position["character"].as_u64().unwrap() as usize,
                    },
                )
                .unwrap()
            };
            (
                offset(&edit["range"]["start"]),
                offset(&edit["range"]["end"]),
                edit["newText"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|e| std::cmp::Reverse(e.0));
    let mut output = source.to_string();
    let mut previous_start = source.len();
    for (start, end, text) in edits {
        assert!(start <= end && end <= previous_start);
        output.replace_range(start..end, text);
        previous_start = start;
    }
    output
}
pub fn replay_case(case: &str) {
    if case.contains("/transport/") {
        http::replay(case);
        return;
    }
    if case.contains("/stdio/") {
        stdio::replay();
        return;
    }
    if case.ends_with("::registryListsImplementedCommands") {
        assert_eq!(
            COMMANDS.map(|c| c.id()),
            [
                "mcdev.completion",
                "mcdev.diagnostics",
                "mcdev.context",
                "mcdev.info",
                "mcdev.reindex",
                "mcdev.reloadProjectContext",
                "mcdev.dumpContext",
                "mcdev.definition",
                "mcdev.references",
                "mcdev.hover",
                "mcdev.codeAction"
            ]
        );
        return;
    }
    if case.ends_with("::onlyCappedClassSourcesAreIncomplete") {
        for source in ["mixin.target", "mixin.targets", "aw.class", "at.class"] {
            assert!(CompletionItemConverter::is_incomplete(&items(
                CLASS_COMPLETION_LIMIT,
                source
            )));
        }
        assert!(!CompletionItemConverter::is_incomplete(&items(
            CLASS_COMPLETION_LIMIT - 1,
            "mixin.target"
        )));
        assert!(!CompletionItemConverter::is_incomplete(&items(
            CLASS_COMPLETION_LIMIT,
            "mixin.atTarget"
        )));
        return;
    }
    if case.ends_with("::wireResponseIncludesIsIncomplete") {
        let mut response = McdevCompletionResponse {
            items: vec![],
            is_incomplete: true,
            warnings: vec![],
            debug: None,
        };
        assert!(serde_json::to_string(&response)
            .unwrap()
            .contains("\"isIncomplete\":true"));
        response.is_incomplete = false;
        assert!(serde_json::to_string(&response)
            .unwrap()
            .contains("\"isIncomplete\":false"));
        return;
    }
    let mut runtimes =
        BTreeMap::<String, (Arc<RecordedHost>, Arc<CommandDispatcher<RecordedHost>>)>::new();
    let mut count = 0;
    for (index, row) in rows()
        .iter()
        .enumerate()
        .filter(|(_, r)| r["legacyCase"] == case && r["frameDepth"] == 1)
    {
        count += 1;
        let api = row["api"].as_str().unwrap();
        let args = &row["args"];
        let instance = args["_instance"].to_string();
        let (host, runtime) = runtimes.entry(instance).or_insert_with(|| {
            let host = Arc::new(RecordedHost::new());
            let runtime = Arc::new(CommandDispatcher::new(host.clone()));
            (host, runtime)
        });
        // Each recorded host reply is the original callback's value. Runtime state persists
        // for the same original handler, and callback source/options/versions are checked.
        host.set(row);
        if api.ends_with(".negativeCacheSizeForTests") {
            compare(row, index, json!(runtime.negative_cache_size().unwrap()));
            host.assert_host_calls();
            continue;
        }
        let command = command(api, args);
        let arguments = arguments(args);
        if api.starts_with("McdevDelegateCommandHandler.") {
            let handler = DelegateCommandHandler::new(runtime.clone());
            if row.get("throwable").is_some() {
                if row["throwable"] == "java.lang.UnsupportedOperationException" {
                    let error = handler
                        .execute_command(&command, &arguments, &NO_CANCELLATION)
                        .unwrap_err();
                    assert!(matches!(error, DelegateFailure::UnsupportedCommand(_)));
                    assert_eq!(error.to_string(), row["message"]);
                } else {
                    let checks = AtomicUsize::new(0);
                    let checker = || {
                        if checks.fetch_add(1, Ordering::SeqCst) > 0 {
                            Err(Cancellation(Arc::new(std::io::Error::other(
                                "mcdev command cancelled",
                            ))))
                        } else {
                            Ok(())
                        }
                    };
                    let error = handler
                        .execute_command(&command, &arguments, &checker)
                        .unwrap_err();
                    assert!(matches!(error, DelegateFailure::Cancelled(_)));
                    assert!(checks.load(Ordering::SeqCst) > 1);
                }
                host.assert_host_calls();
                continue;
            }
            let result = handler
                .execute_command(&command, &arguments, &NO_CANCELLATION)
                .unwrap();
            compare(row, index, result);
        } else {
            let result = if api.ends_with(".handleProjectAware")
                || api.ends_with(".executeProjectAwareCompletion")
            {
                runtime.execute_project_aware_completion(&arguments, &NO_CANCELLATION)
            } else {
                runtime.execute(&command, &arguments)
            }
            .unwrap();
            compare(row, index, json!(result));
        }
        host.assert_host_calls();
    }
    assert!(count > 0, "uncaptured original caller {case}");
}
fn items(count: usize, source: &str) -> Vec<McCompletionItem> {
    (0..count)
        .map(|n| McCompletionItem {
            label: format!("Class{n}"),
            detail: None,
            documentation: None,
            filter_text: format!("Class{n}"),
            insert_text: format!("Class{n}.class"),
            kind: McCompletionKind::Class,
            sort_key: format!("0100_Class{n}"),
            metadata: McCompletionMetadata {
                source: source.into(),
                ..Default::default()
            },
            additional_edits: vec![],
            insert_text_format: Default::default(),
        })
        .collect()
}

#[test]
fn manifest_is_the_exact_frozen_executable_case_set() {
    let manifest: Vec<Value> = serde_json::from_str(include_str!("original_cases.json")).unwrap();
    let captured: Vec<Value> = serde_json::from_str(include_str!("legacy_cases.json")).unwrap();
    let originals = manifest
        .iter()
        .map(|v| v["legacyCase"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(originals.len(), 127);
    assert_eq!(manifest.len(), 127);
    assert_eq!(captured.len(), 127);
    assert!(captured.iter().all(|v| v["status"] == "passed"));
    assert_eq!(
        originals,
        captured
            .iter()
            .map(|v| v["legacyCase"].as_str().unwrap())
            .collect()
    );
    let lf = include_str!("../../commands_legacy.rs").replace("\r\n", "\n");
    for source in [lf.clone(), lf.replace('\n', "\r\n")] {
        // Git checkouts may use either source newline style.
        let source = source.replace("\r\n", "\n");
        let mut cases = BTreeSet::new();
        let mut functions = BTreeSet::new();
        let mut count = 0;
        for (at, _) in source.match_indices("replay::replay_case(") {
            let start = source[at..].find('"').unwrap() + at + 1;
            let end = source[start..].find('"').unwrap() + start;
            assert!(
                cases.insert(&source[start..end]),
                "duplicate executable case"
            );
            let before = &source[..at];
            let function = before.rfind("fn ").unwrap() + 3;
            let finish = before[function..].find('(').unwrap() + function;
            assert!(
                functions.insert(&source[function..finish]),
                "duplicate executable function"
            );
            assert!(before[..function].ends_with("#[test]\nfn "));
            count += 1;
        }
        assert_eq!(count, 127);
        assert_eq!(originals, cases);
    }
    assert_eq!(rows().len(), 1015);
    assert_eq!(rows().iter().filter(|r| r["frameDepth"] == 1).count(), 474);
}

#[test]
fn document_cache_and_negative_cache_do_not_alias_hash_collisions_or_expire_early() {
    use mcdev_companion::commands::cache::*;
    assert_eq!(java_text_hash("Aa"), java_text_hash("BB"));
    assert_eq!(java_text_hash("😀"), 1_772_899);
    let mut docs = DocumentSnapshotCache::new(2, 10);
    assert!(!docs.get("file:///a.kt", None, "Aa", 0).cache_hit);
    assert!(!docs.get("file:///a.kt", None, "BB", 0).cache_hit);
    assert_eq!(docs.get("file:///a.kt", None, "BB", 10).snapshot.text, "BB");
    assert!(docs.get("file:///a.kt", None, "BB", 10).cache_hit);
    assert!(!docs.get("file:///a.kt", None, "BB", 11).cache_hit);
    docs.get("file:///b.kt", Some(1), "日本語😀", 12);
    docs.get("file:///c.kt", Some(1), "c", 12);
    assert!(!docs.get("file:///a.kt", None, "BB", 12).cache_hit);
    docs.invalidate("file:///a.kt");
    assert!(!docs.get("file:///a.kt", None, "BB", 12).cache_hit);
    docs.invalidate_all();
    assert!(!docs.get("file:///a.kt", None, "BB", 12).cache_hit);
    let mut negatives = NegativeCompletionCache::default();
    negatives.put("hash-key".into(), "Aa".into(), "NO_CANDIDATES".into(), 100);
    assert_eq!(
        negatives.get("hash-key", "Aa", 100).as_deref(),
        Some("NO_CANDIDATES")
    );
    assert_eq!(negatives.get("hash-key", "BB", 100), None);
    assert!(negatives.is_empty());
    negatives.put("hash-key".into(), "Aa".into(), "NO_CANDIDATES".into(), 100);
    assert_eq!(negatives.get("hash-key", "Aa", 101), None);
}

#[test]
fn in_flight_identical_requests_recheck_negative_cache_after_the_first_completes() {
    let row = rows()
        .iter()
        .find(|r| {
            r["frameDepth"] == 1
                && r["legacyCase"]
                    .as_str()
                    .unwrap()
                    .ends_with("::reusesNegativeCompletionForSameZeroItemRequest")
        })
        .unwrap();
    let host = Arc::new(RecordedHost::new());
    host.set(row);
    host.buffer_delay_millis.store(150, Ordering::Relaxed);
    let runtime = Arc::new(CommandDispatcher::new(host.clone()));
    let args = arguments(&row["args"]);
    let first_runtime = runtime.clone();
    let first_args = args.clone();
    let first = std::thread::spawn(move || {
        first_runtime
            .execute(McdevCommands::COMPLETION, &first_args)
            .unwrap()
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|s| s == "mixinFacade.tryCompleteBufferOnly")
    {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let second = runtime.execute(McdevCommands::COMPLETION, &args).unwrap();
    let first = first.join().unwrap();
    assert_eq!(
        first.result.as_ref().unwrap()["debug"]["negativeCacheHit"],
        false
    );
    assert_eq!(
        second.result.as_ref().unwrap()["debug"]["negativeCacheHit"],
        true
    );
    assert_eq!(
        second.result.as_ref().unwrap()["debug"]["inFlightDedupHit"],
        true
    );
    host.assert_host_calls();
}
