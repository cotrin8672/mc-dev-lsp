use serde::{Deserialize, Serialize};

/// Values are owned normalized data. Opaque declaration/type/evaluation handles
/// belong to the provider's current evaluation and must never enter these facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "value", rename_all = "camelCase")]
pub enum HostFact<T> {
    Known(T),
    Missing,
    Unavailable { reason: String },
}
impl<T> HostFact<T> {
    pub fn known(&self) -> Option<&T> {
        if let Self::Known(value) = self {
            Some(value)
        } else {
            None
        }
    }
    pub fn is_available(&self) -> bool {
        !matches!(self, Self::Unavailable { .. })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReadStamp {
    pub kind: String,
    pub key: String,
    pub generation: u64,
    pub missing: bool,
}
pub type HostReadSet = Vec<HostReadStamp>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostAnswer<T> {
    pub fact: HostFact<T>,
    pub reads: HostReadSet,
}
impl<T> HostAnswer<T> {
    pub fn known(value: T, reads: HostReadSet) -> Self {
        Self {
            fact: HostFact::Known(value),
            reads,
        }
    }
    pub fn missing(reads: HostReadSet) -> Self {
        Self {
            fact: HostFact::Missing,
            reads,
        }
    }
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            fact: HostFact::Unavailable {
                reason: reason.into(),
            },
            reads: vec![],
        }
    }
}
/// Every dimension participates in cache identity; source text alone is insufficient.
/// Providers must supply all eight known counters. Zero is a known initial value,
/// never a substitute for an unavailable counter. Without this identity callers
/// must bypass reuse (for example HostSemanticRequest.project = None).
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct HostGenerations {
    pub project: u64,
    pub model: u64,
    pub namespace: u64,
    pub classpath: u64,
    pub dependency: u64,
    pub generated_source: u64,
    pub config: u64,
    pub provider: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum HostSourceError {
    Cancelled(String),
    Interrupted(String),
    Failure(String),
}
impl std::fmt::Display for HostSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled(s) | Self::Interrupted(s) | Self::Failure(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for HostSourceError {}
pub type HostSourceResult<T> = Result<HostAnswer<T>, HostSourceError>;

/// Called on every cache reuse in the current evaluation. A provider must both
/// validate positive/negative generations and register fresh current-evaluation
/// reads. Returning false bypasses/rebuilds the cache; no prior handles are reused.
pub trait HostReadValidator: Send + Sync {
    fn validate_current_reads(
        &self,
        generations: HostGenerations,
        reads: &HostReadSet,
    ) -> Result<bool, HostSourceError>;
}

/// Normalized class hierarchy/flags supplied by the host, including SDK types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostReferenceTypeFact {
    pub internal_name: String,
    pub super_internal_name: Option<String>,
    pub interface_internal_names: Vec<String>,
    pub access_flags: u32,
}
