use mcdev_companion::{commands::CommandFailure, completion_transport::*};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpStream},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

fn options(timeout: u64) -> CompletionTransportOptions {
    CompletionTransportOptions {
        request_timeout: Duration::from_millis(timeout),
        notify_endpoint: Arc::new(|_| Ok(true)),
        ..Default::default()
    }
}
fn send_header(socket: &mut TcpStream, endpoint: &Endpoint, payload: &str, auth: &str) {
    write!(socket,"POST /completion HTTP/1.1\r\nHost: {}:{}\r\nAuthorization: {auth}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",endpoint.host,endpoint.port,payload.len()).unwrap();
    socket.flush().unwrap();
}
fn request(endpoint: &Endpoint, payload: Value, auth: Option<&str>) -> (u16, String) {
    let mut socket = TcpStream::connect((&*endpoint.host, endpoint.port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    send_header(
        &mut socket,
        endpoint,
        &payload.to_string(),
        auth.unwrap_or(&format!("Bearer {}", endpoint.token)),
    );
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    let (header, body) = response.split_once("\r\n\r\n").unwrap();
    let length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(length, body.len());
    (
        header.split_whitespace().nth(1).unwrap().parse().unwrap(),
        body.into(),
    )
}
pub fn replay(case: &str) {
    match case.rsplit("::").next().unwrap() {
        "authenticatedRequestsAreHandledConcurrentlyAndReturnJson" => {
            let transport = CompletionTransport::new(
                Arc::new(|payload, _| {
                    thread::sleep(Duration::from_millis(120));
                    Ok(json!({"result":payload,"metadata":{"source":"same-jvm"}}))
                }),
                options(1000),
            )
            .unwrap();
            let endpoint = transport.start().unwrap();
            assert_eq!(endpoint.host, "127.0.0.1");
            assert!(endpoint.token.len() >= 43);
            let first_endpoint = endpoint.clone();
            let first = thread::spawn(move || request(&first_endpoint, json!({"request":1}), None));
            let second = thread::spawn(move || request(&endpoint, json!({"request":2}), None));
            let first = first.join().unwrap();
            let second = second.join().unwrap();
            assert_eq!(first.0, 200);
            assert_eq!(second.0, 200);
            assert!(first.1.contains("same-jvm"));
            assert!(second.1.contains("request"));
            transport.close();
        }
        "requestDeadlineProducesBoundedError" => {
            let transport = CompletionTransport::new(
                Arc::new(|_, checker| {
                    for _ in 0..200 {
                        thread::sleep(Duration::from_millis(10));
                        checker.check_cancelled()?;
                    }
                    Ok(json!({}))
                }),
                options(50),
            )
            .unwrap();
            let endpoint = transport.start().unwrap();
            let response = request(&endpoint, json!({"request":1}), None);
            assert_eq!(response.0, 504);
            assert!(response.1.contains("timed out"));
            transport.close();
        }
        "rejectsUnauthenticatedRequestBeforeDispatch" => {
            let executions = Arc::new(AtomicUsize::new(0));
            let count = executions.clone();
            let transport = CompletionTransport::new(
                Arc::new(move |_, _| {
                    count.fetch_add(1, Ordering::Relaxed);
                    Ok(json!({}))
                }),
                options(5000),
            )
            .unwrap();
            let endpoint = transport.start().unwrap();
            let response = request(&endpoint, json!({"request":1}), Some("Bearer wrong-token"));
            assert_eq!(response.0, 400);
            assert!(response.1.contains("authentication failed"));
            assert_eq!(executions.load(Ordering::Relaxed), 0);
            transport.close();
        }
        "disconnectCancelsInFlightComputation" => {
            let (started_send, started) = mpsc::channel();
            let (cancelled_send, cancelled) = mpsc::channel();
            let transport = CompletionTransport::new(
                Arc::new(move |_, checker| {
                    started_send.send(()).unwrap();
                    for _ in 0..1000 {
                        thread::sleep(Duration::from_millis(10));
                        if let Err(error) = checker.check_cancelled() {
                            cancelled_send.send(()).unwrap();
                            return Err(CommandFailure::Cancelled(error));
                        }
                    }
                    Ok(json!({}))
                }),
                options(5000),
            )
            .unwrap();
            let endpoint = transport.start().unwrap();
            let mut socket = TcpStream::connect((&*endpoint.host, endpoint.port)).unwrap();
            send_header(
                &mut socket,
                &endpoint,
                "{}",
                &format!("Bearer {}", endpoint.token),
            );
            started.recv_timeout(Duration::from_secs(2)).unwrap();
            socket.shutdown(Shutdown::Write).unwrap();
            drop(socket);
            cancelled.recv_timeout(Duration::from_secs(2)).unwrap();
            transport.close();
        }
        "completedRequestDoesNotReportServerTeardownAsFailure" => {
            let (send, receive) = mpsc::channel();
            let mut settings = options(5000);
            settings.on_failure = Arc::new(move |error| {
                let _ = send.send(error);
            });
            let transport =
                CompletionTransport::new(Arc::new(|_, _| Ok(json!({"ok":true}))), settings)
                    .unwrap();
            let endpoint = transport.start().unwrap();
            assert_eq!(request(&endpoint, json!({}), None).0, 200);
            assert!(matches!(
                receive.recv_timeout(Duration::from_secs(2)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ));
            transport.close();
        }
        other => panic!("unmapped HTTP case {other}"),
    }
}

#[test]
fn framed_http_validation_recovers_and_counts_utf8_bytes() {
    let executions = Arc::new(AtomicUsize::new(0));
    let count = executions.clone();
    let mut settings = options(1000);
    settings.max_body_bytes = 32;
    let transport = CompletionTransport::new(
        Arc::new(move |value, _| {
            count.fetch_add(1, Ordering::Relaxed);
            Ok(value)
        }),
        settings,
    )
    .unwrap();
    let endpoint = transport.start().unwrap();
    let raw = |request: String| {
        let mut socket = TcpStream::connect((&*endpoint.host, endpoint.port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket.write_all(request.as_bytes()).unwrap();
        socket.flush().unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 400 "), "{response}");
    };
    let headers = format!("Authorization: Bearer {}\r\n", endpoint.token);
    for request in [
        format!("GET /completion HTTP/1.1\r\n{headers}Content-Length: 2\r\n\r\n{{}}"),
        format!("POST /other HTTP/1.1\r\n{headers}Content-Length: 2\r\n\r\n{{}}"),
        format!("POST /completion HTTP/1.1\r\n{headers}\r\n"),
        format!("POST /completion HTTP/1.1\r\n{headers}Content-Length: -1\r\n\r\n"),
        format!("POST /completion HTTP/1.1\r\n{headers}Content-Length: 33\r\n\r\n"),
        format!("POST /completion HTTP/1.1\r\n{headers}Content-Length: 0\r\n\r\n"),
        format!("POST /completion HTTP/1.1\r\n{headers}Content-Length: 2\r\n\r\n[]"),
        format!("POST /completion HTTP/1.1\r\n{headers}Content-Length: 2\r\n\r\n{{x"),
    ] {
        raw(request);
    }
    assert_eq!(executions.load(Ordering::Relaxed), 0);
    let response = request(&endpoint, json!({"value":"😀日本語"}), None);
    assert_eq!(response.0, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&response.1).unwrap(),
        json!({"value":"😀日本語"})
    );
    assert_eq!(executions.load(Ordering::Relaxed), 1);
    transport.close();
    transport.close();
    assert!(CompletionTransport::new(Arc::new(|_, _| Ok(json!({}))), options(0)).is_err());
}

#[test]
fn computation_errors_and_explicit_cancellation_have_distinct_responses() {
    let (send, receive) = mpsc::channel();
    let mut settings = options(1000);
    settings.on_failure = Arc::new(move |message| send.send(message).unwrap());
    let transport = CompletionTransport::new(
        Arc::new(|value, _| {
            if value["cancel"] == true {
                Err(CommandFailure::Cancelled(
                    mcdev_core::expression_engine::Cancellation(Arc::new(std::io::Error::other(
                        "host cancelled",
                    ))),
                ))
            } else {
                Err(CommandFailure::Runtime("host facts unavailable".into()))
            }
        }),
        settings,
    )
    .unwrap();
    let endpoint = transport.start().unwrap();
    assert_eq!(request(&endpoint, json!({"cancel":true}), None).0, 499);
    assert!(receive.try_recv().is_err());
    let response = request(&endpoint, json!({}), None);
    assert_eq!(response.0, 500);
    assert!(response.1.contains("host facts unavailable"));
    assert_eq!(
        receive.recv_timeout(Duration::from_secs(1)).unwrap(),
        "host facts unavailable"
    );
    transport.close();
}

#[test]
fn independent_authenticated_requests_overlap() {
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let gate = barrier.clone();
    let transport = CompletionTransport::new(
        Arc::new(move |payload, _| {
            gate.wait();
            Ok(payload)
        }),
        options(1000),
    )
    .unwrap();
    let endpoint = transport.start().unwrap();
    let first_endpoint = endpoint.clone();
    let first = thread::spawn(move || request(&first_endpoint, json!({"request":1}), None));
    let second = thread::spawn(move || request(&endpoint, json!({"request":2}), None));
    assert_eq!(first.join().unwrap().0, 200);
    assert_eq!(second.join().unwrap().0, 200);
    transport.close();
}

#[test]
fn unavailable_facts_are_http_200_business_errors_and_authentication_stays_400() {
    let count = Arc::new(AtomicUsize::new(0));
    let executions = count.clone();
    let transport = CompletionTransport::new(
        Arc::new(move |_, _| {
            executions.fetch_add(1, Ordering::Relaxed);
            Err(CommandFailure::Unavailable(mcdev_protocol::McdevError {
                code: mcdev_protocol::McdevErrorCode::BytecodeUnavailable,
                message: "Code/LVT not provided by header facts".into(),
                details: Default::default(),
            }))
        }),
        options(1000),
    )
    .unwrap();
    let endpoint = transport.start().unwrap();
    let response = request(&endpoint, json!({}), None);
    assert_eq!(response.0, 200);
    let value: Value = serde_json::from_str(&response.1).unwrap();
    assert_eq!(value["error"]["code"], "BYTECODE_UNAVAILABLE");
    assert!(value.get("result").is_none());
    let rejected = request(&endpoint, json!({}), Some("Bearer wrong-token"));
    assert_eq!(rejected.0, 400);
    assert!(rejected.1.contains("authentication failed"));
    assert_eq!(count.load(Ordering::Relaxed), 1);
    transport.close();
}
