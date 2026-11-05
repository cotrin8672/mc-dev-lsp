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
    /// Recover only a current Java annotation completion when the host cannot
    /// produce a declaration header for the unfinished buffer.  The site,
    /// target identity, lexical owner and cursor context all remain required.
    pub(super) fn supports_partial_java_completion(
        &self,
        source: &str,
        imports: &JavaSourceImports,
        position: McTextPosition,
        identities: &BTreeMap<String, Fact<TypeFact>>,
        index: &dyn mcdev_core::model::ClassIndex,
    ) -> bool {
        if self.facts.coverage.found().is_some() || imports.is_kotlin {
            return false;
        }
        let Some(offset) = position_to_offset(source, position) else {
            return false;
        };
        let Some(context) = AnnotationContextExtractor::extract_at_offset(source, offset) else {
            return false;
        };
        if context.is_kotlin {
            return false;
        }
        let Some(scope) = AnnotationContextExtractor::resolve_mixin_class_scope(source, offset)
        else {
            return false;
        };
        if scope.raw_targets.len() != 1 {
            return false;
        }
        let lexical_mixins = annotations_with_imports(source, imports)
            .into_iter()
            .filter(|site| site.annotation == MixinAnnotation::Mixin)
            .collect::<Vec<_>>();
        if lexical_mixins.len() != 1 {
            return false;
        }
        let mixin_sites = self
            .ownerless
            .iter()
            .filter(|annotation| {
                identities
                    .get(&annotation.type_ref)
                    .and_then(Fact::found)
                    .is_some_and(|ty| {
                        MixinAnnotation::from_official_fqn(&ty.qualified_name)
                            == Some(MixinAnnotation::Mixin)
                    })
            })
            .collect::<Vec<_>>();
        let [mixin] = mixin_sites.as_slice() else {
            return false;
        };
        let lexical_mixin = &lexical_mixins[0];
        if mixin.range
            != (ByteRange {
                start: lexical_mixin.start,
                end: lexical_mixin.end,
            })
        {
            return false;
        }
        let Some(mixin_type) = identities.get(&mixin.type_ref).and_then(Fact::found) else {
            return false;
        };
        if mixin_type.read_only
            || mixin_type.context_source.is_some()
            || MixinAnnotation::from_official_fqn(&mixin_type.qualified_name)
                != Some(MixinAnnotation::Mixin)
        {
            return false;
        }
        let [argument] = mixin.arguments.as_slice() else {
            return false;
        };
        let Fact::Found {
            value: AnnotationValue::Class { type_ref },
        } = &argument.value
        else {
            return false;
        };
        let Some(target_type) = identities.get(type_ref).and_then(Fact::found) else {
            return false;
        };
        if target_type.read_only || target_type.context_source.is_some() {
            return false;
        }
        let Some(target_entry) = index.find_class_by_fqn(&target_type.qualified_name) else {
            return false;
        };
        let values = mcdev_core::mixin_source::annotation_values_with_imports(
            source,
            lexical_mixin,
            "value",
            imports,
        );
        let [value] = values.as_slice() else {
            return false;
        };
        if value.start != argument.range.start || value.end != argument.range.end {
            return false;
        }
        let raw = value
            .decoded
            .as_ref()
            .map_or(value.value.as_str(), |decoded| decoded.value.as_str())
            .trim_end_matches(".class")
            .trim_end_matches("::class");
        if scope.raw_targets[0] != raw {
            return false;
        }
        let Some(source_target) = MixinTargetResolver::resolve_target(raw, index, Some(imports))
        else {
            return false;
        };
        source_target == target_entry.internal_name
    }

    /// A recovered declaration can omit a complete annotation list while the
    /// host still supplies a positive, source-scoped `@Mixin` site.  Use that
    /// site only when one class header contains it; otherwise ownership stays
    /// unavailable rather than being inferred from spelling or coverage.
    pub(super) fn current_mixin_owner<'a>(
        &self,
        declarations: &'a [Declaration],
        identities: &BTreeMap<String, Fact<TypeFact>>,
    ) -> Option<(&'a Declaration, &Annotation)> {
        let mut owner = None;
        for annotation in &self.ownerless {
            let Some(identity) = identities.get(&annotation.type_ref).and_then(Fact::found) else {
                continue;
            };
            if MixinAnnotation::from_official_fqn(&identity.qualified_name)
                != Some(MixinAnnotation::Mixin)
            {
                continue;
            }
            let candidates = declarations
                .iter()
                .filter(|declaration| {
                    declaration.source_kind.as_deref() == Some("Class")
                        && declaration
                            .header_range
                            .as_ref()
                            .is_some_and(|header| contains(header, &annotation.range))
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 || owner.is_some() {
                return None;
            }
            owner = Some((candidates[0], annotation));
        }
        owner
    }

    pub(super) fn allow_source_kotlin_fallback(
        &self,
        source: &str,
        uri: &str,
        declarations: &[Declaration],
        detailed: bool,
        registry: &mut Registry,
        identities: &BTreeMap<String, Fact<TypeFact>>,
    ) -> bool {
        self.facts
            .validate_with_types_allow_unknown(
                source,
                uri,
                None,
                declarations,
                detailed,
                registry,
                identities,
            )
            .is_ok()
    }

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
        owned_selector: bool,
        current_owner: bool,
    ) -> Result<(), FactsError> {
        if current_owner
            && (owned_selector
                || self.annotation_completion_position.is_some_and(|position| {
                    selectors.is_some_and(|sites| {
                        sites.injectors.iter().any(|injector| {
                            injector.method_selectors.iter().any(|selector| {
                                selector.range.start <= position && position <= selector.range.end
                            })
                        })
                    })
                }))
        {
            return Ok(());
        }
        Err(FactsError::Unknown("annotation inventory coverage is unavailable for this operation or completion position".into()))
    }

    /// An authored member annotation is positive local evidence even when the
    /// flat inventory cannot certify every annotation in the recovered buffer.
    pub(super) fn owned_selector_at_position(
        &self,
        declarations: &[Declaration],
        owner: &Declaration,
        identities: &BTreeMap<String, Fact<TypeFact>>,
        source: &str,
    ) -> Result<bool, FactsError> {
        let Some(position) = self.annotation_completion_position else {
            return Ok(false);
        };
        let Some(position) = position_to_offset(source, position) else {
            return Ok(false);
        };
        for declaration in declarations
            .iter()
            .filter(|declaration| declaration.owner.as_deref() == Some(owner.id.as_str()))
        {
            for annotation in &declaration.annotations {
                if !(annotation.range.start <= position && position <= annotation.range.end) {
                    continue;
                }
                let ty = require_type(identities, &annotation.type_ref)?;
                let Some(_) = MixinAnnotation::from_official_fqn(&ty.qualified_name)
                    .filter(|kind| kind.is_injector())
                else {
                    continue;
                };
                let Some(argument) = annotation_argument(annotation, "method")? else {
                    continue;
                };
                let mut selectors = Vec::new();
                normalized_strings(argument, &mut selectors)?;
                if selectors
                    .iter()
                    .any(|(_, range)| range.start <= position && position <= range.end)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_facts::inputs::InputIdentity;
    use mcdev_core::mixin_source::AnnotationSlot;
    use mcdev_core::model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    const SOURCE: &str = "package com.example.mixin;\r\n\r\nimport com.example.target.SimpleTarget;\r\nimport org.spongepowered.asm.mixin.Mixin;\r\nimport org.spongepowered.asm.mixin.injection.At;\r\nimport org.spongepowered.asm.mixin.injection.Inject;\r\nimport org.spongepowered.asm.mixin.injection.callback.CallbackInfo;\r\n\r\n@Mixin(SimpleTarget.class)\r\npublic abstract class ExampleMixin {\r\n    @Inject(method = \"\r\n    private void mcdev$onDraw(CallbackInfo ci) {}\r\n}\r\n";
    const URI: &str = "file:///C:/Users/gummy/build/e2e-workspace/src/main/java/com/example/mixin/ExampleMixin.java";

    struct Index {
        target: bool,
    }

    impl ClassIndex for Index {
        fn find_classes(&self, _: &str, _: usize) -> Vec<ClassIndexEntry> {
            Vec::new()
        }

        fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry> {
            (self.target && internal_name == "com/example/target/SimpleTarget").then(|| {
                ClassIndexEntry {
                    internal_name: internal_name.into(),
                    simple_name: "SimpleTarget".into(),
                    package_name: "com.example.target".into(),
                }
            })
        }

        fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
            (self.target && fqn == "com.example.target.SimpleTarget").then(|| ClassIndexEntry {
                internal_name: "com/example/target/SimpleTarget".into(),
                simple_name: "SimpleTarget".into(),
                package_name: "com.example.target".into(),
            })
        }

        fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
            Vec::new()
        }

        fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
            Vec::new()
        }
    }

    fn found(value: Value) -> Value {
        json!({"state":"found","value":value})
    }

    fn span(text: &str, needle: &str) -> Value {
        let start = text.find(needle).unwrap();
        json!({"start":start,"end":start + needle.len()})
    }

    fn site() -> (AnnotationSites, Annotation) {
        let mixin = json!({
            "owner": null,
            "typeRef": "52:6",
            "useSiteTarget": null,
            "range": span(SOURCE, "@Mixin(SimpleTarget.class)"),
            "nameRange": {"start": SOURCE.find("Mixin").unwrap(), "end": SOURCE.find("Mixin").unwrap() + 5},
            "arguments": [{
                "name": null,
                "range": span(SOURCE, "SimpleTarget.class"),
                "value": found(json!({"kind":"class","typeRef":"52:7"}))
            }]
        });
        let sites = serde_json::from_value(json!({
            "source": {
                "documentUri": found(json!(URI)),
                "identity": {"kind":"source","revision":"660"},
                "sha256": format!("{:x}", Sha256::digest(SOURCE.as_bytes()))
            },
            "annotations": [mixin.clone()],
            "coverage": {"state":"unknown","reasons":[{"code":"UnsupportedSource"}]}
        }))
        .unwrap();
        (sites, serde_json::from_value(mixin).unwrap())
    }

    fn type_fact(id: &str, qualified_name: &str) -> Fact<TypeFact> {
        Fact::Found {
            value: TypeFact {
                id: id.into(),
                qualified_name: qualified_name.into(),
                package: qualified_name
                    .rsplit_once('.')
                    .map_or_else(String::new, |(p, _)| p.into()),
                read_only: false,
                context_source: None,
            },
        }
    }

    fn candidate() -> (SiteFacts, BTreeMap<String, Fact<TypeFact>>, McTextPosition) {
        let (facts, mixin) = site();
        let mut identities = BTreeMap::new();
        identities.insert(
            "52:6".into(),
            type_fact("mixin", "org.spongepowered.asm.mixin.Mixin"),
        );
        identities.insert(
            "52:7".into(),
            type_fact("target", "com.example.target.SimpleTarget"),
        );
        let position = offset_to_position(
            SOURCE,
            SOURCE.find("method = \"").unwrap() + "method = \"".len(),
        )
        .unwrap();
        (
            SiteFacts {
                facts,
                ownerless: vec![mixin],
            },
            identities,
            position,
        )
    }

    #[test]
    fn exact_fabric_partial_source_wire_requires_positive_site_target_index_and_cursor() {
        let at_source = SOURCE.replacen(
            "    @Inject(method = \"\r\n",
            "    @Inject(method = \"draw\", at = @At(target = \"\r\n",
            1,
        );
        let at_cursor = at_source.find("target = \"").unwrap() + "target = \"".len();
        let at_context = AnnotationContextExtractor::extract_at_offset(&at_source, at_cursor)
            .expect("core parser recognizes unfinished @At(target=...)");
        assert_eq!(at_context.annotation, MixinAnnotation::At);
        assert_eq!(at_context.slot, AnnotationSlot::Target);

        let (sites, identities, position) = candidate();
        let imports = JavaTypeDescriptorResolver::imports_for_uri(SOURCE, URI);
        assert!(sites.supports_partial_java_completion(
            SOURCE,
            &imports,
            position,
            &identities,
            &Index { target: true },
        ));

        let mut unresolved = identities.clone();
        unresolved.insert("52:7".into(), Fact::Unknown { reasons: vec![] });
        assert!(!sites.supports_partial_java_completion(
            SOURCE,
            &imports,
            position,
            &unresolved,
            &Index { target: true },
        ));

        assert!(!sites.supports_partial_java_completion(
            SOURCE,
            &imports,
            position,
            &identities,
            &Index { target: false },
        ));
        assert!(!sites.supports_partial_java_completion(
            SOURCE,
            &imports,
            McTextPosition {
                line: 0,
                character: 0
            },
            &identities,
            &Index { target: true },
        ));

        let (foreign_facts, foreign_mixin) = site();
        let mut foreign_identities = identities;
        foreign_identities.insert(
            "52:6".into(),
            Fact::Found {
                value: TypeFact {
                    id: "foreign-mixin".into(),
                    qualified_name: "org.spongepowered.asm.mixin.Mixin".into(),
                    package: "org.spongepowered.asm.mixin".into(),
                    read_only: true,
                    context_source: Some(SourceDescriptor {
                        id: "foreign-source".into(),
                        display_path: "/foreign/ExampleMixin.java".into(),
                        document_uri: Fact::Found {
                            value: "file:///foreign/ExampleMixin.java".into(),
                        },
                        language_id: "java".into(),
                        identity: InputIdentity {
                            kind: "source".into(),
                            revision: "foreign".into(),
                        },
                        document_version: None,
                        owner_scope: None,
                        memberships: None,
                        editable: None,
                    }),
                },
            },
        );
        assert!(!SiteFacts {
            facts: foreign_facts,
            ownerless: vec![foreign_mixin]
        }
        .supports_partial_java_completion(
            SOURCE,
            &imports,
            position,
            &foreign_identities,
            &Index { target: true },
        ));
    }
}
