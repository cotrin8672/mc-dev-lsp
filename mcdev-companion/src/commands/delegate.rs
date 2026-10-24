use super::{host::*, runtime::CommandDispatcher};
use mcdev_core::expression_engine::{Cancellation, CancellationChecker};
use mcdev_protocol::{Command, ProtocolPayloadDecoder};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug)]
pub enum DelegateFailure {
    UnsupportedCommand(String),
    Cancelled(Cancellation),
    Runtime(String),
}
impl std::fmt::Display for DelegateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedCommand(s) | Self::Runtime(s) => f.write_str(s),
            Self::Cancelled(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for DelegateFailure {}
pub struct DelegateCommandHandler<H: CommandHost> {
    pub dispatcher: Arc<CommandDispatcher<H>>,
}
impl<H: CommandHost> DelegateCommandHandler<H> {
    pub fn new(dispatcher: Arc<CommandDispatcher<H>>) -> Self {
        Self { dispatcher }
    }
    pub fn execute_command(
        &self,
        command: &str,
        args: &[Value],
        cancellation: &dyn CancellationChecker,
    ) -> Result<Value, DelegateFailure> {
        if Command::parse(command).is_none() {
            return Err(DelegateFailure::UnsupportedCommand(format!(
                "mcdev command is not registered: {command}"
            )));
        }
        cancellation
            .check_cancelled()
            .map_err(DelegateFailure::Cancelled)?;
        let result = self
            .dispatcher
            .execute_with_cancellation(command, args, cancellation)
            .map_err(|e| match e {
                CommandFailure::Cancelled(e) => DelegateFailure::Cancelled(e),
                CommandFailure::Runtime(e) => DelegateFailure::Runtime(e),
                CommandFailure::Unavailable(e) => DelegateFailure::Runtime(e.message),
            })?;
        ProtocolPayloadDecoder
            .encode_to_map(&result)
            .map(Value::Object)
            .map_err(|e| DelegateFailure::Runtime(e.to_string()))
    }
}
