use mcdev_companion::transport::{read_frame, write_frame};
use serde_json::{json, Value};
use std::{
    fs,
    io::BufReader,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Process {
    pub child: Child,
    pub input: Option<ChildStdin>,
    output: Receiver<Result<Option<Value>, std::io::Error>>,
}
impl Process {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../build/companion-process-logs");
        fs::create_dir_all(&dir).unwrap();
        let name = format!(
            "{}-{}.stderr.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let stderr = fs::File::create(dir.join(name)).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_mcdev-companion"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || loop {
            let value = read_frame(&mut output);
            let end = !matches!(value, Ok(Some(_)));
            if tx.send(value).is_err() || end {
                break;
            }
        });
        Self {
            child,
            input,
            output: rx,
        }
    }
    pub fn send(&mut self, message: Value) {
        write_frame(self.input.as_mut().unwrap(), &message).unwrap();
    }
    pub fn receive(&self) -> Value {
        self.output
            .recv_timeout(Duration::from_secs(10))
            .expect("child response deadline")
            .unwrap()
            .expect("child EOF")
    }
    pub fn initialize(&mut self) {
        self.send(json!({"jsonrpc":"2.0","id":"initialize","method":"initialize","params":{"protocol":{"major":1,"minor":0},"provider":"mcdev","capabilities":["semantic-v1"]}}));
        assert_eq!(
            self.receive(),
            json!({"jsonrpc":"2.0","id":"initialize","result":{"protocol":{"major":1,"minor":0},"capabilities":["semantic-v1"]}})
        );
    }
    pub fn request(&mut self, id: &str, method: &str, params: Value) {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
    }
    pub fn eof(&mut self) {
        drop(self.input.take());
    }
    pub fn wait(&mut self) -> std::process::ExitStatus {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(std::time::Instant::now() < deadline, "child exit deadline");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn empty_evaluation(id: &str) -> Value {
    json!({"evaluation":id,"owner":"Unsaved.kt","source":"","configuration":{},"declarations":[]})
}
