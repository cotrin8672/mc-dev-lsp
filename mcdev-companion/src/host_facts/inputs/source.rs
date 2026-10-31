use super::*;
use crate::wire::{validate_declarations, Declaration, Registry};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFacts {
    pub declarations: Vec<Declaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_scope: Option<Fact<()>>,
}

impl SourceFacts {
    /// Validate against the exact evaluation text or checked snapshot, never a disk read.
    pub fn validate(&self, text: &str, registry: &mut Registry) -> Result<(), FactsError> {
        self.validate_context()?;
        validate_declarations(&self.declarations, text, registry, true).map_err(protocol)
    }

    fn validate_context(&self) -> Result<(), FactsError> {
        match (&self.source, &self.semantic_scope) {
            (None, None) => (),
            (Some(source), Some(scope)) => {
                validate_source(source)?;
                validate_fact(scope)?;
                if scope.found().is_none() && !self.declarations.is_empty() {
                    return Err(protocol(
                        "unknown foreign source scope returned declarations",
                    ));
                }
            }
            _ => {
                return Err(protocol(
                    "foreign source facts lack their snapshot or semantic scope",
                ))
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourcePosition {
    pub line: u32,
    pub character: u32,
}

/// LSP UTF-16 positions, distinct from sourceFacts UTF-8 byte ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocationRange {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocationIdentity {
    pub identity: InputIdentity,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub target: String,
    pub document_uri: String,
    pub range: SourceLocationRange,
    pub source: SourceLocationIdentity,
}

impl SourceLocation {
    fn validate(&self, target: &str) -> Result<(), FactsError> {
        if self.target != target
            || !self.document_uri.starts_with("file://")
            || mcdev_core::project::uri_to_path(&self.document_uri).is_err()
            || self.range.start > self.range.end
            || !valid_identity(&self.source.identity, "source")
            || !valid_digest(&self.source.sha256)
        {
            return Err(protocol(
                "source location target, URI, range or source identity mismatch",
            ));
        }
        Ok(())
    }
}

impl<R: FactReader> ScopedInputs<R> {
    pub fn source_facts(
        &mut self,
        source: Option<&SourceEntry>,
    ) -> Result<SourceFacts, FactsError> {
        if let Some(source) = source {
            return Ok(self
                .source_facts_many(std::slice::from_ref(source))?
                .remove(0));
        }
        let facts: SourceFacts = self.one(InputOp::SourceFacts, json!({"op":"sourceFacts"}))?;
        facts.validate_context()?;
        if facts.source.is_some() {
            return Err(protocol(
                "owner sourceFacts returned a foreign snapshot context",
            ));
        }
        Ok(facts)
    }

    pub fn source_facts_many(
        &mut self,
        sources: &[SourceEntry],
    ) -> Result<Vec<SourceFacts>, FactsError> {
        for source in sources {
            check_entry(
                source.scope,
                self.scope,
                &source.descriptor.id,
                &source.descriptor,
                &self.sources,
            )?;
        }
        let queries: Vec<_> = sources
            .iter()
            .map(|source| json!({"op":"sourceFacts","source":source.descriptor.id}))
            .collect();
        self.read(InputOp::SourceFacts, &queries)?
            .into_iter()
            .zip(sources)
            .map(|(value, source)| {
                let facts: SourceFacts = decode(value)?;
                facts.validate_context()?;
                if facts.source.as_ref() != Some(&source.descriptor) {
                    return Err(protocol("sourceFacts snapshot identity mismatch"));
                }
                Ok(facts)
            })
            .collect()
    }

    /// Targets must be host-issued identities; the host checks their kind and live evaluation.
    pub fn source_location(&mut self, target: &str) -> Result<Fact<SourceLocation>, FactsError> {
        Ok(self.source_locations(&[target.to_owned()])?.remove(0))
    }

    pub fn source_locations(
        &mut self,
        targets: &[String],
    ) -> Result<Vec<Fact<SourceLocation>>, FactsError> {
        if targets
            .iter()
            .any(|target| !valid_token(target) || target.len() > 256)
        {
            return Err(protocol("invalid source-location target token"));
        }
        let queries: Vec<_> = targets
            .iter()
            .map(|target| json!({"op":"sourceLocation","target":target}))
            .collect();
        self.read(InputOp::SourceLocation, &queries)?
            .into_iter()
            .zip(targets)
            .map(|(value, target)| {
                let fact: Fact<SourceLocation> = decode(value)?;
                validate_fact(&fact)?;
                match &fact {
                    Fact::Found { value } => value.validate(target)?,
                    Fact::Unknown { .. } => (),
                    Fact::DefinitelyAbsent => {
                        return Err(protocol("sourceLocation cannot certify absence"))
                    }
                }
                Ok(fact)
            })
            .collect()
    }
}
