mod support;
use serde_json::json;
use support::{empty_evaluation, Process};

#[test]
fn stock_f595_array_configuration_enters_fact_queries_instead_of_rejecting_evaluation() {
    let messages: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "fixtures/semantic_v1/stock_f595_array_configuration.json"
    ))
    .unwrap();
    let mut p = Process::new();
    p.send(messages[0].clone());
    assert_eq!(
        p.receive()["result"]["capabilities"],
        json!(["semantic-v1"])
    );
    assert_eq!(messages[1]["params"]["configuration"], json!([]));
    p.send(messages[1].clone());
    let batch = p.receive();
    assert_eq!(batch["method"], "analysis/readFacts");
    assert_eq!(batch["params"]["evaluation"], "4");
    let queries = batch["params"]["queries"].as_array().unwrap();
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0], json!({"op":"resolveTypeRef","reference":"4:3"}));
    assert_eq!(queries[1], json!({"op":"resolveTypeRef","reference":"4:5"}));
    let facts = queries
        .iter()
        .map(|_| json!({"state":"unknown","reasons":[{"code":"fixtureScopeUnavailable"}]}))
        .collect::<Vec<_>>();
    p.send(json!({"jsonrpc":"2.0","id":batch["id"],"result":{"facts":facts}}));
    let result = p.receive();
    assert_eq!(result["id"], "4");
    assert!(result.get("error").is_none(), "{result}");
    assert_eq!(result["result"]["sites"], json!([]));
    p.eof();
    assert!(p.wait().success());
}
#[test]
fn opaque_provider_configuration_preserves_every_json_value_kind() {
    let mut p = Process::new();
    p.initialize();
    for (i, configuration) in [
        json!({}),
        json!([]),
        json!(["opaque"]),
        json!(null),
        json!(true),
        json!(7),
        json!("opaque"),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("config-{i}");
        let mut evaluation = empty_evaluation(&id);
        evaluation["configuration"] = configuration;
        p.request(&id, "analysis/evaluate", evaluation);
        assert_eq!(
            p.receive(),
            json!({"jsonrpc":"2.0","id":id,"result":{"sites":[],"diagnostics":[]}})
        );
    }
    p.eof();
    assert!(p.wait().success());
}

#[test]
fn persistent_process_handles_multiple_evaluations_shutdown_and_eof() {
    let mut p = Process::new();
    p.initialize();
    for id in ["evaluation-one", "evaluation-two", "unicode-😀"] {
        p.request(id, "analysis/evaluate", empty_evaluation(id));
        assert_eq!(
            p.receive(),
            json!({"jsonrpc":"2.0","id":id,"result":{"sites":[],"diagnostics":[]}})
        );
    }
    p.request("stop", "shutdown", json!({}));
    assert_eq!(
        p.receive(),
        json!({"jsonrpc":"2.0","id":"stop","result":null})
    );
    p.request(
        "after-stop",
        "analysis/evaluate",
        empty_evaluation("after-stop"),
    );
    assert_eq!(p.receive()["error"]["code"], -32002);
    p.send(json!({"jsonrpc":"2.0","method":"exit"}));
    assert!(p.wait().success());
    let mut p = Process::new();
    p.initialize();
    p.eof();
    assert!(p.wait().success());
}
#[test]
fn protocol_or_required_capability_mismatch_is_rejected() {
    for params in [
        json!({"protocol":{"major":2,"minor":0},"provider":"mcdev","capabilities":["semantic-v1"]}),
        json!({"protocol":{"major":1,"minor":0},"provider":"mcdev","capabilities":[]}),
        json!({"protocol":{"major":1,"minor":0},"provider":"mcdev","capabilities":["semantic-v1"],"requiredCapabilities":["jvm-bytecode-v99"]}),
    ] {
        let mut p = Process::new();
        p.request("init", "initialize", params);
        assert_eq!(p.receive()["error"]["code"], -32602);
        assert!(!p.wait().success());
    }
}
#[test]
fn uninitialized_foreign_owner_and_evaluation_tokens_never_produce_contributions() {
    let mut p = Process::new();
    p.request("before", "analysis/evaluate", empty_evaluation("before"));
    assert_eq!(p.receive()["error"]["code"], -32002);
    p.initialize();
    p.request(
        "wrong-token",
        "analysis/evaluate",
        empty_evaluation("other"),
    );
    assert_eq!(p.receive()["error"]["code"], -32602);
    let mut evaluation = empty_evaluation("bad-owner");
    evaluation["source"] = json!("member");
    evaluation["declarations"] = json!([{"id":"d1","owner":"foreign","name":"member","range":{"start":0,"end":6},"annotations":[]}]);
    p.request("bad-owner", "analysis/evaluate", evaluation);
    assert_eq!(p.receive()["error"]["code"], -32602);
    p.eof();
    assert!(p.wait().success());
}

fn reading_evaluation(id: &str) -> serde_json::Value {
    json!({"evaluation":id,"owner":"Owner.java","source":"@Mixin class Owner","configuration":{},"declarations":[{"id":"decl-owner","owner":null,"name":"Owner","range":{"start":13,"end":18},"annotations":[{"typeRef":"ref-mixin","useSiteTarget":null,"range":{"start":0,"end":6},"arguments":[]}]}]})
}

#[test]
fn cancel_suppresses_old_results_and_late_batch_response_does_not_mix_with_next_evaluation() {
    let mut p = Process::new();
    p.initialize();
    p.request("old", "analysis/evaluate", reading_evaluation("old"));
    let batch = p.receive();
    assert_eq!(batch["method"], "analysis/readFacts");
    assert_eq!(batch["params"]["evaluation"], "old");
    p.send(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"old"}}));
    assert_eq!(
        p.receive(),
        json!({"jsonrpc":"2.0","id":"old","result":{"sites":[],"diagnostics":[]}})
    );
    p.send(json!({"jsonrpc":"2.0","id":batch["id"],"result":{"facts":[{"state":"found","value":{"id":"old-type","qualifiedName":"org.spongepowered.asm.mixin.Mixin","package":"org.spongepowered.asm.mixin"}}]}}));
    p.request("fresh", "analysis/evaluate", empty_evaluation("fresh"));
    assert_eq!(p.receive()["id"], "fresh");
    p.eof();
    assert!(p.wait().success());
}
#[test]
fn stale_error_and_foreign_response_ids_do_not_become_missing_target_diagnostics() {
    let mut p = Process::new();
    p.initialize();
    for (id, foreign) in [("stale", false), ("foreign", true)] {
        p.request(id, "analysis/evaluate", reading_evaluation(id));
        let request = p.receive();
        if foreign {
            p.send(json!({"jsonrpc":"2.0","id":"some-other-evaluation-rpc","result":{"facts":[]}}));
            let response = p.receive();
            assert_eq!(response["id"], id);
            assert_eq!(response["error"]["code"], -32602);
            assert!(response.get("result").is_none());
        } else {
            p.send(json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32602,"message":"Stale"}}));
            assert_eq!(p.receive()["result"], json!({"sites":[],"diagnostics":[]}));
        }
    }
    p.eof();
    assert!(p.wait().success());
}
#[test]
fn shutdown_and_eof_interrupt_an_outstanding_fact_read() {
    let mut p = Process::new();
    p.initialize();
    p.request("old", "analysis/evaluate", reading_evaluation("old"));
    let _ = p.receive();
    p.request("shutdown", "shutdown", json!({}));
    assert_eq!(p.receive()["id"], "shutdown");
    assert_eq!(p.receive()["result"], json!({"sites":[],"diagnostics":[]}));
    p.send(json!({"jsonrpc":"2.0","method":"exit"}));
    assert!(p.wait().success());
    let mut p = Process::new();
    p.initialize();
    p.request("old", "analysis/evaluate", reading_evaluation("old"));
    let _ = p.receive();
    p.eof();
    assert!(p.wait().success());
}

#[test]
fn malformed_unknown_and_wrongly_typed_handles_are_rejected_without_partial_results() {
    let mut p = Process::new();
    p.initialize();
    for (id, fact) in [
        ("bad-reasons", json!({"state":"unknown","reasons":[]})),
        (
            "wrong-kind",
            json!({"state":"found","value":{"id":"decl-owner","qualifiedName":"org.spongepowered.asm.mixin.Mixin"}}),
        ),
    ] {
        p.request(id, "analysis/evaluate", reading_evaluation(id));
        let batch = p.receive();
        p.send(json!({"jsonrpc":"2.0","id":batch["id"],"result":{"facts":[fact]}}));
        let response = p.receive();
        assert_eq!(response["id"], id);
        assert_eq!(response["error"]["code"], -32602);
        assert!(response.get("result").is_none());
    }
    p.eof();
    assert!(p.wait().success());
}

#[test]
fn foreign_cancel_is_ignored_and_request_ids_cannot_alias_the_evaluation_token() {
    let mut p = Process::new();
    p.initialize();
    let id = "mcdev-rpc:1";
    p.request(id, "analysis/evaluate", reading_evaluation(id));
    let batch = p.receive();
    assert_ne!(batch["id"], id);
    p.send(
        json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"foreign-evaluation"}}),
    );
    p.send(json!({"jsonrpc":"2.0","id":batch["id"],"result":{"facts":[{"state":"found","value":{"id":"type-mixin","qualifiedName":"org.spongepowered.asm.mixin.Mixin"}}]}}));
    let response = p.receive();
    assert_eq!(response["id"], id);
    assert_eq!(
        response["result"]["diagnostics"].as_array().unwrap().len(),
        1
    );
    assert!(response["result"]["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .starts_with("[mcdev.mixin.incomplete]"));
    p.eof();
    assert!(p.wait().success());
}
