use super::*;
use crate::mixin_source::MixinClassModel;
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostSemanticEnvironment {
    pub classpath_entries: Vec<String>,
    pub sourcepath_entries: Vec<String>,
    pub unit_name: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticAvailability {
    Normalized,
    /// An explicit stable host capability absence, not an exception message.
    StableFallback,
    TransientFallback,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSemanticModel {
    pub model: MixinClassModel,
    pub availability: SemanticAvailability,
    pub parse_ms: u64,
}
impl HostSemanticModel {
    /// Stable capability absence and transient provider failures use the existing
    /// MC domain parser, retaining visible uncertainty and a host-supplied reason.
    pub fn fallback(source: &str, uri: &str, reason: &str, stable: bool, parse_ms: u64) -> Self {
        use crate::mixin_source::{
            MixinSemanticModelParser, ParseConfidence, ParseSource, SemanticParseDebugInfo,
        };
        Self {
            model: MixinSemanticModelParser::parse_with_metadata(
                source,
                uri,
                ParseSource::HandWrittenFallback,
                ParseConfidence::Low,
                vec![reason.into()],
                SemanticParseDebugInfo {
                    parse_source: ParseSource::HandWrittenFallback,
                    fallback_reason: Some(reason.into()),
                    ..Default::default()
                },
                vec![],
            ),
            availability: if stable {
                SemanticAvailability::StableFallback
            } else {
                SemanticAvailability::TransientFallback
            },
            parse_ms,
        }
    }
}
pub struct HostSemanticRequest<'a> {
    pub source: &'a str,
    pub source_uri: &'a str,
    pub document_version: Option<i64>,
    pub environment: &'a HostSemanticEnvironment,
    pub project: Option<&'a HostProjectKey>,
}
pub trait HostSemanticModelProvider: HostReadValidator {
    fn parse(&self, request: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel>;
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SemanticKey {
    source_uri: String,
    version: Option<i64>,
    source_hash: [u8; 32],
    project: HostProjectKey,
    classpath: Vec<String>,
    sourcepath: Vec<String>,
    unit_name: Option<String>,
}
pub struct SemanticModelCache {
    cache: HostFactCache<SemanticKey, HostSemanticModel>,
}
impl Default for SemanticModelCache {
    fn default() -> Self {
        Self::new(64, Duration::from_secs(600))
    }
}
impl SemanticModelCache {
    pub fn new(capacity: usize, max_age: Duration) -> Self {
        Self {
            cache: HostFactCache::new(capacity, Some(max_age)),
        }
    }
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
    pub fn invalidate(&self, uri: &str) {
        self.cache.retain(|k| k.source_uri != uri);
    }
    pub fn clear(&self) {
        self.cache.clear();
    }
    pub fn get(
        &self,
        request: &HostSemanticRequest<'_>,
        provider: &dyn HostSemanticModelProvider,
    ) -> Result<CachedHostAnswer<HostSemanticModel>, HostSourceError> {
        // No complete generation identity => no reuse. A source hash cannot
        // establish namespace/classpath/model currency on its own.
        let Some(project) = request.project else {
            return Ok(CachedHostAnswer {
                answer: provider.parse(request)?,
                cache_hit: false,
            });
        };
        let key = SemanticKey {
            source_uri: request.source_uri.into(),
            version: request.document_version,
            source_hash: Sha256::digest(request.source.as_bytes()).into(),
            project: project.clone(),
            classpath: request.environment.classpath_entries.clone(),
            sourcepath: request.environment.sourcepath_entries.clone(),
            unit_name: request.environment.unit_name.clone(),
        };
        self.cache.get_or_compute_cacheable(
            key,
            project.generations,
            provider,
            || {
                let answer = provider.parse(request)?;
                // A transient fallback still produces a useful model this request,
                // but never becomes a stable cached success.
                Ok(answer)
            },
            |m| m.availability != SemanticAvailability::TransientFallback,
        )
    }
}
