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
        self.send(json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{
            "provider":"mcdev.test","protocol":{"major":1,"minor":0},
            "capabilities":["semantic-v1","provider-commands-v1","project-class-facts-v1",
                "source-snapshots-v1","owned-resources-v1","runtime-class-bytes-v1","jvm-catalog-v1"],
            "supportedOps":{"project-class-facts-v1":["projectContext","typeJvm"],
                "source-snapshots-v1":["sourceInventory","sourceSnapshot"],
                "owned-resources-v1":["resourceInventory","resourceLookup","resourceSnapshot"],
                "runtime-class-bytes-v1":["runtimeClass","classBytes"],
                "jvm-catalog-v1":["jvmCatalog","jvmClass"]},
            "commands":["mcdev.completion","mcdev.diagnostics","mcdev.info"]
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
    assert!(!commands.contains(&json!("mcdev.info")));

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
