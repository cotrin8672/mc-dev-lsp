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
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SourceProvenance {
    CurrentWorkspaceSource {
        target: String,
        source: SourceLocationIdentity,
    },
    SelectedDependencySource {
        target: String,
        source: SourceLocationIdentity,
        jvm_owner: String,
        origin_id: String,
    },
}

impl SourceProvenance {
    pub fn validate(&self, target: &str) -> Result<(), FactsError> {
        let (observed, source, jvm_owner, origin_id) = match self {
            Self::CurrentWorkspaceSource { target, source } => (target, source, None, None),
            Self::SelectedDependencySource {
                target,
                source,
                jvm_owner,
                origin_id,
            } => (target, source, Some(jvm_owner), Some(origin_id)),
        };
        if observed != target
            || !valid_token(observed)
            || !valid_identity(&source.identity, "source")
            || !valid_digest(&source.sha256)
            || jvm_owner.is_some_and(|owner| !valid_jvm_name(owner, false))
            || origin_id.is_some_and(|origin| !valid_token(origin))
        {
            return Err(protocol("source provenance identity or target mismatch"));
        }
        Ok(())
    }

    pub fn source(&self) -> &SourceLocationIdentity {
        match self {
            Self::CurrentWorkspaceSource { source, .. }
            | Self::SelectedDependencySource { source, .. } => source,
        }
    }

    pub fn selected_dependency(&self) -> Option<(&str, &str)> {
        match self {
            Self::SelectedDependencySource {
                jvm_owner,
                origin_id,
                ..
            } => Some((jvm_owner, origin_id)),
            Self::CurrentWorkspaceSource { .. } => None,
        }
    }
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

    pub fn source_provenance(
        &mut self,
        target: &str,
    ) -> Result<Fact<SourceProvenance>, FactsError> {
        if !valid_token(target) {
            return Err(protocol("invalid source-provenance target token"));
        }
        let value = self
            .read(
                InputOp::SourceProvenance,
                &[json!({"op":"sourceProvenance","target":target})],
            )?
            .into_iter()
            .next()
            .ok_or_else(|| protocol("missing source-provenance fact"))?;
        validate_source_provenance_shape(&value)?;
        let fact: Fact<SourceProvenance> = decode(value)?;
        validate_fact(&fact)?;
        match &fact {
            Fact::Found { value } => value.validate(target)?,
            Fact::Unknown { .. } => (),
            Fact::DefinitelyAbsent => {
                return Err(protocol("source provenance cannot certify absence"))
            }
        }
        Ok(fact)
    }
}

fn validate_source_provenance_shape(value: &Value) -> Result<(), FactsError> {
    let object = value
        .as_object()
        .ok_or_else(|| protocol("source provenance fact must be an object"))?;
    let state = object
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| protocol("source provenance fact lacks state"))?;
    match state {
        "found" => {
            if object.len() != 2 || !object.contains_key("value") {
                return Err(protocol(
                    "source provenance found fact has extra or missing fields",
                ));
            }
            let value = object["value"]
                .as_object()
                .ok_or_else(|| protocol("source provenance value must be an object"))?;
            let kind = value
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| protocol("source provenance value lacks kind"))?;
            match kind {
                "currentWorkspaceSource"
                    if value.len() == 3
                        && value.contains_key("target")
                        && value.contains_key("source") => {}
                "currentWorkspaceSource" => {
                    return Err(protocol(
                        "current workspace provenance has extra or missing fields",
                    ));
                }
                "selectedDependencySource" => {
                    let allowed = ["kind", "target", "source", "jvmOwner", "originId"];
                    if value.len() != allowed.len()
                        || !allowed.iter().all(|key| value.contains_key(*key))
                    {
                        return Err(protocol(
                            "selected dependency provenance has extra or missing fields",
                        ));
                    }
                }
                _ => return Err(protocol("unknown source provenance kind")),
            }
            let source = value["source"]
                .as_object()
                .ok_or_else(|| protocol("source provenance source must be an object"))?;
            if source.len() != 2
                || !source.contains_key("identity")
                || !source.contains_key("sha256")
            {
                return Err(protocol(
                    "source provenance source has extra or missing fields",
                ));
            }
            let identity = source["identity"]
                .as_object()
                .ok_or_else(|| protocol("source provenance identity must be an object"))?;
            if identity.len() != 2
                || !identity.contains_key("kind")
                || !identity.contains_key("revision")
            {
                return Err(protocol(
                    "source provenance identity has extra or missing fields",
                ));
            }
        }
        "unknown" => {
            if object.len() != 2 || !object.contains_key("reasons") {
                return Err(protocol(
                    "source provenance unknown fact has extra or missing fields",
                ));
            }
        }
        "definitelyAbsent" => {
            if object.len() != 1 {
                return Err(protocol(
                    "source provenance absence fact has extra or missing fields",
                ));
            }
        }
        _ => return Err(protocol("unknown source provenance fact state")),
    }
    Ok(())
}
