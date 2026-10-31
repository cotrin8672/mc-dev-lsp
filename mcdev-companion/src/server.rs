use crate::{
    host_facts::FactsError,
    policy::{self, Host},
    provider_commands::{CommandEvent, CommandFacts, CommandHandler},
    transport::{read_frame_with_length, write_frame},
    wire::Evaluation,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, VecDeque},
    io::{self, BufRead, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};

const RPC_LIMIT: Duration = Duration::from_secs(25);
const MAX_BATCHES: usize = 64;
const MAX_CONTINUATIONS: usize = 256;
const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;
const MAX_COMMAND_RESULT: usize = 512 * 1024;
const MAX_COMMAND_WORKERS: usize = 8;

#[derive(Debug)]
pub(crate) enum Failure {
    Cancelled,
    Shutdown,
    Exit,
    Closed,
    Host { code: i64, message: String },
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
            Self::Host { code, message } => write!(f, "host error {code}: {message}"),
            other => write!(f, "{other:?}"),
        }
    }
}

type Incoming = Result<Option<(Value, usize)>, io::Error>;
struct Server<W> {
    input: Receiver<Incoming>,
    output: W,
    next: u64,
    initialized: bool,
    shutdown: bool,
    active: Option<Value>,
    deadline: Option<Instant>,
    batches: usize,
    continuations: usize,
    stream_bytes: usize,
    retired: VecDeque<Value>,
    initialization: Value,
    commands: BTreeSet<String>,
    command_handler: Arc<CommandHandler>,
    command_workers: Arc<AtomicUsize>,
}

struct CommandWorker(Arc<AtomicUsize>);
impl Drop for CommandWorker {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// The reader owns stdin; the bounded channel keeps cancellation observable during host reads.
/// No workspace, parser, Java process or second LSP connection exists in this process.
pub fn run<R: BufRead + Send + 'static, W: Write>(input: R, output: W) -> io::Result<()> {
    run_with_command_handler(input, output, crate::commands::ipc::execute)
}

fn negotiate(params: &Value) -> Result<(Value, BTreeSet<String>), String> {
    let caps = params["capabilities"]
        .as_array()
        .ok_or("missing capabilities")?;
    if params["protocol"]["major"] != 1
        || params["protocol"]["minor"].as_u64().is_none()
        || params["provider"]
            .as_str()
            .is_none_or(|v| v.is_empty() || v.len() > 256)
        || caps.iter().any(|v| !v.is_string())
        || !caps.iter().any(|v| v == "semantic-v1")
    {
        return Err("incompatible semantic extension protocol or capabilities".into());
    }
    let offered = |name: &str| caps.iter().any(|v| v == name);
    let mut accepted = vec!["semantic-v1".to_string()];
    let mut operations = serde_json::Map::new();
    if params.get("supportedOps").is_some_and(|v| !v.is_object()) {
        return Err("supportedOps must be an object".into());
    }
    for (capability, known) in [
        ("project-class-facts-v1", &["projectContext", "typeJvm"][..]),
        (
            "source-snapshots-v1",
            &["sourceInventory", "sourceSnapshot"][..],
        ),
        (
            "owned-resources-v1",
            &["resourceInventory", "resourceLookup", "resourceSnapshot"][..],
        ),
        (
            "runtime-class-bytes-v1",
            &["runtimeClass", "classBytes"][..],
        ),
        ("jvm-catalog-v1", &["jvmCatalog", "jvmClass"][..]),
        ("source-facts-v2", &["sourceFacts"][..]),
        ("source-locations-v1", &["sourceLocation"][..]),
        ("project-inputs-v1", &["projectInputs"][..]),
        (
            "root-files-v1",
            &["rootFileInventory", "rootFileSnapshot"][..],
        ),
    ] {
        if !offered(capability) {
            continue;
        }
        let Some(value) = params["supportedOps"].get(capability) else {
            continue;
        };
        let items = value.as_array().ok_or("operation set must be an array")?;
        if items.iter().any(|v| !v.is_string()) {
            return Err("operation names must be strings".into());
        }
        let intersection: Vec<Value> = known
            .iter()
            .filter(|name| items.iter().any(|v| v == **name))
            .map(|name| json!(name))
            .collect();
        if !intersection.is_empty() {
            accepted.push(capability.into());
            operations.insert(capability.into(), Value::Array(intersection));
        }
    }
    if offered("binary-annotations-v1") {
        accepted.push("binary-annotations-v1".into());
    }
    let mut commands = BTreeSet::new();
    if offered("provider-commands-v1") {
        let configured = params
            .get("commands")
            .map(|v| v.as_array().ok_or("commands must be an array"))
            .transpose()?;
        if let Some(configured) = configured {
            if configured.len() > 64 {
                return Err("too many command ids".into());
            }
            for value in configured {
                let name = value
                    .as_str()
                    .filter(|name| !name.is_empty() && name.len() <= 128 && name.is_ascii())
                    .ok_or("invalid command id")?;
                if mcdev_protocol::Command::parse(name).is_some()
                    && crate::commands::ipc::supports_command(name)
                {
                    commands.insert(name.to_string());
                }
            }
        }
        accepted.push("provider-commands-v1".into());
    }
    if let Some(required) = params.get("requiredCapabilities") {
        let required = required
            .as_array()
            .ok_or("requiredCapabilities must be an array")?;
        if required.iter().any(|v| {
            !v.as_str()
                .is_some_and(|name| accepted.iter().any(|cap| cap == name))
        }) {
            return Err("missing required capability".into());
        }
    }
    let mut result = json!({"protocol":{"major":1,"minor":0},"capabilities":accepted});
    if !operations.is_empty() {
        result["supportedOps"] = Value::Object(operations);
    }
    if offered("provider-commands-v1") {
        result["commands"] = json!(commands);
    }
    Ok((result, commands))
}

pub fn run_with_command_handler<R, W, H>(mut input: R, output: W, handler: H) -> io::Result<()>
where
    R: BufRead + Send + 'static,
    W: Write,
    H: Fn(Value, Value, CommandFacts) -> Result<Value, FactsError> + Send + Sync + 'static,
{
    let (tx, rx) = mpsc::sync_channel(32);
    std::thread::spawn(move || loop {
        let result = read_frame_with_length(&mut input);
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
        continuations: 0,
        stream_bytes: 0,
        retired: VecDeque::new(),
        initialization: Value::Null,
        commands: BTreeSet::new(),
        command_handler: Arc::new(handler),
        command_workers: Arc::new(AtomicUsize::new(0)),
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
    fn receive(&self) -> Result<(Value, usize), Failure> {
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
        Self::decode_incoming(incoming)
    }
    fn decode_incoming(incoming: Incoming) -> Result<(Value, usize), Failure> {
        let (message, length) = incoming
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
        Ok((message, length))
    }
    fn retire(&mut self, id: Value) {
        self.retired.push_back(id);
        if self.retired.len() > 64 {
            self.retired.pop_front();
        }
    }
    fn initialize(&mut self, id: &Value, params: &Value) -> Result<(), Failure> {
        if self.initialized {
            self.error(id, -32602, "already initialized")?;
            return Err(Failure::Protocol("initialization rejected".into()));
        }
        let (result, commands) = match negotiate(params) {
            Ok(result) => result,
            Err(message) => {
                self.error(id, -32602, &message)?;
                return Err(Failure::Protocol("initialization rejected".into()));
            }
        };
        self.initialized = true;
        self.initialization = result.clone();
        self.commands = commands;
        self.result(id, result)
    }
    fn serve(&mut self) -> Result<(), Failure> {
        loop {
            let message = match self.receive() {
                Ok((m, _)) => m,
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
                    self.continuations = 0;
                    self.stream_bytes = 0;
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
                        Err(Failure::Host {
                            code: -32800 | -32801,
                            ..
                        }) => self.result(&id, json!({"sites":[],"diagnostics":[]}))?,
                        Err(Failure::Host { message, .. }) if message == "Stale" => {
                            self.result(&id, json!({"sites":[],"diagnostics":[]}))?
                        }
                        Err(Failure::Closed) | Err(Failure::Exit) => return Ok(()),
                        Err(error) => self.error(&id, -32602, &error.to_string())?,
                    }
                }
                Some("analysis/executeCommand") => {
                    let id = id.ok_or_else(|| Failure::Protocol("command requires id".into()))?;
                    match self.execute_command(id, message["params"].clone()) {
                        Ok(()) => {}
                        Err(Failure::Exit | Failure::Closed) => return Ok(()),
                        Err(error) => return Err(error),
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

    fn execute_command(&mut self, id: Value, params: Value) -> Result<(), Failure> {
        if !self.initialized || self.shutdown {
            return self.error(&id, -32002, "companion is not initialized or has shut down");
        }
        let command = params["command"].as_str();
        let arguments = params["arguments"].as_array();
        let evaluation = serde_json::from_value::<Evaluation>(params.clone());
        if command.is_none_or(|name| !self.commands.contains(name))
            || arguments.is_none_or(|values| values.len() > 256)
            || !params["context"].is_object()
            || evaluation.as_ref().is_err()
            || evaluation
                .as_ref()
                .is_ok_and(|value| id != value.evaluation)
        {
            return self.error(&id, -32602, "invalid or unnegotiated provider command");
        }
        if self.command_workers.fetch_add(1, Ordering::AcqRel) >= MAX_COMMAND_WORKERS {
            self.command_workers.fetch_sub(1, Ordering::AcqRel);
            return self.error(&id, -32002, "too many command workers still draining");
        }

        self.active = Some(id.clone());
        let deadline = Instant::now() + RPC_LIMIT;
        self.deadline = Some(deadline);
        self.batches = 0;
        self.continuations = 0;
        self.stream_bytes = 0;
        let (facts, worker_events) =
            CommandFacts::new(id.clone(), self.initialization.clone(), deadline);
        let worker_facts = facts.clone();
        let handler = self.command_handler.clone();
        let initialization = self.initialization.clone();
        let worker = CommandWorker(self.command_workers.clone());
        let started = std::thread::Builder::new().spawn(move || {
            let _worker = worker;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                handler(params, initialization, worker_facts.clone())
            }))
            .unwrap_or_else(|_| Err(FactsError::Protocol("command worker panicked".into())));
            worker_facts.finished(result);
        });
        if let Err(error) = started {
            facts.close();
            self.active = None;
            self.deadline = None;
            return self.error(
                &id,
                -32002,
                &format!("cannot start command worker: {error}"),
            );
        }

        let outcome = loop {
            if Instant::now() >= deadline {
                break Err(Failure::Cancelled);
            }
            match self.input.try_recv() {
                Ok(incoming) => match self.command_input(&id, incoming) {
                    Ok(Some(error)) | Err(error) => break Err(error),
                    Ok(None) => continue,
                },
                Err(mpsc::TryRecvError::Disconnected) => break Err(Failure::Closed),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            match worker_events.try_recv() {
                Ok(CommandEvent::Finished(result)) => break Ok(result),
                Ok(CommandEvent::Request {
                    method,
                    params,
                    reply,
                }) => {
                    match self.request(&method, params) {
                        Ok(value) => {
                            let _ = reply.send(Ok(value));
                        }
                        Err(error) => {
                            let recoverable = matches!(&error,
                                Failure::Host { code: -32602, message }
                                if message.contains("split queries"));
                            let fact_error = fact_failure(&error);
                            if !recoverable {
                                facts.close();
                            }
                            let _ = reply.send(Err(fact_error));
                            if !recoverable {
                                break Err(error);
                            }
                        }
                    }
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    break Err(Failure::Protocol("command worker closed".into()));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
            let wait = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(10));
            let incoming = match self.input.recv_timeout(wait) {
                Ok(value) => value,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break Err(Failure::Closed),
            };
            match self.command_input(&id, incoming) {
                Ok(Some(error)) | Err(error) => break Err(error),
                Ok(None) => {}
            }
        };
        facts.close();
        self.active = None;
        self.deadline = None;

        match outcome {
            Ok(Ok(value)) => {
                let response = json!({"jsonrpc":"2.0","id":id,"result":value});
                let size = serde_json::to_vec(&response)
                    .map_err(|error| Failure::Protocol(error.to_string()))?
                    .len();
                if size > MAX_COMMAND_RESULT {
                    self.error(&id, -32002, "provider command response exceeds limit")
                } else {
                    self.send(response)
                }
            }
            Ok(Err(error)) => self.command_error(&id, &error),
            Err(Failure::Exit | Failure::Closed) => Err(Failure::Exit),
            Err(error) => self.command_error(&id, &fact_failure(&error)),
        }
    }

    fn command_input(
        &mut self,
        id: &Value,
        incoming: Incoming,
    ) -> Result<Option<Failure>, Failure> {
        let (message, _) = Self::decode_incoming(incoming)?;
        match message["method"].as_str() {
            Some("$/cancelRequest") if message["params"]["id"] == *id => {
                Ok(Some(Failure::Cancelled))
            }
            Some("$/cancelRequest") => Ok(None),
            Some("shutdown") => {
                self.shutdown = true;
                if let Some(shutdown_id) = message.get("id") {
                    self.result(shutdown_id, Value::Null)?;
                }
                Ok(Some(Failure::Shutdown))
            }
            Some("exit") => Ok(Some(Failure::Exit)),
            Some(_) => {
                if let Some(other_id) = message.get("id") {
                    self.error(other_id, -32001, "evaluation already running")?;
                }
                Ok(None)
            }
            None if self.retired.contains(&message["id"]) => Ok(None),
            None => Ok(Some(Failure::Protocol("foreign host response id".into()))),
        }
    }

    fn command_error(&mut self, id: &Value, error: &FactsError) -> Result<(), Failure> {
        let code = match error {
            FactsError::Cancelled(_) => -32800,
            FactsError::Stale(_) => -32801,
            FactsError::Protocol(_) => -32602,
            _ => -32002,
        };
        self.error(id, code, &error.to_string())
    }
}

fn fact_failure(error: &Failure) -> FactsError {
    match error {
        Failure::Cancelled | Failure::Shutdown | Failure::Exit | Failure::Closed => {
            FactsError::Cancelled(error.to_string())
        }
        Failure::Host { message, .. } if message == "Stale" => FactsError::Stale(message.clone()),
        Failure::Protocol(message) if message == "evaluation deadline exceeded" => {
            FactsError::Cancelled(message.clone())
        }
        Failure::Host {
            code: -32800,
            message,
        } => FactsError::Cancelled(message.clone()),
        Failure::Host {
            code: -32801,
            message,
        } => FactsError::Stale(message.clone()),
        Failure::Host {
            code: -32002,
            message,
        } => FactsError::Unknown(message.clone()),
        _ => FactsError::Protocol(error.to_string()),
    }
}
impl<W: Write> Host for Server<W> {
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        let active = self
            .active
            .clone()
            .ok_or_else(|| Failure::Protocol("host read outside evaluation".into()))?;
        if params["evaluation"] != active {
            return Err(Failure::Protocol("foreign evaluation request".into()));
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(Failure::Protocol("evaluation deadline exceeded".into()));
        }
        let queries = if method == "analysis/readFacts" {
            params["queries"].as_array()
        } else {
            None
        };
        let continuation_count = queries.map_or(0, |queries| {
            queries
                .iter()
                .filter(|query| {
                    matches!(
                        query["op"].as_str(),
                        Some(
                            "resourceSnapshot"
                                | "classBytes"
                                | "jvmCatalog"
                                | "rootFileInventory"
                                | "rootFileSnapshot"
                        )
                    ) && query.get("after").is_some_and(|value| !value.is_null())
                })
                .count()
        });
        if continuation_count == 0
            || queries.is_none_or(|queries| continuation_count != queries.len())
        {
            self.batches += 1;
        }
        self.continuations += continuation_count;
        if self.batches > MAX_BATCHES {
            return Err(Failure::Protocol("evaluation batch limit".into()));
        }
        if self.continuations > MAX_CONTINUATIONS {
            return Err(Failure::Protocol("evaluation continuation limit".into()));
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
            let (message, body_length) = self.receive()?;
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
                    if continuation_count != 0 {
                        self.stream_bytes = self.stream_bytes.saturating_add(body_length);
                        if self.stream_bytes > MAX_STREAM_BYTES {
                            return Err(Failure::Protocol(
                                "evaluation stream response limit".into(),
                            ));
                        }
                    }
                    if message.get("result").is_some() == message.get("error").is_some() {
                        return Err(Failure::Protocol("malformed host response".into()));
                    }
                    if let Some(error) = message.get("error") {
                        if error["message"] == "Stale" && error["code"].as_i64().is_none() {
                            return Err(Failure::Cancelled);
                        }
                        let code = error["code"]
                            .as_i64()
                            .ok_or_else(|| Failure::Protocol("malformed host error".into()))?;
                        let message = error["message"]
                            .as_str()
                            .ok_or_else(|| Failure::Protocol("malformed host error".into()))?;
                        return Err(Failure::Host {
                            code,
                            message: message.into(),
                        });
                    }
                    return Ok(message["result"].clone());
                }
                None if self.retired.contains(&message["id"]) => continue,
                None => return Err(Failure::Protocol("foreign host response id".into())),
            }
        }
    }
}

// Semantic evaluations and commands use the same bounded fact request path.
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
        .map_err(|error| fact_failure(&error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::read_frame;

    #[test]
    fn root_file_negotiation_is_optional_and_operation_scoped() {
        let base = json!({"protocol":{"major":1,"minor":0},"provider":"mcdev","capabilities":["semantic-v1"]});
        let (legacy, _) = negotiate(&base).unwrap();
        assert_eq!(legacy["capabilities"], json!(["semantic-v1"]));
        assert!(legacy.get("supportedOps").is_none());
        let mut host = base.clone();
        host["capabilities"] = json!(["semantic-v1", "root-files-v1"]);
        let (unselected, _) = negotiate(&host).unwrap();
        assert_eq!(unselected["capabilities"], json!(["semantic-v1"]));
        host["supportedOps"] = json!({"root-files-v1":["rootFileSnapshot","future"]});
        let (selected, _) = negotiate(&host).unwrap();
        assert_eq!(
            selected["capabilities"],
            json!(["semantic-v1", "root-files-v1"])
        );
        assert_eq!(
            selected["supportedOps"],
            json!({"root-files-v1":["rootFileSnapshot"]})
        );
    }

    #[test]
    fn new_input_operations_intersect_and_binary_annotations_select_by_capability_only() {
        let params = json!({"protocol":{"major":1,"minor":0},"provider":"mcdev",
            "capabilities":["semantic-v1","source-facts-v2","source-locations-v1","project-inputs-v1","binary-annotations-v1"],
            "supportedOps":{"source-facts-v2":["sourceFacts","future"],"source-locations-v1":["sourceLocation"],
                "project-inputs-v1":["projectInputs"]},"requiredCapabilities":["binary-annotations-v1"]});
        let (reply, _) = negotiate(&params).unwrap();
        assert!(reply["capabilities"]
            .as_array()
            .unwrap()
            .contains(&json!("binary-annotations-v1")));
        assert_eq!(
            reply["supportedOps"]["source-facts-v2"],
            json!(["sourceFacts"])
        );
        assert_eq!(
            reply["supportedOps"]["source-locations-v1"],
            json!(["sourceLocation"])
        );
        assert_eq!(
            reply["supportedOps"]["project-inputs-v1"],
            json!(["projectInputs"])
        );
        assert!(reply["supportedOps"].get("binary-annotations-v1").is_none());
        let mut no_operations = params.clone();
        no_operations
            .as_object_mut()
            .unwrap()
            .remove("supportedOps");
        let (reply, _) = negotiate(&no_operations).unwrap();
        assert_eq!(
            reply["capabilities"],
            json!(["semantic-v1", "binary-annotations-v1"])
        );
        assert!(reply.get("supportedOps").is_none());
    }

    fn incoming(value: Value) -> Incoming {
        let length = serde_json::to_vec(&value).unwrap().len();
        Ok(Some((value, length)))
    }

    fn test_server(input: Receiver<Incoming>) -> Server<Vec<u8>> {
        Server {
            input,
            output: Vec::new(),
            next: 0,
            initialized: true,
            shutdown: false,
            active: Some(json!("e1")),
            deadline: Some(Instant::now() + RPC_LIMIT),
            batches: 0,
            continuations: 0,
            stream_bytes: 0,
            retired: VecDeque::new(),
            initialization: Value::Null,
            commands: BTreeSet::new(),
            command_handler: Arc::new(|_, _, _| Ok(Value::Null)),
            command_workers: Arc::new(AtomicUsize::new(0)),
        }
    }
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
        let mut server = test_server(receiver);
        let ops=SupportedOps::intersect(&json!({"capabilities":["semantic-v1",CAPABILITY],"supportedOps":{CAPABILITY:["projectContext","typeJvm"]}})).unwrap();
        let mut registry = Registry::default();
        sender
            .send(incoming(
                json!({"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":"e1"}}),
            ))
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
            .send(incoming(
                json!({"jsonrpc":"2.0","id":"mcdev-rpc:1","result":{"facts":[]}}),
            ))
            .unwrap();
        let unknown = json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]});
        sender.send(incoming(json!({"jsonrpc":"2.0","id":"mcdev-rpc:2","result":{"facts":[{"workspaceUri":{"state":"found","value":"file:///workspace"},"scope":unknown,"sourceRoots":unknown,"outputRoots":unknown,"classpath":unknown,"sdk":unknown}]}}))).unwrap();
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
        let mut server = test_server(receiver);
        server.active = Some(json!("evaluation"));
        server.deadline = Some(Instant::now());
        server.batches = MAX_BATCHES;
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
            .contains("deadline"));
        assert!(server.output.is_empty());
    }

    #[test]
    fn stream_limit_uses_original_json_body_bytes() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut server = test_server(receiver);
        let body = r#"{   "jsonrpc": "2.0", "id": "mcdev-rpc:1", "result": { "facts": [] }   }"#;
        let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let parsed = read_frame_with_length(&mut std::io::Cursor::new(frame)).unwrap();
        let (value, length) = parsed.as_ref().unwrap();
        assert_eq!(*length, body.len());
        assert!(serde_json::to_vec(value).unwrap().len() < *length);
        server.stream_bytes = MAX_STREAM_BYTES - length + 1;
        sender.send(Ok(parsed)).unwrap();
        assert!(server
            .request(
                "analysis/readFacts",
                json!({"evaluation":"e1","queries":[{"op":"jvmCatalog","after":"cursor"}]}),
            )
            .unwrap_err()
            .to_string()
            .contains("stream response limit"));
    }

    #[test]
    fn root_file_continuations_share_limits_without_consuming_metadata_batches() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut server = test_server(receiver);
        server.batches = MAX_BATCHES;
        for operation in ["rootFileInventory", "rootFileSnapshot"] {
            for _ in 0..70 {
                let id = format!("mcdev-rpc:{}", server.next + 1);
                sender
                    .send(incoming(
                        json!({"jsonrpc":"2.0","id":id,"result":{"facts":[]}}),
                    ))
                    .unwrap();
                let query = if operation == "rootFileInventory" {
                    json!({"op":operation,"filter":{},"after":"cursor"})
                } else {
                    json!({"op":operation,"file":"issued","after":"cursor"})
                };
                server
                    .request(
                        "analysis/readFacts",
                        json!({"evaluation":"e1","queries":[query]}),
                    )
                    .unwrap();
                assert_eq!(server.batches, MAX_BATCHES);
            }
        }
        assert_eq!(server.continuations, 140);
        server.continuations = MAX_CONTINUATIONS;
        let length = server.output.len();
        assert!(server
            .request(
                "analysis/readFacts",
                json!({"evaluation":"e1","queries":[
            {"op":"rootFileSnapshot","file":"issued","after":"cursor"}]})
            )
            .unwrap_err()
            .to_string()
            .contains("continuation limit"));
        assert_eq!(server.output.len(), length);

        let (_sender, receiver) = mpsc::sync_channel(1);
        let mut server = test_server(receiver);
        server.batches = MAX_BATCHES;
        assert!(server
            .request(
                "analysis/readFacts",
                json!({"evaluation":"e1","queries":[
            {"op":"rootFileSnapshot","file":"issued","after":null}]})
            )
            .unwrap_err()
            .to_string()
            .contains("batch limit"));
        assert!(server.output.is_empty());
        server.batches = MAX_BATCHES;
        assert!(server.request("analysis/readFacts",json!({"evaluation":"e1","queries":[
            {"op":"rootFileSnapshot","file":"issued","after":"cursor"},{"op":"resolveType","name":"Other"}]}))
            .unwrap_err().to_string().contains("batch limit"));
        assert!(server.output.is_empty());
    }

    #[test]
    fn root_file_stream_limit_counts_original_escaped_response_bytes() {
        for operation in ["rootFileInventory", "rootFileSnapshot"] {
            let (sender, receiver) = mpsc::sync_channel(1);
            let mut server = test_server(receiver);
            let body = r#"{  "jsonrpc":"2.0", "id":"mcdev-rpc:1", "result":{"facts":[{"text":"\u0000"}]} }"#;
            let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
            let parsed = read_frame_with_length(&mut std::io::Cursor::new(frame)).unwrap();
            let (value, length) = parsed.as_ref().unwrap();
            assert!(serde_json::to_vec(value).unwrap().len() < *length);
            server.stream_bytes = MAX_STREAM_BYTES - length + 1;
            sender.send(Ok(parsed)).unwrap();
            assert!(server
                .request(
                    "analysis/readFacts",
                    json!({"evaluation":"e1","queries":[
                {"op":operation,"after":"cursor"}]})
                )
                .unwrap_err()
                .to_string()
                .contains("stream response limit"));
        }
    }

    #[test]
    fn already_queued_cancellation_precedes_a_completed_command() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut server = test_server(receiver);
        server.commands.insert("mcdev.completion".into());
        sender
            .send(incoming(json!({"jsonrpc":"2.0","method":"$/cancelRequest",
            "params":{"id":"e1"}})))
            .unwrap();
        server
            .execute_command(
                json!("e1"),
                json!({
                    "evaluation":"e1","owner":"/workspace/A.java","source":"class A",
                    "configuration":{},"declarations":[],"context":{},
                    "command":"mcdev.completion","arguments":[]
                }),
            )
            .unwrap();
        let result = read_frame(&mut std::io::Cursor::new(&server.output))
            .unwrap()
            .unwrap();
        assert_eq!(result["error"]["code"], -32800);
        assert!(result.get("result").is_none());
    }
}
