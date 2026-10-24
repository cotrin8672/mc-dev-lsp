//! Native test harness for the unpublished two-op adapter, not a production mode.
use super::two_ops_support::*;
use mcdev_companion::{
    host_facts::{bridge::*, FactsError},
    transport::{read_frame, write_frame},
    wire::{Fact, HandleKind, Registry, TypeFact},
};
use mcdev_core::{
    McCompletionItem, McCompletionKind, McCompletionMetadata, McCompletionReplacementRange,
};
use mcdev_protocol::{
    convert::{CompletionConvertContext, CompletionItemConverter},
    McdevCompletionResponse, McdevResponseEnvelope,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::BufReader,
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
struct Rpc {
    stream: BufReader<TcpStream>,
    next: u64,
}
impl FactReader for Rpc {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.next += 1;
        let id = format!("two-op-rpc-{}", self.next);
        write_frame(self.stream.get_mut(),&json!({"jsonrpc":"2.0","id":id,"method":"analysis/readFacts","params":{"evaluation":evaluation,"queries":queries}})).map_err(|e|FactsError::Protocol(e.to_string()))?;
        let response = read_frame(&mut self.stream)
            .map_err(|e| FactsError::Protocol(e.to_string()))?
            .ok_or_else(|| FactsError::Cancelled("host EOF".into()))?;
        if response["id"] != id {
            return Err(FactsError::Protocol("foreign RPC id".into()));
        }
        if response["error"]["message"] == "Stale" {
            return Err(FactsError::Cancelled(
                "host rejected stale transaction".into(),
            ));
        }
        if response.get("error").is_some() {
            return Err(FactsError::Protocol("host rejected optional read".into()));
        }
        Ok(response["result"].clone())
    }
}
pub fn worker() {
    let port = std::env::var("MCDEV_TWO_OPS_TEST_PORT")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut rpc = Rpc {
        stream: BufReader::new(socket),
        next: 0,
    };
    let result = (|| -> Result<Value, FactsError> {
        let resolved = rpc.read_facts(
            EVALUATION,
            &[json!({"op":"resolveType","name":"fixture.Model"})],
        )?;
        let fact: Fact<TypeFact> = serde_json::from_value(resolved["facts"][0].clone())
            .map_err(|e| FactsError::Protocol(e.to_string()))?;
        let target = fact
            .found()
            .ok_or_else(|| FactsError::Unknown("type unavailable".into()))?;
        let mut registry = Registry::default();
        registry
            .register(&target.id, HandleKind::Type)
            .map_err(FactsError::Protocol)?;
        let ops = ops();
        let mut scoped = ScopedFacts::new(EVALUATION, &ops, &mut registry, &mut rpc)?;
        let project = scoped.project_context()?;
        let ty = scoped.type_jvm(&target.id)?;
        let core = abi_members(&ty)?;
        let items = core
            .methods
            .iter()
            .map(|method| McCompletionItem {
                label: method.readable_signature.clone(),
                detail: None,
                documentation: None,
                filter_text: method.name.clone(),
                insert_text: format!("{}{}", method.name, method.descriptor),
                kind: McCompletionKind::Method,
                sort_key: method.name.clone(),
                metadata: McCompletionMetadata {
                    source: "fixture.rawAbi".into(),
                    owner: Some(core.internal_name.clone()),
                    name: Some(method.name.clone()),
                    descriptor: Some(method.descriptor.clone()),
                    namespace: None,
                },
                additional_edits: vec![],
                insert_text_format: Default::default(),
            })
            .collect::<Vec<_>>();
        let mut conversion = CompletionConvertContext::new("😀des", None);
        conversion.replacement_range = Some(McCompletionReplacementRange {
            start_offset: 4,
            end_offset: 7,
        });
        let dtos = CompletionItemConverter::to_dtos(&items, &conversion)
            .map_err(|e| FactsError::Protocol(e.to_string()))?;
        Ok(
            json!({"project":project,"typeJvm":ty,"coreMembers":core,"response":McdevResponseEnvelope::success(McdevCompletionResponse{items:dtos,is_incomplete:false,warnings:vec![],debug:None},["completion".to_string()])}),
        )
    })();
    let result = match result {
        Ok(value) => value,
        Err(FactsError::Cancelled(reason)) => json!({"cancelled":true,"reason":reason}),
        Err(error) => {
            serde_json::to_value(McdevResponseEnvelope::<Value>::failure(error.protocol())).unwrap()
        }
    };
    write_frame(
        rpc.stream.get_mut(),
        &json!({"jsonrpc":"2.0","method":"fixture/result","params":result}),
    )
    .unwrap();
}
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub fn run() {
    for mode in ["complete", "source-only", "stale"] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../build/two-ops-process-logs")
            .join(format!(
                "{}-{mode}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&dir).unwrap();
        let mut child = Owned(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "two_ops_process_worker",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(
                    "MCDEV_TWO_OPS_TEST_PORT",
                    listener.local_addr().unwrap().port().to_string(),
                )
                .stdout(Stdio::from(
                    fs::File::create(dir.join("worker.stdout.log")).unwrap(),
                ))
                .stderr(Stdio::from(
                    fs::File::create(dir.join("worker.stderr.log")).unwrap(),
                ))
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        let socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "worker connect deadline");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("{e}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut stream = BufReader::new(socket);
        let mut transcript = vec![];
        let mut reader = Reader::new();
        if mode == "source-only" {
            reader.ty["abi"] = unknown("SourceOnly");
            reader.ty["jvmAbiCoverage"] = unknown("SourceOnly");
        }
        let output = loop {
            assert!(Instant::now() < deadline);
            let request = read_frame(&mut stream).unwrap().unwrap();
            transcript.push(json!({"direction":"childToHost","message":request}));
            if request["method"] == "fixture/result" {
                break request["params"].clone();
            }
            assert_eq!(request["method"], "analysis/readFacts");
            assert_eq!(request["params"]["evaluation"], EVALUATION);
            let queries = request["params"]["queries"].as_array().unwrap();
            let response = if mode == "stale" && queries[0]["op"] == "typeJvm" {
                json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32602,"message":"Stale"}})
            } else {
                let result = if queries[0]["op"] == "resolveType" {
                    json!({"facts":[{"state":"found","value":{"id":TYPE,"qualifiedName":"fixture.Model","package":"fixture"}}]})
                } else {
                    reader.read_facts(EVALUATION, queries).unwrap()
                };
                json!({"jsonrpc":"2.0","id":request["id"],"result":result})
            };
            transcript.push(json!({"direction":"hostToChild","message":response}));
            write_frame(stream.get_mut(), &response).unwrap();
        };
        fs::write(
            dir.join("frames.jsonl"),
            transcript
                .iter()
                .map(|v| v.to_string() + "\n")
                .collect::<String>(),
        )
        .unwrap();
        fs::write(
            dir.join("result.json"),
            serde_json::to_vec_pretty(&output).unwrap(),
        )
        .unwrap();
        match mode {
            "complete" => {
                assert_eq!(output["typeJvm"], ty());
                assert_eq!(
                    output["coreMembers"]["methods"].as_array().unwrap().len(),
                    6
                );
                let items = output["response"]["result"]["items"].as_array().unwrap();
                assert_eq!(items.len(), 6);
                assert!(items.iter().any(|i| i["metadata"]["name"] == "<clinit>"));
                assert!(items
                    .iter()
                    .all(|i| i["edit"]["range"]["start"]["character"] == 2
                        && i["edit"]["range"]["end"]["character"] == 5));
            }
            "source-only" => {
                assert_eq!(output["error"]["code"], "INCOMPLETE_PROJECT_CONTEXT");
                assert!(output.get("result").is_none());
            }
            _ => {
                assert_eq!(output["cancelled"], true);
                assert!(output.get("result").is_none());
            }
        }
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "worker exit deadline; {}",
                dir.display()
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(status.success(), "{}", dir.display());
    }
}
