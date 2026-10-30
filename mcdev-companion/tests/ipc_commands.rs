//! Exercises the executable and its real command dispatcher over framed stdio.
use mcdev_companion::transport::{read_frame, write_frame};
use mcdev_core::project::path_to_uri;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

struct ChildServer {
    child: Child,
    input: ChildStdin,
    output: Receiver<Result<Value, String>>,
}

impl ChildServer {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mcdev-companion"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn companion");
        let input = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (tx, output) = mpsc::channel();
        thread::spawn(move || loop {
            match read_frame(&mut stdout) {
                Ok(Some(value)) => {
                    if tx.send(Ok(value)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    break;
                }
            }
        });
        Self {
            child,
            input,
            output,
        }
    }
    fn send(&mut self, value: Value) {
        write_frame(&mut self.input, &value).unwrap();
    }
    fn receive(&self) -> Value {
        self.output
            .recv_timeout(Duration::from_secs(15))
            .expect("companion response deadline exceeded")
            .expect("invalid companion response")
    }
    fn initialize(&mut self) -> Value {
        self.initialize_commands(&[
            "mcdev.completion",
            "mcdev.diagnostics",
            "mcdev.context",
            "mcdev.definition",
            "mcdev.hover",
            "mcdev.references",
            "mcdev.codeAction",
            "mcdev.reindex",
            "mcdev.reloadProjectContext",
            "mcdev.dumpContext",
            "mcdev.info",
        ])
    }
    fn initialize_commands(&mut self, commands: &[&str]) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{
            "provider":"mcdev.test","protocol":{"major":1,"minor":0},
            "capabilities":["semantic-v1","provider-commands-v1","project-class-facts-v1",
                "source-snapshots-v1","owned-resources-v1","runtime-class-bytes-v1","jvm-catalog-v1"],
            "supportedOps":{"project-class-facts-v1":["projectContext","typeJvm"],
                "source-snapshots-v1":["sourceInventory","sourceSnapshot"],
                "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"],
                "runtime-class-bytes-v1":["runtimeClass","classBytes"],
                "jvm-catalog-v1":["jvmCatalog","jvmClass"]},
            "commands":commands
        }}));
        self.receive()
    }
}

impl Drop for ChildServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn root() -> PathBuf {
    std::env::temp_dir().join(format!("mcdev-ipc-process-{}", std::process::id()))
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}

fn base64(bytes: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for part in bytes.chunks(3) {
        let a = part[0];
        let b = *part.get(1).unwrap_or(&0);
        let c = *part.get(2).unwrap_or(&0);
        out.push(alphabet[(a >> 2) as usize] as char);
        out.push(alphabet[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        out.push(if part.len() > 1 {
            alphabet[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if part.len() > 2 {
            alphabet[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn command(root: &Path, evaluation: &str, name: &str, source: &str, position: i32) -> Value {
    let document = root.join("Owner.java");
    let uri = path_to_uri(&document);
    json!({"jsonrpc":"2.0","id":evaluation,"method":"analysis/executeCommand","params":{
        "evaluation":evaluation,"owner":document.to_string_lossy(),"source":source,
        "configuration":{},"declarations":[],"command":name,
        "context":{"workspaceRoot":root.to_string_lossy(),"documentUri":uri,
            "sourceSha256":sha(source.as_bytes()),"documentVersion":7,"languageId":"java"},
        "arguments":[{"context":{"workspaceRoot":path_to_uri(root),"documentUri":uri,
            "languageId":"java","position":{"line":0,"character":position},
            "bufferText":source,"documentVersion":7}}]
    }})
}

#[test]
fn framed_companion_routes_completion_and_rejects_a_stale_inner_context() {
    let mut server = ChildServer::start();
    let acknowledgement = server.initialize();
    let commands = acknowledgement["result"]["commands"].as_array().unwrap();
    assert!(commands.contains(&json!("mcdev.completion")));
    assert!(commands.contains(&json!("mcdev.info")));

    let root = root();
    let mut stale = command(&root, "eval-1", "mcdev.completion", "@Inject(meth", 12);
    stale["params"]["arguments"][0]["context"]["bufferText"] = json!("changed");
    server.send(stale);
    let rejected = server.receive();
    assert_eq!(rejected["id"], "eval-1");
    assert_eq!(rejected["error"]["code"], -32801);

    server.send(command(
        &root,
        "eval-2",
        "mcdev.completion",
        "@Inject(meth",
        12,
    ));
    let result = server.receive();
    assert_eq!(result["id"], "eval-2");
    assert!(result["result"]["result"]["items"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
}

#[test]
fn project_command_reads_host_snapshots_and_keeps_missing_semantics_unavailable() {
    let mut server = ChildServer::start();
    let acknowledgement = server.initialize();
    assert!(acknowledgement["result"]["commands"]
        .as_array()
        .unwrap()
        .contains(&json!("mcdev.diagnostics")));
    let root = root();
    let path = root.join("Owner.java");
    let uri = path_to_uri(&path);
    let text = "class Owner {}";
    server.send(command(&root, "eval-3", "mcdev.diagnostics", text, 0));
    let mut observed = Vec::new();
    let final_response = loop {
        let request = server.receive();
        if request["id"] == "eval-3" {
            break request;
        }
        assert_eq!(request["method"], "analysis/readFacts");
        assert_eq!(request["params"]["evaluation"], "eval-3");
        let queries = request["params"]["queries"].as_array().unwrap();
        let values: Vec<Value> = queries.iter().map(|query| {
            let op = query["op"].as_str().unwrap();
            observed.push(op.to_owned());
            let descriptor = json!({"id":"src:1","displayPath":path.to_string_lossy(),
                "documentUri":found(json!(uri)),"languageId":"java",
                "identity":{"kind":"source","revision":"1"}});
            match op {
                "projectContext" => json!({"workspaceUri":{"state":"unknown","reasons":[{"code":"MissingFacts"}]},
                    "scope":found(json!({"module":":demo","sourceSet":"main"})),
                    "sourceRoots":found(json!([root.to_string_lossy()])),
                    "outputRoots":found(json!([root.join("out").to_string_lossy()])),
                    "classpath":found(json!({"entries":[],"coverage":found(Value::Null)})),
                    "sdk":found(json!({"identity":"sdk","api":{"kind":"full"}}))}),
                "sourceInventory" => json!({"sources":[descriptor],"next":null,"coverage":found(Value::Null)}),
                "sourceSnapshot" => {
                    assert_eq!(query["source"], "src:1");
                    let mut snapshot = descriptor;
                    snapshot["text"] = json!(text);
                    snapshot["sha256"] = json!(sha(text.as_bytes()));
                    found(snapshot)
                }
                "resourceInventory" => json!({"resources":[],"next":null,"coverage":found(json!(true))}),
                other => panic!("unexpected host query {other}"),
            }
        }).collect();
        server.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"facts":values}}));
    };
    assert_eq!(
        observed,
        [
            "projectContext",
            "sourceInventory",
            "sourceSnapshot",
            "resourceInventory"
        ]
    );
    assert_eq!(
        final_response["result"]["error"]["code"],
        "INCOMPLETE_PROJECT_CONTEXT"
    );
    assert!(final_response["result"]["result"].is_null());
}

#[test]
fn unknown_sdk_does_not_block_sdk_free_awat_diagnostics() {
    let mut server = ChildServer::start();
    let acknowledgement = server.initialize();
    assert!(acknowledgement["result"]["commands"]
        .as_array()
        .unwrap()
        .contains(&json!("mcdev.diagnostics")));
    let root = root();
    let path = root.join("access.aw");
    let uri = path_to_uri(&path);
    for (evaluation, text, has_class) in [
        ("eval-aw-header", "accessWidener v2 named\n", false),
        (
            "eval-aw-class",
            "accessWidener v2 named\naccessible class java/lang/Object\n",
            true,
        ),
    ] {
        let mut request = command(&root, evaluation, "mcdev.diagnostics", text, 0);
        request["params"]["owner"] = json!(path.to_string_lossy());
        request["params"]["context"]["documentUri"] = json!(uri);
        request["params"]["context"]["languageId"] = json!("accesswidener");
        request["params"]["arguments"][0]["context"]["documentUri"] = json!(uri);
        request["params"]["arguments"][0]["context"]["languageId"] = json!("accesswidener");
        server.send(request);
        let mut observed = Vec::new();
        let result = loop {
            let request = server.receive();
            if request["id"] == evaluation {
                break request;
            }
            assert_eq!(request["method"], "analysis/readFacts");
            assert_eq!(request["params"]["evaluation"], evaluation);
            let queries = request["params"]["queries"].as_array().unwrap();
            let values: Vec<Value> = queries.iter().map(|query| {
                let op = query["op"].as_str().unwrap();
                observed.push(op.to_owned());
                match op {
                    "projectContext" => json!({"workspaceUri":{"state":"unknown","reasons":[{"code":"MissingFacts"}]},
                        "scope":found(json!({"module":":demo","sourceSet":"main"})),
                        "sourceRoots":found(json!([root.to_string_lossy()])),
                        "outputRoots":found(json!([root.join("out").to_string_lossy()])),
                        "classpath":found(json!({"entries":[],"coverage":found(Value::Null)})),
                        "sdk":{"state":"unknown","reasons":[{"code":"MissingFacts"}]}}),
                    "sourceInventory" => json!({"sources":[],"next":null,"coverage":found(Value::Null)}),
                    "resourceInventory" => json!({"resources":[{"id":"resource:1","relativePath":"access.aw",
                        "association":"source","classpathOrder":0,"editable":true,"documentUri":found(json!(uri)),
                        "documentVersion":7,"identity":{"kind":"resource","revision":"1"}}],
                        "next":null,"coverage":found(json!(true))}),
                    "resourceSnapshot" => {
                        assert_eq!(query["resource"], "resource:1");
                        found(json!({"identity":{"kind":"resource","revision":"1","sha256":sha(text.as_bytes())},
                            "documentVersion":7,"size":text.len(),"offset":0,"base64":base64(text.as_bytes()),
                            "utf8":true,"text":found(json!(text)),"next":null}))
                    }
                    other => panic!("unexpected host query {other}"),
                }
            }).collect();
            server.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"facts":values}}));
        };
        assert_eq!(
            observed,
            [
                "projectContext",
                "sourceInventory",
                "resourceInventory",
                "resourceSnapshot"
            ]
        );
        if has_class {
            assert_eq!(
                result["result"]["error"]["code"],
                "INCOMPLETE_PROJECT_CONTEXT"
            );
            assert!(result["result"]["result"].is_null());
        } else {
            assert!(result["result"]["error"].is_null(), "{result}");
            assert_eq!(result["result"]["result"]["diagnostics"], json!([]));
        }
    }
}

#[test]
fn command_acknowledgement_is_the_configured_implemented_intersection() {
    let mut server = ChildServer::start();
    let ack =
        server.initialize_commands(&["mcdev.info", "mcdev.references", "mcdev.unimplemented"]);
    assert_eq!(
        ack["result"]["commands"],
        json!(["mcdev.info", "mcdev.references"])
    );

    let mut server = ChildServer::start();
    let ack = server.initialize();
    assert_eq!(
        ack["result"]["commands"],
        json!([
            "mcdev.codeAction",
            "mcdev.completion",
            "mcdev.context",
            "mcdev.definition",
            "mcdev.diagnostics",
            "mcdev.dumpContext",
            "mcdev.hover",
            "mcdev.info",
            "mcdev.references",
            "mcdev.reindex",
            "mcdev.reloadProjectContext"
        ])
    );
}

#[test]
fn kotlin_code_action_returns_the_original_empty_dto_without_host_reads() {
    let mut server = ChildServer::start();
    server.initialize();
    let root = root();
    let path = root.join("Owner.kt");
    let uri = path_to_uri(&path);
    for (evaluation, version) in [("kotlin-actions", 1), ("kotlin-actions-mismatch", 2)] {
        let mut request = command(&root, evaluation, "mcdev.codeAction", "class Owner", 0);
        request["params"]["owner"] = json!(path.to_string_lossy());
        request["params"]["context"]["documentUri"] = json!(uri);
        request["params"]["context"]["languageId"] = json!("kotlin");
        let context = &mut request["params"]["arguments"][0]["context"];
        context["documentUri"] = json!(uri);
        context["languageId"] = json!("kotlin");
        context["protocolVersion"] = json!(version);
        server.send(request);
        let response = server.receive();
        assert_eq!(
            response["id"], evaluation,
            "must not request host facts: {response}"
        );
        if version == 1 {
            assert_eq!(
                response["result"],
                json!({
                    "protocolVersion":1,"serverVersion":"0.1.0","capabilities":["codeAction"],
                    "result":{"actions":[]}
                })
            );
        } else {
            assert_eq!(response["result"]["error"]["code"], "PROTOCOL_MISMATCH");
            assert!(response["result"]["result"].is_null());
        }
    }
}

fn owner_only_response(server: &mut ChildServer, evaluation: &str, root: &Path) -> Value {
    for _ in 0..32 {
        let request = server.receive();
        if request["id"] == evaluation {
            return request;
        }
        assert_eq!(request["method"], "analysis/readFacts");
        assert_eq!(request["params"]["evaluation"], evaluation);
        let values = request["params"]["queries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|query| match query["op"].as_str().unwrap() {
                "projectContext" => json!({"workspaceUri":found(json!(path_to_uri(root))),
                    "scope":found(json!({"module":":demo","sourceSet":"main"})),
                    "sourceRoots":found(json!([root.to_string_lossy()])),
                    "outputRoots":found(json!([root.join("out").to_string_lossy()])),
                    "classpath":found(json!({"entries":[],"coverage":found(Value::Null)})),
                    "sdk":{"state":"unknown","reasons":[{"code":"MissingFacts"}]}}),
                "sourceInventory" => {
                    json!({"sources":[],"next":null,"coverage":found(Value::Null)})
                }
                "resourceInventory" => {
                    json!({"resources":[],"next":null,"coverage":found(json!(true))})
                }
                "resolveTypeRef" => {
                    let (id, name) = match query["reference"].as_str().unwrap() {
                        "ref-mixin" => ("type-mixin", "org.spongepowered.asm.mixin.Mixin"),
                        "ref-target" => ("type-target", "game.Target"),
                        other => panic!("unexpected reference {other}"),
                    };
                    found(json!({"id":id,"qualifiedName":name,"package":"game"}))
                }
                "declarationType" => found(json!({"id":"type-owner","qualifiedName":"Owner","package":""})),
                "typeJvm" => {
                    assert_eq!(query["target"], "type-target");
                    let unknown = json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]});
                    json!({"binaryName":found(json!({"internalName":"game/Target","descriptor":"Lgame/Target;"})),
                        "classFlags":unknown,"declaredMembers":{"members":[],"coverage":unknown},
                        "abi":unknown,"jvmAbiCoverage":unknown})
                }
                other => panic!("unexpected host query {other}"),
            })
            .collect::<Vec<_>>();
        server.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"facts":values}}));
    }
    panic!("command exceeded bounded host query count")
}

#[test]
fn owner_only_commands_preserve_typed_incompleteness_and_refresh_authority() {
    let mut server = ChildServer::start();
    server.initialize();
    let root = root();
    for (index, name, refreshed) in [
        (0, "mcdev.info", false),
        (1, "mcdev.dumpContext", false),
        (2, "mcdev.codeAction", false),
        (3, "mcdev.reindex", false),
        (4, "mcdev.reloadProjectContext", false),
        (5, "mcdev.reindex", true),
        (6, "mcdev.reloadProjectContext", true),
    ] {
        let evaluation = format!("incomplete-{index}");
        let mut request = command(&root, &evaluation, name, "class Owner {}", 0);
        if refreshed {
            request["params"]["context"]["refreshGeneration"] = json!("129");
        }
        server.send(request);
        let response = owner_only_response(&mut server, &evaluation, &root);
        let message =
            if !refreshed && matches!(name, "mcdev.reindex" | "mcdev.reloadProjectContext") {
                "host project refresh authority is unavailable"
            } else {
                "complete project source-set and resource inventory is unavailable"
            };
        assert_eq!(
            response["result"],
            json!({"protocolVersion":1,"serverVersion":"0.1.0",
            "capabilities":[],"error":{"code":"INCOMPLETE_PROJECT_CONTEXT","message":message,"details":{}}}),
            "{name}"
        );
    }
}

#[test]
fn references_return_empty_only_when_definition_targets_are_known_empty() {
    let mut server = ChildServer::start();
    server.initialize();
    let root = root();
    let path = root.join("access.aw");
    let uri = path_to_uri(&path);
    let mut request = command(
        &root,
        "empty-references",
        "mcdev.references",
        "accessWidener v2 named\n",
        0,
    );
    request["params"]["owner"] = json!(path.to_string_lossy());
    request["params"]["context"]["documentUri"] = json!(uri);
    request["params"]["context"]["languageId"] = json!("accesswidener");
    request["params"]["arguments"][0]["context"]["documentUri"] = json!(uri);
    request["params"]["arguments"][0]["context"]["languageId"] = json!("accesswidener");
    server.send(request);
    let response = owner_only_response(&mut server, "empty-references", &root);
    assert_eq!(
        response["result"],
        json!({
            "protocolVersion":1,"serverVersion":"0.1.0","capabilities":["references"],
            "result":{"locations":[]}
        })
    );

    server.send(command(
        &root,
        "unknown-references",
        "mcdev.references",
        "@Mixin(Target.class) class Owner {}",
        10,
    ));
    let response = owner_only_response(&mut server, "unknown-references", &root);
    assert_eq!(
        response["result"]["error"]["code"],
        "INCOMPLETE_PROJECT_CONTEXT"
    );
    assert!(
        response["result"]["result"].is_null(),
        "unknown definitions must not become empty references"
    );
}

#[test]
fn available_ipc_source_fallback_cannot_prove_empty_references() {
    let mut server = ChildServer::start();
    server.initialize();
    let root = root();
    for (language, extension, literal) in [
        ("java", "java", "Target.class"),
        ("kotlin", "kt", "Target::class"),
    ] {
        let source = format!("@org.spongepowered.asm.mixin.Mixin({literal}) class Owner {{}}");
        let evaluation = format!("fallback-references-{language}");
        let path = root.join(format!("Owner.{extension}"));
        let uri = path_to_uri(&path);
        let mut request = command(
            &root,
            &evaluation,
            "mcdev.references",
            &source,
            source.len() as i32,
        );
        request["params"]["owner"] = json!(path.to_string_lossy());
        request["params"]["context"]["documentUri"] = json!(uri);
        request["params"]["context"]["languageId"] = json!(language);
        request["params"]["arguments"][0]["context"]["documentUri"] = json!(uri);
        request["params"]["arguments"][0]["context"]["languageId"] = json!(language);
        let owner = source.find("Owner").unwrap();
        let target = source.find(literal).unwrap();
        request["params"]["declarations"] = json!([{
            "id":"owner", "owner":null,"name":"Owner","range":{"start":owner,"end":owner+5},
            "annotations":[{"typeRef":"ref-mixin","useSiteTarget":null,
                "range":{"start":0,"end":source.find(')').unwrap()+1},
                "arguments":[{"name":null,"range":{"start":target,"end":target+literal.len()},
                    "value":found(json!({"kind":"class","typeRef":"ref-target"}))}]}]
        }]);
        server.send(request);
        let response = owner_only_response(&mut server, &evaluation, &root);
        assert_eq!(
            response["result"],
            json!({"protocolVersion":1,"serverVersion":"0.1.0","capabilities":[],
                "error":{"code":"INCOMPLETE_PROJECT_CONTEXT",
                    "message":"host source declaration coverage and ranges are unavailable","details":{}}}),
            "available {language} fallback is not authoritative target absence"
        );
    }
}
