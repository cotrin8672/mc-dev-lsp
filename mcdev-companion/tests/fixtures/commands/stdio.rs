use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn request(id: i32, source: &str, character: usize) -> Value {
    json!({"id":id,"payload":{"context":{"protocolVersion":1,"workspaceRoot":"file:///workspace","documentUri":"file:///workspace/Mixin.java","languageId":"java","position":{"line":0,"character":character},"bufferText":source,"documentVersion":1,"client":{"name":"stdio-test","version":"1"}},"trigger":{"kind":"manual"},"options":{"preferredAtTarget":"smart","mixinClassInsert":"import","injectMethodDescriptor":"auto"}}})
}
pub fn replay() {
    let source = "@Inject(method = \"tick\", at = @At(value = \"INVOKE\", target = \"\"))";
    let requests = vec![
        request(1, "@Inject(meth", 12).to_string(),
        request(2, "@Expression(\"{ ret", 18).to_string(),
        "{malformed".into(),
        request(3, "@Inject(meth", 12).to_string(),
        request(
            4,
            source,
            source.find("target = \"").unwrap() + "target = \"".len(),
        )
        .to_string(),
    ];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../build/command-line-process-logs")
        .join(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .to_string(),
        );
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("requests.jsonl"), requests.join("\n") + "\n").unwrap();
    let mut child = OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_mcdev-companion"))
            .arg("--stdio-completion")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(dir.join("stderr.log")).unwrap())
            .spawn()
            .unwrap(),
    );
    let mut output = child.0.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        output.read_to_end(&mut bytes).unwrap();
        bytes
    });
    {
        let mut input = child.0.stdin.take().unwrap();
        for request in &requests {
            writeln!(input, "{request}").unwrap();
        }
        input.flush().unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "stdio helper did not exit after EOF; logs {}",
            dir.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        status.success(),
        "stdio helper exit {status}; logs {}",
        dir.display()
    );
    let bytes = reader.join().unwrap();
    fs::write(dir.join("responses.jsonl"), &bytes).unwrap();
    let output = String::from_utf8(bytes).unwrap();
    let responses = output
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), requests.len(), "{}", dir.display());
    let first = &responses[0];
    assert_eq!(first["id"], 1);
    assert_eq!(first["handled"], true);
    let first_item = &first["response"]["result"]["items"][0];
    assert_eq!(first_item["edit"]["range"]["start"]["character"], 8);
    assert_eq!(first_item["edit"]["range"]["end"]["character"], 12);
    let second = &responses[1];
    assert_eq!(second["id"], 2);
    assert_eq!(second["handled"], true);
    assert!(!second["response"]["result"]["items"][0]["edit"]["newText"]
        .as_str()
        .unwrap()
        .is_empty());
    let malformed = &responses[2];
    assert_eq!(malformed["handled"], false);
    assert_eq!(malformed["error"]["code"], "PARSE_ERROR");
    let recovered = &responses[3];
    assert_eq!(recovered["id"], 3);
    assert_eq!(recovered["handled"], true);
    assert!(!recovered["response"]["result"]["items"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(responses[4]["id"], 4);
    assert_eq!(responses[4]["handled"], false);
}

#[test]
fn line_protocol_retains_ids_and_recovers_from_each_invalid_payload() {
    use mcdev_companion::completion_lines::{complete_no_index, process_completion_line_with};
    for (line, id) in [
        ("{malformed", None),
        ("[]", None),
        ("null", None),
        ("{\"id\":\"😀\",\"payload\":[]}", Some(json!("😀"))),
        ("{\"id\":7,\"payload\":{}}", Some(json!(7))),
        ("{\"id\":null}", None),
    ] {
        let response = process_completion_line_with(line.as_bytes(), &|_| {
            panic!("invalid payload dispatched")
        })
        .unwrap();
        assert_eq!(response.id, id);
        assert!(!response.handled);
        assert_eq!(response.error.unwrap().code, "PARSE_ERROR");
    }
    let mut unsupported = request(8, "@Inject(meth", 12);
    unsupported["payload"]["context"]["protocolVersion"] = json!(99);
    let response =
        process_completion_line_with(unsupported.to_string().as_bytes(), &complete_no_index)
            .unwrap();
    assert_eq!(response.id, Some(json!(8)));
    assert!(!response.handled);
    assert!(response.error.is_none());
    let recovered = process_completion_line_with(
        request(9, "@Inject(meth", 12).to_string().as_bytes(),
        &complete_no_index,
    )
    .unwrap();
    assert!(recovered.handled);
    assert_eq!(recovered.id, Some(json!(9)));
}
