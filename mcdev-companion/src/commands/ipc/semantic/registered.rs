use super::*;
use crate::host_facts::inputs::SourceLocation;
use mcdev_core::{
    extras_model::HostShareSource,
    host_source::HostSemanticEnvironment,
    mixin_facade::{HostMixinSourceProvider, RegisteredHandlerSource, RegisteredMixinSource},
    project::MixinConfigRef,
};
use std::path::Path;

/// Resource reads retain their captured provider; type/source association is resolved by the host.
pub struct RegisteredSourceProvider {
    base: Arc<dyn HostMixinSourceProvider>,
    semantic: Arc<HostSemanticProvider>,
    source_locations: bool,
}
impl RegisteredSourceProvider {
    pub fn new(
        base: Arc<dyn HostMixinSourceProvider>,
        semantic: Arc<HostSemanticProvider>,
        source_locations: bool,
    ) -> Self {
        Self {
            base,
            semantic,
            source_locations,
        }
    }
}
impl HostSemanticProvider {
    fn registered_snapshot(
        &self,
        fqn: &str,
        current_path: Option<&Path>,
    ) -> Result<Option<mcdev_core::SourceScanEntry>, FactsError> {
        self.facts.check_active()?;
        let ty: Fact<TypeFact> = self
            .read(&[json!({"op":"resolveType","name":fqn})])?
            .pop()
            .ok_or_else(|| FactsError::Protocol("missing registered type fact".into()))?;
        let ty = require_fact(ty, "registered Mixin type in the selected source scope")?;
        validate_type(&ty, &mut Registry::default())?;
        validate_source_context(ty.read_only, ty.context_source.as_ref(), None)?;
        let location: Fact<SourceLocation> = self
            .read(&[json!({"op":"sourceLocation","target":ty.id})])?
            .pop()
            .ok_or_else(|| FactsError::Protocol("missing registered source location".into()))?;
        let location = require_fact(location, "registered Mixin source location")?;
        let range = super::super::definition::checked_location_range(&location, &ty.id)?;
        let path = mcdev_core::project::normalize_path(
            &mcdev_core::project::uri_to_path(&location.document_uri)
                .map_err(FactsError::Protocol)?,
        );
        let (_, snapshot) = self.project_sources.get(&path).ok_or_else(|| {
            FactsError::Unknown(
                "registered Mixin is outside the accepted project source snapshots".into(),
            )
        })?;
        if location.source.identity != snapshot.descriptor.identity
            || location.source.sha256 != snapshot.sha256
        {
            return Err(FactsError::Stale(
                "registered Mixin source identity disagrees with its accepted snapshot".into(),
            ));
        }
        if mcdev_core::position_to_offset(&snapshot.text, range.start).is_none()
            || mcdev_core::position_to_offset(&snapshot.text, range.end).is_none()
        {
            return Err(FactsError::Protocol(
                "registered Mixin name range is outside its accepted source".into(),
            ));
        }
        self.facts.check_active()?;
        Ok(
            if current_path
                .is_some_and(|current| mcdev_core::project::normalize_path(current) == path)
            {
                None
            } else {
                Some(mcdev_core::SourceScanEntry {
                    document_uri: location.document_uri,
                    text: snapshot.text.clone(),
                })
            },
        )
    }
}
impl HostMixinSourceProvider for RegisteredSourceProvider {
    fn registered_handler_source(
        &self,
        fqn: &str,
        current_path: Option<&Path>,
    ) -> HostSourceResult<RegisteredHandlerSource> {
        if !self.source_locations || !self.semantic.source_facts_v2 {
            return Ok(HostAnswer::unavailable(
                "normalized registered handler source inputs are unavailable",
            ));
        }
        let result = (|| {
            let Some(snapshot) = self.semantic.registered_snapshot(fqn, current_path)? else {
                return Ok(RegisteredHandlerSource::Current);
            };
            let model =
                self.semantic
                    .parse(&HostSemanticRequest {
                        source: &snapshot.text,
                        source_uri: &snapshot.document_uri,
                        document_version: None,
                        environment: &HostSemanticEnvironment::default(),
                        project: None,
                    })
                    .map_err(|error| match error {
                        HostSourceError::Cancelled(message)
                        | HostSourceError::Interrupted(message) => FactsError::Cancelled(message),
                        HostSourceError::Failure(message) => FactsError::Unknown(message),
                    })?;
            let model = match model.fact {
                HostFact::Known(model) => model,
                HostFact::Unavailable { reason } => return Err(FactsError::Unknown(reason)),
                HostFact::Missing => {
                    return Err(FactsError::Unknown(
                        "registered source semantic model is missing".into(),
                    ))
                }
            };
            let handlers = model.model.normalized_handler_source.ok_or_else(|| {
                FactsError::Unknown("registered source handler facts are unavailable".into())
            })?;
            if handlers.document_uri != snapshot.document_uri {
                return Err(FactsError::Stale("registered handler URI changed".into()));
            }
            handlers
                .checked_sites(&snapshot.text)
                .map_err(|error| FactsError::Unknown(error.to_string()))?;
            self.semantic.facts.check_active()?;
            Ok(RegisteredHandlerSource::Source(HostShareSource {
                text: snapshot.text,
                handlers,
            }))
        })();
        match result {
            Ok(source) => Ok(HostAnswer::known(source, vec![])),
            Err(FactsError::Cancelled(message)) => Err(HostSourceError::Cancelled(message)),
            Err(error) => Ok(HostAnswer::unavailable(error.to_string())),
        }
    }
    fn config(&self, path: &Path) -> HostSourceResult<MixinConfigRef> {
        self.semantic
            .facts
            .check_active()
            .map_err(FactsError::core)?;
        self.base.config(path)
    }
    fn read_source_file(&self, path: &Path) -> HostSourceResult<String> {
        self.semantic
            .facts
            .check_active()
            .map_err(FactsError::core)?;
        self.base.read_source_file(path)
    }
    fn compilation_unit_source(
        &self,
        fqn: &str,
        current_path: Option<&Path>,
    ) -> HostSourceResult<RegisteredMixinSource> {
        if !self.source_locations {
            return self.base.compilation_unit_source(fqn, current_path);
        }
        match self.semantic.registered_snapshot(fqn, current_path) {
            Ok(source) => Ok(HostAnswer::known(
                source.map_or(RegisteredMixinSource::Current, |source| {
                    RegisteredMixinSource::Source(source.text)
                }),
                vec![],
            )),
            Err(FactsError::Cancelled(message)) => Err(HostSourceError::Cancelled(message)),
            Err(error) => Ok(HostAnswer::unavailable(error.to_string())),
        }
    }
}
