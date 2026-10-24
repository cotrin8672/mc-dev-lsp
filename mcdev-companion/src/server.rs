use crate::{
    policy::{self, Host},
    transport::{read_frame, write_frame},
    wire::Evaluation,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    io::{self, BufRead, Write},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

const RPC_LIMIT: Duration = Duration::from_secs(25);
const MAX_BATCHES: usize = 64;

#[derive(Debug)]
pub(crate) enum Failure {
    Cancelled,
    Shutdown,
    Exit,
    Closed,
    Protocol(String),
}
impl From<String> for Failure {
    fn from(value: String) -> Self {
        Self::Protocol(value)
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Protocol(s) => f.write_str(s),
            other => write!(f, "{other:?}"),
        }
    }
}

type Incoming = Result<Option<Value>, io::Error>;
struct Server<W> {
    input: Receiver<Incoming>,
    output: W,
    next: u64,
    initialized: bool,
    shutdown: bool,
    active: Option<Value>,
    deadline: Option<Instant>,
    batches: usize,
    retired: VecDeque<Value>,
}

/// The reader owns stdin; the bounded channel keeps cancellation observable during host reads.
/// No workspace, parser, Java process or second LSP connection exists in this process.
pub fn run<R: BufRead + Send + 'static, W: Write>(mut input: R, output: W) -> io::Result<()> {
    let (tx, rx) = mpsc::sync_channel(32);
    std::thread::spawn(move || loop {
        let result = read_frame(&mut input);
        let end = !matches!(result, Ok(Some(_)));
        if tx.send(result).is_err() || end {
            break;
        }
    });
    let mut server = Server {
        input: rx,
        output,
        next: 0,
        initialized: false,
        shutdown: false,
        active: None,
        deadline: None,
        batches: 0,
        retired: VecDeque::new(),
    };
    server
        .serve()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}
impl<W: Write> Server<W> {
    fn send(&mut self, message: Value) -> Result<(), Failure> {
        write_frame(&mut self.output, &message).map_err(|e| Failure::Protocol(e.to_string()))
    }
    fn result(&mut self, id: &Value, result: Value) -> Result<(), Failure> {
        self.send(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
    fn error(&mut self, id: &Value, code: i64, message: &str) -> Result<(), Failure> {
        self.send(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}))
    }
    fn receive(&self) -> Result<Value, Failure> {
        let incoming = if let Some(deadline) = self.deadline {
            self.input
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|e| match e {
                    mpsc::RecvTimeoutError::Timeout => {
                        Failure::Protocol("evaluation deadline exceeded".into())
                    }
                    mpsc::RecvTimeoutError::Disconnected => Failure::Closed,
                })?
        } else {
            self.input.recv().map_err(|_| Failure::Closed)?
        };
        let message = incoming
            .map_err(|e| Failure::Protocol(e.to_string()))?
            .ok_or(Failure::Closed)?;
        if !message.is_object() || message["jsonrpc"] != "2.0" {
            return Err(Failure::Protocol("invalid JSON-RPC 2.0 message".into()));
        }
        if let Some(id) = message.get("id") {
            if !id.is_string() && !id.is_i64() && !id.is_u64() {
                return Err(Failure::Protocol("invalid request id".into()));
            }
        }
        Ok(message)
    }
    fn retire(&mut self, id: Value) {
        self.retired.push_back(id);
        if self.retired.len() > 64 {
            self.retired.pop_front();
        }
    }
    fn initialize(&mut self, id: &Value, params: &Value) -> Result<(), Failure> {
        let caps = params["capabilities"].as_array();
        let provider = params["provider"].as_str();
        let required = params.get("requiredCapabilities").and_then(Value::as_array);
        if self.initialized
            || params["protocol"]["major"] != 1
            || params["protocol"]["minor"].as_u64().is_none()
            || provider.is_none_or(|p| p.is_empty() || p.len() > 256)
            || !caps.is_some_and(|c| c.iter().any(|v| v == "semantic-v1"))
            || caps.is_some_and(|c| c.iter().any(|v| !v.is_string()))
            || params
                .get("requiredCapabilities")
                .is_some_and(|v| !v.is_array())
            || required.is_some_and(|c| c.iter().any(|v| v != "semantic-v1"))
        {
            self.error(
                id,
                -32602,
                "incompatible semantic extension protocol or capabilities",
            )?;
            return Err(Failure::Protocol("initialization rejected".into()));
        }
        self.initialized = true;
        self.result(
            id,
            json!({"protocol":{"major":1,"minor":0},"capabilities":["semantic-v1"]}),
        )
    }
    fn serve(&mut self) -> Result<(), Failure> {
        loop {
            let message = match self.receive() {
                Ok(m) => m,
                Err(Failure::Closed) => return Ok(()),
                Err(e) => return Err(e),
            };
            let id = message.get("id").cloned();
            match message["method"].as_str() {
                Some("initialize") => {
                    let id =
                        id.ok_or_else(|| Failure::Protocol("initialize requires id".into()))?;
                    self.initialize(&id, &message["params"])?;
                }
                Some("analysis/evaluate") => {
                    let id = id.ok_or_else(|| Failure::Protocol("evaluate requires id".into()))?;
                    if !self.initialized || self.shutdown {
                        self.error(&id, -32002, "companion is not initialized or has shut down")?;
                        continue;
                    }
                    let e: Evaluation = match serde_json::from_value(message["params"].clone()) {
                        Ok(e) => e,
                        Err(_) => {
                            self.error(&id, -32602, "invalid evaluation source facts")?;
                            continue;
                        }
                    };
                    if id != e.evaluation {
                        self.error(&id, -32602, "evaluation token and request id differ")?;
                        continue;
                    }
                    self.active = Some(id.clone());
                    self.deadline = Some(Instant::now() + RPC_LIMIT);
                    self.batches = 0;
                    let result = policy::analyze(&e, self);
                    self.active = None;
                    self.deadline = None;
                    match result {
                        Ok(value) => self.result(
                            &id,
                            serde_json::to_value(value)
                                .map_err(|e| Failure::Protocol(e.to_string()))?,
                        )?,
                        Err(Failure::Cancelled) | Err(Failure::Shutdown) => {
                            self.result(&id, json!({"sites":[],"diagnostics":[]}))?
                        }
                        Err(Failure::Closed) | Err(Failure::Exit) => return Ok(()),
                        Err(error) => self.error(&id, -32602, &error.to_string())?,
                    }
                }
                Some("shutdown") => {
                    self.shutdown = true;
                    if let Some(id) = id {
                        self.result(&id, Value::Null)?;
                    }
                }
                Some("exit") => return Ok(()),
                Some("$/cancelRequest") => (),
                Some(_) => {
                    if let Some(id) = id {
                        self.error(&id, -32601, "unsupported method")?;
                    }
                }
                None if id.as_ref().is_some_and(|id| self.retired.contains(id)) => (),
                None => return Err(Failure::Protocol("unexpected host response id".into())),
            }
        }
    }
}
impl<W: Write> Host for Server<W> {
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        self.batches += 1;
        if self.batches > MAX_BATCHES {
            return Err(Failure::Protocol("evaluation batch limit".into()));
        }
        let active = self
            .active
            .clone()
            .ok_or_else(|| Failure::Protocol("host read outside evaluation".into()))?;
        if params["evaluation"] != active {
            return Err(Failure::Protocol("foreign evaluation request".into()));
        }
        let id = loop {
            self.next = self
                .next
                .checked_add(1)
                .ok_or_else(|| Failure::Protocol("request sequence exhausted".into()))?;
            let id = json!(format!("mcdev-rpc:{}", self.next));
            if id != active {
                break id;
            }
        };
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            let message = self.receive()?;
            match message["method"].as_str() {
                Some("$/cancelRequest") if message["params"]["id"] == active => {
                    self.retire(id);
                    return Err(Failure::Cancelled);
                }
                Some("$/cancelRequest") => continue,
                Some("exit") => return Err(Failure::Exit),
                Some("shutdown") => {
                    self.shutdown = true;
                    if let Some(id) = message.get("id") {
                        self.result(id, Value::Null)?;
                    }
                    self.retire(id);
                    return Err(Failure::Shutdown);
                }
                Some(_) => {
                    if let Some(id) = message.get("id") {
                        self.error(id, -32001, "evaluation already running")?;
                    }
                    continue;
                }
                None if message["id"] == id => {
                    if message.get("result").is_some() == message.get("error").is_some() {
                        return Err(Failure::Protocol("malformed host response".into()));
                    }
                    if let Some(error) = message.get("error") {
                        if error["message"] == "Stale" {
                            return Err(Failure::Cancelled);
                        }
                        return Err(Failure::Protocol(format!("host rejected batch: {error}")));
                    }
                    return Ok(message["result"].clone());
                }
                None if self.retired.contains(&message["id"]) => continue,
                None => return Err(Failure::Protocol("foreign host response id".into())),
            }
        }
    }
}

// The two-op adapter borrows this same actor. Optional negotiation is still
// disabled in initialize until the host publishes its implemented checkpoint.
impl<W: Write> crate::host_facts::bridge::FactReader for Server<W> {
    fn read_facts(
        &mut self,
        evaluation: &str,
        queries: &[Value],
    ) -> Result<Value, crate::host_facts::FactsError> {
        use crate::host_facts::FactsError;
        if queries.is_empty() || queries.len() > crate::wire::MAX_BATCH {
            return Err(FactsError::Protocol(
                "optional fact query batch limit".into(),
            ));
        }
        self.request(
            "analysis/readFacts",
            json!({"evaluation":evaluation,"queries":queries}),
        )
        .map_err(|error| match error {
            Failure::Protocol(message) => FactsError::Protocol(message),
            Failure::Cancelled | Failure::Shutdown | Failure::Exit | Failure::Closed => {
                FactsError::Cancelled(error.to_string())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_scoped_reader_uses_existing_cancel_and_late_reply_router() {
        use crate::{
            host_facts::{
                bridge::ScopedFacts,
                draft::{SupportedOps, CAPABILITY},
                FactsError,
            },
            wire::Registry,
        };
        let (sender, receiver) = mpsc::sync_channel(8);
        let mut server = Server {
            input: receiver,
            output: Vec::new(),
            next: 0,
            initialized: true,
            shutdown: false,
            active: Some(json!("e1")),
            deadline: Some(Instant::now() + RPC_LIMIT),
            batches: 0,
            retired: VecDeque::new(),
        };
        let ops=SupportedOps::intersect(&json!({"capabilities":["semantic-v1",CAPABILITY],"supportedOps":{CAPABILITY:["projectContext","typeJvm"]}})).unwrap();
        let mut registry = Registry::default();
        sender
            .send(Ok(Some(
                json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"e1"}}),
            )))
            .unwrap();
        assert!(matches!(
            ScopedFacts::new("e1", &ops, &mut registry, &mut server)
                .unwrap()
                .project_context(),
            Err(FactsError::Cancelled(_))
        ));
        assert!(server.retired.contains(&json!("mcdev-rpc:1")));
        server.active = Some(json!("e2"));
        server.batches = 0;
        sender
            .send(Ok(Some(
                json!({"jsonrpc":"2.0","id":"mcdev-rpc:1","result":{"facts":[]}}),
            )))
            .unwrap();
        let unknown = json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]});
        sender.send(Ok(Some(json!({"jsonrpc":"2.0","id":"mcdev-rpc:2","result":{"facts":[{"workspaceUri":{"state":"found","value":"file:///workspace"},"scope":unknown,"sourceRoots":unknown,"outputRoots":unknown,"classpath":unknown,"sdk":unknown}]}})))).unwrap();
        let project = ScopedFacts::new("e2", &ops, &mut registry, &mut server)
            .unwrap()
            .project_context()
            .unwrap();
        assert_eq!(
            project.workspace_uri.found().map(String::as_str),
            Some("file:///workspace")
        );
        let mut output = std::io::Cursor::new(&server.output);
        let first = read_frame(&mut output).unwrap().unwrap();
        let second = read_frame(&mut output).unwrap().unwrap();
        assert_eq!(first["params"]["evaluation"], "e1");
        assert_eq!(second["params"]["evaluation"], "e2");
        assert_eq!(
            second["params"]["queries"],
            json!([{"op":"projectContext"}])
        );
    }
    #[test]
    fn expired_evaluation_and_excess_batches_fail_without_sending_a_partial_request() {
        let (_sender, receiver) = mpsc::sync_channel(1);
        let mut server = Server {
            input: receiver,
            output: Vec::new(),
            next: 0,
            initialized: true,
            shutdown: false,
            active: Some(json!("evaluation")),
            deadline: Some(Instant::now()),
            batches: MAX_BATCHES,
            retired: VecDeque::new(),
        };
        assert!(server
            .receive()
            .unwrap_err()
            .to_string()
            .contains("deadline"));
        assert!(server
            .request(
                "analysis/readFacts",
                json!({"evaluation":"evaluation","queries":[]})
            )
            .unwrap_err()
            .to_string()
            .contains("batch limit"));
        assert!(server.output.is_empty());
    }
}
