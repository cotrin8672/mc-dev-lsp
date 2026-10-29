//! Evaluation-scoped bridge between command computation and the framed RPC actor.
use crate::{
    host_facts::{bridge::FactReader, FactsError},
    wire::MAX_BATCH,
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    time::Instant,
};

pub type CommandHandler =
    dyn Fn(Value, Value, CommandFacts) -> Result<Value, FactsError> + Send + Sync + 'static;

pub(crate) enum CommandEvent {
    Request {
        method: String,
        params: Value,
        reply: SyncSender<Result<Value, FactsError>>,
    },
    Finished(Result<Value, FactsError>),
}

struct Lease {
    evaluation: Value,
    initialization: Value,
    deadline: Instant,
    active: AtomicBool,
    request_gate: Mutex<()>,
    events: SyncSender<CommandEvent>,
}

/// Clones share one evaluation lease; no handle can outlive its host transaction.
#[derive(Clone)]
pub struct CommandFacts(Arc<Lease>);

impl CommandFacts {
    pub(crate) fn new(
        evaluation: Value,
        initialization: Value,
        deadline: Instant,
    ) -> (Self, Receiver<CommandEvent>) {
        let (events, receiver) = mpsc::sync_channel(32);
        (
            Self(Arc::new(Lease {
                evaluation,
                initialization,
                deadline,
                active: AtomicBool::new(true),
                request_gate: Mutex::new(()),
                events,
            })),
            receiver,
        )
    }

    pub(crate) fn close(&self) {
        self.0.active.store(false, Ordering::Release);
    }

    pub(crate) fn finished(&self, value: Result<Value, FactsError>) {
        let _ = self.0.events.try_send(CommandEvent::Finished(value));
    }

    pub fn check_active(&self) -> Result<(), FactsError> {
        if !self.0.active.load(Ordering::Acquire) {
            return Err(FactsError::Cancelled("evaluation is closed".into()));
        }
        if Instant::now() >= self.0.deadline {
            self.close();
            return Err(FactsError::Cancelled("evaluation deadline exceeded".into()));
        }
        Ok(())
    }

    pub fn request(&self, method: &str, params: Value) -> Result<Value, FactsError> {
        self.check_active()?;
        if params["evaluation"] != self.0.evaluation {
            return Err(FactsError::Protocol("foreign evaluation request".into()));
        }
        match method {
            "analysis/readFacts" => {
                let queries = params["queries"]
                    .as_array()
                    .filter(|queries| !queries.is_empty() && queries.len() <= MAX_BATCH)
                    .ok_or_else(|| FactsError::Protocol("invalid host fact batch".into()))?;
                for query in queries {
                    let operation = query["op"].as_str().ok_or_else(|| {
                        FactsError::Protocol("invalid host fact operation".into())
                    })?;
                    let base = [
                        "resolveType",
                        "resolveTypeRef",
                        "declarationType",
                        "declarationMember",
                        "declaredMembers",
                    ]
                    .contains(&operation);
                    let optional = self.0.initialization["supportedOps"]
                        .as_object()
                        .is_some_and(|sets| {
                            sets.values().any(|set| {
                                set.as_array().is_some_and(|operations| {
                                    operations.iter().any(|value| value == operation)
                                })
                            })
                        });
                    if !base && !optional {
                        return Err(FactsError::Unsupported(format!(
                            "unnegotiated host operation: {operation}"
                        )));
                    }
                }
            }
            "analysis/require" => {
                params["requirements"]
                    .as_array()
                    .filter(|values| !values.is_empty() && values.len() <= MAX_BATCH)
                    .ok_or_else(|| FactsError::Protocol("invalid requirements batch".into()))?;
            }
            _ => return Err(FactsError::Unsupported("unsupported host request".into())),
        }
        let _guard = self
            .0
            .request_gate
            .lock()
            .map_err(|_| FactsError::Protocol("host request lock poisoned".into()))?;
        self.check_active()?;
        let (reply, receiver) = mpsc::sync_channel(1);
        self.0
            .events
            .try_send(CommandEvent::Request {
                method: method.into(),
                params,
                reply,
            })
            .map_err(|_| FactsError::Cancelled("host actor is unavailable".into()))?;
        let remaining = self.0.deadline.saturating_duration_since(Instant::now());
        let response = receiver
            .recv_timeout(remaining)
            .map_err(|_| FactsError::Cancelled("host request timed out or actor closed".into()))?;
        match response {
            Err(error) => Err(error),
            Ok(value) => {
                self.check_active()?;
                Ok(value)
            }
        }
    }
}

impl FactReader for CommandFacts {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.request(
            "analysis/readFacts",
            json!({"evaluation": evaluation, "queries": queries}),
        )
    }
}

pub use crate::server::run_with_command_handler;
