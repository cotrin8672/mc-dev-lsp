//! Evaluation-scoped adapters for negotiated host facts and selected inputs.
pub mod bridge;
pub mod draft;
pub mod inputs;

use mcdev_core::host_source::{HostFact, HostSourceError};
use mcdev_protocol::{McdevError, McdevErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason", rename_all = "camelCase")]
pub enum FactsError {
    Unsupported(String),
    Unknown(String),
    Stale(String),
    Cancelled(String),
    Protocol(String),
    BytecodeUnavailable(String),
}
impl std::fmt::Display for FactsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(s)
            | Self::Unknown(s)
            | Self::Stale(s)
            | Self::Cancelled(s)
            | Self::Protocol(s)
            | Self::BytecodeUnavailable(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for FactsError {}
impl FactsError {
    pub fn from_core(error: HostSourceError, bytecode: bool) -> Self {
        match error {
            HostSourceError::Cancelled(s) | HostSourceError::Interrupted(s) => Self::Cancelled(s),
            HostSourceError::Failure(s) if bytecode => Self::BytecodeUnavailable(s),
            HostSourceError::Failure(s) => Self::Unknown(s),
        }
    }
    pub fn command(self) -> crate::commands::CommandFailure {
        match self {
            Self::Cancelled(s) => crate::commands::CommandFailure::Cancelled(
                mcdev_core::expression_engine::Cancellation(std::sync::Arc::new(
                    std::io::Error::new(std::io::ErrorKind::Interrupted, s),
                )),
            ),
            other => crate::commands::CommandFailure::Unavailable(other.protocol()),
        }
    }
    pub fn core(self) -> HostSourceError {
        match self {
            Self::Cancelled(s) => HostSourceError::Cancelled(s),
            other => HostSourceError::Failure(other.to_string()),
        }
    }
    pub fn protocol(&self) -> McdevError {
        McdevError {
            code: match self {
                Self::BytecodeUnavailable(_) => McdevErrorCode::BytecodeUnavailable,
                Self::Protocol(_) => McdevErrorCode::InternalError,
                _ => McdevErrorCode::IncompleteProjectContext,
            },
            message: self.to_string(),
            details: BTreeMap::new(),
        }
    }
}

pub fn require_complete<T>(fact: HostFact<T>, bytecode: bool) -> Result<Option<T>, FactsError> {
    match fact {
        HostFact::Known(value) => Ok(Some(value)),
        HostFact::Missing => Ok(None),
        HostFact::Unavailable { reason } => Err(if bytecode {
            FactsError::BytecodeUnavailable(reason)
        } else {
            FactsError::Unknown(reason)
        }),
    }
}
