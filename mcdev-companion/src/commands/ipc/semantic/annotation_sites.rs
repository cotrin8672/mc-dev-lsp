use super::*;
use crate::host_facts::inputs::AnnotationSites;
use mcdev_core::mixin_source::HostSelectorCompletions;

pub(super) struct SiteFacts {
    pub facts: AnnotationSites,
    ownerless: Vec<Annotation>,
}

pub(super) fn collect_value_refs(argument: &Argument, refs: &mut BTreeSet<String>) {
    match argument.value.found() {
        Some(AnnotationValue::Class { type_ref } | AnnotationValue::Enum { type_ref, .. }) => {
            refs.insert(type_ref.clone());
        }
        Some(AnnotationValue::Array { values }) => {
            for argument in values {
                collect_value_refs(argument, refs);
            }
        }
        Some(AnnotationValue::Annotation { annotation }) => {
            for argument in &annotation.arguments {
                collect_value_refs(argument, refs);
            }
        }
        _ => (),
    }
}

impl SiteFacts {
    pub(super) fn reconcile(
        &mut self,
        source: &str,
        uri: &str,
        declarations: &[Declaration],
        detailed: bool,
        registry: &mut Registry,
        identities: &BTreeMap<String, Fact<TypeFact>>,
    ) -> Result<(), FactsError> {
        self.ownerless = self
            .facts
            .validate_with_types(
                source,
                uri,
                None,
                declarations,
                detailed,
                registry,
                identities,
            )?
            .into_iter()
            .cloned()
            .collect();
        Ok(())
    }
}

impl HostSemanticProvider {
    pub(super) fn read_annotation_sites(
        &self,
        source: &str,
        uri: &str,
        selected: Option<&SourceEntry>,
        registry: &mut Registry,
    ) -> Result<Option<SiteFacts>, FactsError> {
        if !self.annotation_sites {
            return Ok(None);
        }
        let mut query = json!({"op":"annotationSites"});
        if let Some(selected) = selected {
            query["source"] = json!(selected.descriptor.id);
        }
        let facts: AnnotationSites = self
            .read(&[query])?
            .pop()
            .ok_or_else(|| FactsError::Protocol("missing annotation site inventory".into()))?;
        let known_owner = mcdev_core::project::uri_to_path(uri)
            .ok()
            .map(|path| mcdev_core::project::normalize_path(&path))
            .and_then(|path| self.project_sources.get(&path))
            .filter(|(_, snapshot)| snapshot.text == source);
        let expected = selected
            .map(|entry| &entry.descriptor)
            .or_else(|| known_owner.map(|(entry, _)| &entry.descriptor));
        facts.validate_source(source, uri, expected)?;
        facts.validate_values(source, registry)?;
        Ok(Some(SiteFacts {
            facts,
            ownerless: Vec::new(),
        }))
    }

    pub(super) fn project_annotation_selectors(
        &self,
        sites: &SiteFacts,
        mixin: &Declaration,
        identities: &BTreeMap<String, Fact<TypeFact>>,
        source: &str,
        current_owner: bool,
    ) -> Result<HostSelectorCompletions, FactsError> {
        let mut injectors = Vec::new();
        for annotation in &sites.ownerless {
            let range = text_range(source, &annotation.range)?;
            if current_owner
                && self
                    .annotation_completion_position
                    .is_some_and(|position| position < range.start || range.end < position)
            {
                continue;
            }
            let ty = require_type(identities, &annotation.type_ref)?;
            let Some(kind) = MixinAnnotation::from_official_fqn(&ty.qualified_name)
                .filter(|kind| kind.is_injector())
            else {
                if current_owner && self.annotation_completion_position.is_some() {
                    return Err(FactsError::Unknown(
                        "current unowned annotation is not an official injector".into(),
                    ));
                }
                continue;
            };
            let body = mixin.body_range.as_ref().ok_or_else(|| {
                FactsError::Unknown("unowned selector has no authored Mixin body context".into())
            })?;
            if !contains(body, &annotation.range) {
                continue;
            }
            if annotation.use_site_target.is_some() {
                return Err(FactsError::Unknown(
                    "unowned injector use-site authority is unavailable".into(),
                ));
            }
            // Completion knows this annotation and its selector, not a method declaration.
            injectors.push(convert_injector(
                annotation, kind, None, identities, source,
            )?);
        }
        let coverage = match &sites.facts.coverage {
            Fact::Found { .. } => HostFact::Known(()),
            Fact::Unknown { reasons } => HostFact::Unavailable {
                reason: format!(
                    "annotation inventory coverage is unavailable: {}",
                    json!(reasons)
                ),
            },
            Fact::DefinitelyAbsent => {
                return Err(FactsError::Protocol(
                    "annotation coverage cannot certify absence".into(),
                ))
            }
        };
        Ok(HostSelectorCompletions {
            injectors,
            coverage,
        })
    }

    pub(super) fn require_local_annotation_completion(
        &self,
        selectors: Option<&HostSelectorCompletions>,
        current_owner: bool,
    ) -> Result<(), FactsError> {
        if current_owner
            && self.annotation_completion_position.is_some_and(|position| {
                selectors.is_some_and(|sites| {
                    sites.injectors.iter().any(|injector| {
                        injector.method_selectors.iter().any(|selector| {
                            selector.range.start <= position && position <= selector.range.end
                        })
                    })
                })
            })
        {
            return Ok(());
        }
        Err(FactsError::Unknown("annotation inventory coverage is unavailable for this operation or completion position".into()))
    }
}
