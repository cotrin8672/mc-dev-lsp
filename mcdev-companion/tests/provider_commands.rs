use mcdev_companion::{
    host_facts::{bridge::FactReader, FactsError},
    provider_commands::{run_with_command_handler, CommandFacts},
    transport::{read_frame, write_frame},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{self, BufReader, Cursor, Read, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

struct ChannelReader {
    chunks: Receiver<Vec<u8>>,
    current: Cursor<Vec<u8>>,
}
impl Read for ChannelReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        loop {
            let count = self.current.read(output)?;
            if count != 0 || output.is_empty() {
                return Ok(count);
            }
            match self.chunks.recv() {
                Ok(bytes) => self.current = Cursor::new(bytes),
                Err(_) => return Ok(0),
            }
        }
    }
}
struct ChannelWriter {
    output: Sender<Vec<u8>>,
    buffer: Vec<u8>,
}
impl Write for ChannelWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            self.output
                .send(std::mem::take(&mut self.buffer))
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "receiver closed"))?;
        }
        Ok(())
    }
}
struct Session {
    input: Sender<Vec<u8>>,
    output: Receiver<Vec<u8>>,
    task: JoinHandle<io::Result<()>>,
}
impl Session {
    fn start<H>(handler: H) -> Self
    where
        H: Fn(Value, Value, CommandFacts) -> Result<Value, FactsError> + Send + Sync + 'static,
    {
        let (input, input_rx) = mpsc::channel();
        let (output_tx, output) = mpsc::channel();
        let task = thread::spawn(move || {
            let reader = ChannelReader {
                chunks: input_rx,
                current: Cursor::new(Vec::new()),
            };
            let writer = ChannelWriter {
                output: output_tx,
                buffer: Vec::new(),
            };
            run_with_command_handler(BufReader::new(reader), writer, handler)
        });
        Self {
            input,
            output,
            task,
        }
    }
    fn send(&self, value: Value) {
        let mut frame = Vec::new();
        write_frame(&mut frame, &value).unwrap();
        self.input.send(frame).unwrap();
    }
    fn receive(&self) -> Value {
        let frame = self
            .output
            .recv_timeout(Duration::from_secs(5))
            .expect("actor did not respond");
        read_frame(&mut Cursor::new(frame)).unwrap().unwrap()
    }
    fn close(self) {
        self.send(json!({"jsonrpc":"2.0","method":"exit"}));
        drop(self.input);
        self.task.join().unwrap().unwrap();
    }
}

fn initialize() -> Value {
    json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{
        "protocol":{"major":1,"minor":0},"provider":"mcdev",
        "capabilities":["semantic-v1","source-snapshots-v1","jvm-catalog-v1","provider-commands-v1"],
        "supportedOps":{"source-snapshots-v1":["sourceInventory","sourceSnapshot","future"],
            "jvm-catalog-v1":["jvmCatalog","jvmClass"]},
        "commands":["mcdev.info","mcdev.completion","unknown.command"]
    }})
}
fn command(id: &str, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"analysis/executeCommand","params":{
        "evaluation":id,"owner":"/workspace/Owner.kt","source":"class Owner",
        "declarations":[],"configuration":{},"command":name,"arguments":arguments,
        "context":{"workspaceRoot":"/workspace","documentUri":"file:///workspace/Owner.kt",
            "documentVersion":7,"refreshGeneration":"opaque-host-generation"}
    }})
}
fn acknowledge(session: &Session) -> Value {
    session.send(initialize());
    let acknowledgement = session.receive();
    assert_eq!(acknowledgement["id"], "init");
    acknowledgement["result"].clone()
}

#[test]
fn negotiation_and_command_preserve_opaque_payload_and_host_facts() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let session = Session::start(move |params, initialization, mut facts| {
        observed.fetch_add(1, Ordering::SeqCst);
        let result = facts.read_facts(
            params["evaluation"].as_str().unwrap(),
            &[json!({"op":"sourceInventory"})],
        )?;
        Ok(json!({"original":params,"acknowledgement":initialization,"host":result}))
    });
    let ack = acknowledge(&session);
    assert_eq!(ack["commands"], json!(["mcdev.completion", "mcdev.info"]));
    assert_eq!(
        ack["supportedOps"]["source-snapshots-v1"],
        json!(["sourceInventory", "sourceSnapshot"])
    );
    assert_eq!(
        ack["supportedOps"]["jvm-catalog-v1"],
        json!(["jvmCatalog", "jvmClass"])
    );

    let original = command(
        "evaluation-1",
        "mcdev.completion",
        json!([{"CallbackName":"KeepCase","text":"😀"}]),
    );
    session.send(original.clone());
    let read = session.receive();
    assert_eq!(read["method"], "analysis/readFacts");
    assert_eq!(
        read["params"],
        json!({"evaluation":"evaluation-1","queries":[{"op":"sourceInventory"}]})
    );
    let payload =
        json!({"facts":[{"sources":[],"coverage":{"state":"found","value":null},"next":null}]});
    session.send(json!({"jsonrpc":"2.0","id":read["id"],"result":payload}));
    let response = session.receive();
    assert_eq!(response["id"], "evaluation-1");
    assert_eq!(response["result"]["original"], original["params"]);
    assert_eq!(response["result"]["acknowledgement"], ack);
    assert_eq!(response["result"]["host"], payload);
    session.send(command("not-supported", "unknown.command", json!([])));
    assert_eq!(session.receive()["error"]["code"], -32602);
    session.send(command("not-allowed", "mcdev.diagnostics", json!([])));
    assert_eq!(session.receive()["error"]["code"], -32602);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    session.close();
}

#[test]
fn published_provider_command_frame_retains_every_original_field() {
    let fixture = include_bytes!("fixtures/provider_commands/provider-commands-v1.frames");
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture)),
        "9b9bc7a8d6dd03c18681c7b02bb638bc3284b2bfb1228cee73dbcf9abf734379"
    );
    let mut frame = Cursor::new(fixture.as_slice());
    let mut message = read_frame(&mut frame).unwrap().unwrap();
    assert!(read_frame(&mut frame).unwrap().is_none());
    assert_eq!(message["method"], "analysis/executeCommand");
    assert_eq!(message["params"]["command"], "example.complete");
    message["params"]["command"] = json!("mcdev.completion");
    let session = Session::start(|params, _, _| Ok(params));
    acknowledge(&session);
    session.send(message.clone());
    let response = session.receive();
    assert_eq!(response["id"], "7");
    assert_eq!(response["result"], message["params"]);
    assert_eq!(response["result"]["arguments"][0]["text"], "😀");
    session.close();
}

#[test]
fn cancellation_retires_late_reply_and_next_command_still_runs() {
    let session = Session::start(|params, _, mut facts| {
        if params["arguments"][0] == "first" {
            facts.read_facts(
                params["evaluation"].as_str().unwrap(),
                &[json!({"op":"sourceInventory"})],
            )?;
        }
        Ok(json!({"arguments":params["arguments"]}))
    });
    acknowledge(&session);
    session.send(command("old", "mcdev.completion", json!(["first"])));
    let read = session.receive();
    session.send(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"old"}}));
    assert_eq!(session.receive()["error"]["code"], -32800);
    session.send(json!({"jsonrpc":"2.0","id":read["id"],"result":{"facts":[]}}));
    session.send(command("fresh", "mcdev.completion", json!(["second"])));
    let result = session.receive();
    assert_eq!(result["id"], "fresh");
    assert_eq!(result["result"], json!({"arguments":["second"]}));
    session.close();
}

#[test]
fn shutdown_interrupts_an_outstanding_command_fact_read() {
    let session = Session::start(|params, _, mut facts| {
        facts.read_facts(
            params["evaluation"].as_str().unwrap(),
            &[json!({"op":"sourceInventory"})],
        )?;
        Ok(Value::Null)
    });
    acknowledge(&session);
    session.send(command("old", "mcdev.completion", json!([])));
    assert_eq!(session.receive()["method"], "analysis/readFacts");
    session.send(json!({"jsonrpc":"2.0","id":"shutdown","method":"shutdown"}));
    assert_eq!(
        session.receive(),
        json!({"jsonrpc":"2.0","id":"shutdown","result":null})
    );
    assert_eq!(session.receive()["error"]["code"], -32800);
    session.close();
}

#[test]
fn eof_interrupts_an_outstanding_command_fact_read() {
    let (report, report_rx) = mpsc::channel();
    let session = Session::start(move |params, _, mut facts| {
        let result = facts.read_facts(
            params["evaluation"].as_str().unwrap(),
            &[json!({"op":"sourceInventory"})],
        );
        report.send(result).unwrap();
        Ok(Value::Null)
    });
    acknowledge(&session);
    session.send(command("old", "mcdev.completion", json!([])));
    assert_eq!(session.receive()["method"], "analysis/readFacts");
    drop(session.input);
    assert!(session.task.join().unwrap().is_ok());
    assert!(matches!(
        report_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        Err(FactsError::Cancelled(_))
    ));
}

#[test]
fn stale_host_read_closes_lease_before_any_retry() {
    let (report, report_rx) = mpsc::channel();
    let session = Session::start(move |params, _, facts| {
        let query = json!({"evaluation":params["evaluation"],"queries":[{"op":"sourceInventory"}]});
        let first = facts.request("analysis/readFacts", query.clone());
        let second = facts.request("analysis/readFacts", query);
        report.send((first, second)).unwrap();
        Ok(Value::Null)
    });
    acknowledge(&session);
    session.send(command("stale", "mcdev.completion", json!([])));
    let read = session.receive();
    session
        .send(json!({"jsonrpc":"2.0","id":read["id"],"error":{"code":-32801,"message":"stale"}}));
    assert_eq!(session.receive()["error"]["code"], -32801);
    let (first, second) = report_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(first, Err(FactsError::Stale(_))));
    assert!(matches!(second, Err(FactsError::Cancelled(_))));
    session.close();
}

#[test]
fn invalid_host_token_closes_evaluation_without_retry() {
    let (report, report_rx) = mpsc::channel();
    let session = Session::start(move |params, _, facts| {
        let query = json!({"evaluation":params["evaluation"],"queries":[
            {"op":"jvmClass","entry":"wrong-token-kind"}]});
        let first = facts.request("analysis/readFacts", query.clone());
        let second = facts.request("analysis/readFacts", query);
        report.send((first, second)).unwrap();
        Ok(Value::Null)
    });
    acknowledge(&session);
    session.send(command("invalid", "mcdev.completion", json!([])));
    let read = session.receive();
    session.send(json!({"jsonrpc":"2.0","id":read["id"],"error":{
        "code":-32602,"message":"invalid token"}}));
    assert_eq!(session.receive()["error"]["code"], -32602);
    let (first, second) = report_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(first, Err(FactsError::Protocol(_))));
    assert!(matches!(second, Err(FactsError::Cancelled(_))));
    session.close();
}

#[test]
fn response_budget_split_is_recoverable_inside_same_evaluation() {
    let session = Session::start(|params, _, facts| {
        let query = json!({"evaluation":params["evaluation"],"queries":[{"op":"sourceInventory"}]});
        let first = facts.request("analysis/readFacts", query.clone());
        assert!(
            matches!(first, Err(FactsError::Protocol(ref message)) if message.contains("split queries"))
        );
        facts.request("analysis/readFacts", query)
    });
    acknowledge(&session);
    session.send(command("split", "mcdev.completion", json!([])));
    let first = session.receive();
    session.send(json!({"jsonrpc":"2.0","id":first["id"],"error":{
        "code":-32602,"message":"extension response batch too large; split queries"}}));
    let second = session.receive();
    assert_eq!(second["params"]["evaluation"], "split");
    session.send(json!({"jsonrpc":"2.0","id":second["id"],"result":{"facts":[]}}));
    assert_eq!(session.receive()["result"], json!({"facts":[]}));
    session.close();
}

#[test]
fn stream_continuations_do_not_consume_metadata_batch_budget() {
    let session = Session::start(|params, _, facts| {
        let evaluation = params["evaluation"].clone();
        for _ in 0..64 {
            facts.request(
                "analysis/readFacts",
                json!({"evaluation":evaluation,"queries":[{"op":"sourceInventory"}]}),
            )?;
        }
        for _ in 0..257 {
            facts.request(
                "analysis/readFacts",
                json!({"evaluation":evaluation,"queries":[{"op":"jvmCatalog","after":"cursor"}]}),
            )?;
        }
        Ok(Value::Null)
    });
    acknowledge(&session);
    session.send(command("quota", "mcdev.completion", json!([])));
    for _ in 0..(64 + 256) {
        let request = session.receive();
        assert_eq!(request["method"], "analysis/readFacts");
        session.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"facts":[]}}));
    }
    let response = session.receive();
    assert_eq!(response["error"]["code"], -32602);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("continuation limit"));
    session.close();
}

#[test]
fn cancelled_workers_that_do_not_cooperate_are_bounded() {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_gate = gate.clone();
    let (started, started_rx) = mpsc::channel();
    let session = Session::start(move |_, _, _| {
        started.send(()).unwrap();
        let (lock, wake) = &*worker_gate;
        let mut released = lock.lock().unwrap();
        while !*released {
            released = wake.wait(released).unwrap();
        }
        Ok(Value::Null)
    });
    acknowledge(&session);
    for index in 0..8 {
        let id = format!("cancel-{index}");
        session.send(command(&id, "mcdev.completion", json!([])));
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        session.send(json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":id}}));
        assert_eq!(session.receive()["error"]["code"], -32800);
    }
    session.send(command("ninth", "mcdev.completion", json!([])));
    let denied = session.receive();
    assert_eq!(denied["error"]["code"], -32002);
    assert!(denied["error"]["message"]
        .as_str()
        .unwrap()
        .contains("still draining"));
    let (lock, wake) = &*gate;
    *lock.lock().unwrap() = true;
    wake.notify_all();
    session.close();
}
