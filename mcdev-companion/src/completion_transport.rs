//! Explicit authenticated loopback command transport. It owns no language/project model.
use crate::commands::CommandFailure;
use mcdev_core::expression_engine::{Cancellation, CancellationChecker};
use mcdev_protocol::{McdevError, McdevErrorCode, McdevResponseEnvelope};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{self, BufReader, Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

pub type CompletionExecutor =
    dyn Fn(Value, &dyn CancellationChecker) -> Result<Value, CommandFailure> + Send + Sync;
pub type EndpointNotifier = dyn Fn(&Endpoint) -> Result<bool, String> + Send + Sync;
pub type FailureObserver = dyn Fn(String) + Send + Sync;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub token: String,
    pub timeout_ms: u64,
    pub protocol_version: i32,
}
pub struct CompletionTransportOptions {
    pub request_timeout: Duration,
    pub max_body_bytes: usize,
    pub notify_endpoint: Arc<EndpointNotifier>,
    pub on_failure: Arc<FailureObserver>,
}
impl Default for CompletionTransportOptions {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_millis(5000),
            max_body_bytes: 4 * 1024 * 1024,
            notify_endpoint: Arc::new(|_| Ok(false)),
            on_failure: Arc::new(|_| {}),
        }
    }
}
struct Active {
    socket: TcpStream,
    cancelled: Arc<AtomicBool>,
}
struct Inner {
    execute: Arc<CompletionExecutor>,
    options: CompletionTransportOptions,
    closed: AtomicBool,
    endpoint: Mutex<Option<Endpoint>>,
    listener: Mutex<Option<TcpListener>>,
    active: Mutex<BTreeMap<u64, Active>>,
    next: AtomicU64,
}
pub struct CompletionTransport {
    inner: Arc<Inner>,
}
impl CompletionTransport {
    pub fn new(
        execute: Arc<CompletionExecutor>,
        options: CompletionTransportOptions,
    ) -> io::Result<Self> {
        if options.request_timeout.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "requestTimeoutMillis must be positive",
            ));
        }
        if options.max_body_bytes == 0 || options.max_body_bytes > i32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "maxBodyBytes must be positive",
            ));
        }
        Ok(Self {
            inner: Arc::new(Inner {
                execute,
                options,
                closed: AtomicBool::new(false),
                endpoint: Mutex::new(None),
                listener: Mutex::new(None),
                active: Mutex::new(BTreeMap::new()),
                next: AtomicU64::new(0),
            }),
        })
    }
    pub fn start(&self) -> io::Result<Endpoint> {
        let mut endpoint = self.inner.endpoint.lock().map_err(poison)?;
        if let Some(endpoint) = endpoint.as_ref() {
            return Ok(endpoint.clone());
        }
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "transport is closed",
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let started = Endpoint {
            host: "127.0.0.1".into(),
            port: listener.local_addr()?.port(),
            token: random_token()?,
            timeout_ms: self
                .inner
                .options
                .request_timeout
                .as_millis()
                .min(u64::MAX as u128) as u64,
            protocol_version: 1,
        };
        let accept_listener = listener.try_clone()?;
        *self.inner.listener.lock().map_err(poison)? = Some(listener);
        *endpoint = Some(started.clone());
        let inner = self.inner.clone();
        thread::spawn(move || accept_loop(inner, accept_listener));
        let inner = self.inner.clone();
        let notify = started.clone();
        thread::spawn(move || {
            let started = Instant::now();
            while !inner.closed.load(Ordering::Acquire)
                && started.elapsed() <= Duration::from_secs(30)
            {
                match (inner.options.notify_endpoint)(&notify) {
                    Ok(true) => break,
                    Ok(false) => (),
                    Err(error) => (inner.options.on_failure)(error),
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        Ok(started)
    }
    pub fn endpoint(&self) -> Option<Endpoint> {
        self.inner.endpoint.lock().ok().and_then(|e| e.clone())
    }
    pub fn close(&self) {
        if self.inner.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.inner
            .listener
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        for active in self
            .inner
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            active.cancelled.store(true, Ordering::Release);
            let _ = active.socket.shutdown(Shutdown::Both);
        }
    }
}
impl Drop for CompletionTransport {
    fn drop(&mut self) {
        self.close();
    }
}
fn poison<T>(error: std::sync::PoisonError<T>) -> io::Error {
    io::Error::other(error.to_string())
}
fn accept_loop(inner: Arc<Inner>, listener: TcpListener) {
    while !inner.closed.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((socket, _)) => {
                if let Err(error) = socket.set_nonblocking(false) {
                    (inner.options.on_failure)(error.to_string());
                    continue;
                }
                let id = inner.next.fetch_add(1, Ordering::Relaxed);
                let cancelled = Arc::new(AtomicBool::new(false));
                let clone = match socket.try_clone() {
                    Ok(s) => s,
                    Err(e) => {
                        (inner.options.on_failure)(e.to_string());
                        continue;
                    }
                };
                inner
                    .active
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(
                        id,
                        Active {
                            socket: clone,
                            cancelled: cancelled.clone(),
                        },
                    );
                let inner = inner.clone();
                thread::spawn(move || handle_connection(inner, id, socket, cancelled));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5))
            }
            Err(error) => {
                if !inner.closed.load(Ordering::Acquire) {
                    (inner.options.on_failure)(error.to_string());
                }
                break;
            }
        }
    }
}
enum RequestError {
    Invalid(String),
    Io(io::Error),
}
impl From<io::Error> for RequestError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
fn invalid(message: &str) -> RequestError {
    RequestError::Invalid(message.into())
}
fn read_request(
    socket: &TcpStream,
    token: &str,
    max_body: usize,
) -> Result<(Value, BufReader<TcpStream>), RequestError> {
    let mut input = BufReader::new(socket.try_clone()?);
    let mut header = Vec::new();
    loop {
        let mut byte = [0];
        if input.read(&mut byte)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete mcdev completion request",
            )
            .into());
        }
        if header.len() >= 16 * 1024 {
            return Err(invalid("mcdev completion request headers are too large"));
        }
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = header[..header.len() - 4]
        .iter()
        .map(|b| char::from(*b))
        .collect::<String>();
    let mut lines = header.split("\r\n");
    let request = lines.next().unwrap_or("").split(' ').collect::<Vec<_>>();
    if request.len() != 3 || request[0] != "POST" || request[1] != "/completion" {
        return Err(invalid(
            "mcdev completion transport accepts POST /completion",
        ));
    }
    let headers = lines
        .filter_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| !name.is_empty())
                .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    if headers.get("authorization") != Some(&format!("Bearer {token}")) {
        return Err(invalid("mcdev completion transport authentication failed"));
    }
    let length = headers
        .get("content-length")
        .and_then(|n| n.parse::<i32>().ok())
        .ok_or_else(|| invalid("content-length is required"))?;
    if length < 1 || length as usize > max_body {
        return Err(invalid("mcdev completion request body is too large"));
    }
    let mut body = vec![0; length as usize];
    input.read_exact(&mut body).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete mcdev completion request body",
            )
        } else {
            e
        }
    })?;
    let payload = serde_json::from_slice::<Value>(&body)
        .map_err(|_| invalid("mcdev completion request is not valid JSON"))?;
    if !payload.is_object() {
        return Err(invalid("mcdev completion request must be a JSON object"));
    }
    Ok((payload, input))
}
fn envelope(message: &str) -> Value {
    serde_json::to_value(McdevResponseEnvelope::<Value>::failure(McdevError {
        code: McdevErrorCode::InternalError,
        message: message.into(),
        details: Default::default(),
    }))
    .unwrap()
}
fn write_response(socket: &mut TcpStream, status: u16, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value)?;
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        499 => "Client Closed Request",
        500 => "Internal Server Error",
        504 => "Gateway Timeout",
        _ => "Error",
    };
    write!(socket,"HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len())?;
    socket.write_all(&body)?;
    socket.flush()
}
fn reject_request(socket: &mut TcpStream, message: &str) -> io::Result<()> {
    write_response(socket, 400, &envelope(message))?;
    // Deliver the response FIN before discarding an unauthenticated request body.
    // Closing with unread TCP bytes can reset the connection on Windows.
    socket.shutdown(Shutdown::Write)?;
    socket.set_read_timeout(Some(Duration::from_millis(50)))?;
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut bytes = [0; 1024];
    let mut remaining = 64 * 1024;
    while remaining > 0 && Instant::now() < deadline {
        match socket.read(&mut bytes[..remaining.min(1024)]) {
            Ok(0) | Err(_) => break,
            Ok(count) => remaining -= count,
        }
    }
    Ok(())
}
fn handle_connection(
    inner: Arc<Inner>,
    id: u64,
    mut socket: TcpStream,
    cancelled: Arc<AtomicBool>,
) {
    let result = (|| -> io::Result<()> {
        socket.set_read_timeout(Some(inner.options.request_timeout))?;
        let token = inner
            .endpoint
            .lock()
            .map_err(poison)?
            .as_ref()
            .unwrap()
            .token
            .clone();
        let (payload, mut input) = match read_request(&socket, &token, inner.options.max_body_bytes)
        {
            Ok(value) => value,
            Err(RequestError::Invalid(message)) => return reject_request(&mut socket, &message),
            Err(RequestError::Io(error)) => return Err(error),
        };
        socket.set_read_timeout(Some(Duration::from_millis(250)))?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let compute = inner.clone();
        let compute_cancelled = cancelled.clone();
        thread::spawn(move || {
            let checker = || {
                if compute_cancelled.load(Ordering::Acquire)
                    || compute.closed.load(Ordering::Acquire)
                {
                    Err(Cancellation(Arc::new(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "mcdev completion request cancelled",
                    ))))
                } else {
                    Ok(())
                }
            };
            let result = catch_unwind(AssertUnwindSafe(|| (compute.execute)(payload, &checker)))
                .unwrap_or_else(|error| {
                    Err(CommandFailure::Runtime(
                        error
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
                            .unwrap_or_else(|| "mcdev completion computation panicked".into()),
                    ))
                });
            let _ = sender.send(result);
        });
        let watch = inner.clone();
        let watch_cancelled = cancelled.clone();
        thread::spawn(move || {
            while !watch.closed.load(Ordering::Acquire) && !watch_cancelled.load(Ordering::Acquire)
            {
                let mut byte = [0];
                match input.read(&mut byte) {
                    Ok(0) => {
                        watch_cancelled.store(true, Ordering::Release);
                        return;
                    }
                    Ok(_) => (),
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                        ) =>
                    {
                        ()
                    }
                    Err(error) => {
                        if !watch.closed.load(Ordering::Acquire)
                            && !watch_cancelled.load(Ordering::Acquire)
                        {
                            (watch.options.on_failure)(error.to_string());
                        }
                        watch_cancelled.store(true, Ordering::Release);
                        return;
                    }
                }
            }
        });
        let started = Instant::now();
        loop {
            if cancelled.load(Ordering::Acquire) || inner.closed.load(Ordering::Acquire) {
                return Ok(());
            }
            let remaining = inner
                .options
                .request_timeout
                .saturating_sub(started.elapsed());
            if remaining.is_zero() {
                cancelled.store(true, Ordering::Release);
                return write_response(
                    &mut socket,
                    504,
                    &envelope("mcdev completion request timed out"),
                );
            }
            match receiver.recv_timeout(remaining.min(Duration::from_millis(250))) {
                Ok(Ok(value)) => {
                    cancelled.store(true, Ordering::Release);
                    return write_response(&mut socket, 200, &value);
                }
                Ok(Err(CommandFailure::Cancelled(_))) => {
                    if cancelled.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    cancelled.store(true, Ordering::Release);
                    return write_response(
                        &mut socket,
                        499,
                        &envelope("mcdev completion request cancelled"),
                    );
                }
                Ok(Err(CommandFailure::Unavailable(error))) => {
                    cancelled.store(true, Ordering::Release);
                    return write_response(
                        &mut socket,
                        200,
                        &serde_json::to_value(McdevResponseEnvelope::<Value>::failure(error))?,
                    );
                }
                Ok(Err(CommandFailure::Runtime(message))) => {
                    (inner.options.on_failure)(message.clone());
                    cancelled.store(true, Ordering::Release);
                    return write_response(&mut socket, 500, &envelope(&message));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::other("mcdev completion worker disconnected"))
                }
            }
        }
    })();
    if let Err(error) = result {
        if !inner.closed.load(Ordering::Acquire) && !cancelled.load(Ordering::Acquire) {
            (inner.options.on_failure)(error.to_string());
        }
    }
    cancelled.store(true, Ordering::Release);
    inner
        .active
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    let _ = socket.shutdown(Shutdown::Both);
}
fn random_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|e| io::Error::other(format!("OS random source failed: {e}")))?;
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut token = String::with_capacity(43);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        token.push(char::from(TABLE[(a >> 2) as usize]));
        token.push(char::from(TABLE[(((a & 3) << 4) | (b >> 4)) as usize]));
        if chunk.len() > 1 {
            token.push(char::from(TABLE[(((b & 15) << 2) | (c >> 6)) as usize]));
        }
        if chunk.len() > 2 {
            token.push(char::from(TABLE[(c & 63) as usize]));
        }
    }
    Ok(token)
}
