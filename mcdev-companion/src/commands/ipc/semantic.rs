//! Convert facts from one live ktlsp evaluation into the existing MC semantic model.
//!
//! Values and opaque identities in this module are owned by the evaluation;
//! nothing here caches or re-resolves a source name as a JVM binary name.
mod annotation_sites;
mod parameters;
mod registered;
pub use registered::RegisteredSourceProvider;

use crate::{
    host_facts::{
        draft::EditingMember,
        inputs::{SourceDescriptor, SourceEntry, SourceFacts, SourceSnapshot},
        FactsError,
    },
    provider_commands::CommandFacts,
    wire::{
        Annotation, AnnotationValue, Argument, ByteRange, Declaration, Evaluation, Fact,
        HandleKind, IntegerType, Registry, TypeFact, MAX_BATCH,
    },
};
use mcdev_core::{
    descriptor::{self, JvmType},
    host_source::{
        project_host_semantic_document, HostAnnotationFact, HostAnswer, HostConstant,
        HostExpressionFact, HostExpressionKind, HostFact, HostGenerations, HostNodeValidity,
        HostPrimitive, HostReadSet, HostReadValidator, HostSemanticDocument, HostSemanticMember,
        HostSemanticModel, HostSemanticModelProvider, HostSemanticRequest, HostSourceError,
        HostSourceResult, HostTypeFact, SemanticAvailability,
    },
    mixin_source::{
        annotations_with_imports, parse_member_declaration, AnnotationContextExtractor,
        AtSelectorModel, InjectorKind, InjectorModel, JavaModifier, JavaSourceImports,
        JavaTypeDescriptorResolver, JavaTypeKind, MethodSelector, MixinAnnotation,
        MixinMemberAnnotationKind, MixinMemberDeclarationParser, MixinSemanticModelParser,
        MixinTargetRef, MixinTargetResolver, ParseConfidence, ParseSource, SemanticParseDebugInfo,
    },
    offset_to_position, position_to_offset, McTextPosition, McTextRange,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// The actor supplies an evaluation-scoped implementation. This seam also lets
/// tests supply host facts without substituting the MC semantic computation.
pub trait SemanticFactClient: Send + Sync {
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError>;
    fn check_active(&self) -> Result<(), FactsError>;
}

impl SemanticFactClient for CommandFacts {
    fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        self.request(
            "analysis/readFacts",
            json!({"evaluation": evaluation, "queries": queries}),
        )
    }
    fn check_active(&self) -> Result<(), FactsError> {
        CommandFacts::check_active(self)
    }
}

pub struct HostSemanticProvider {
    evaluation: Evaluation,
    facts: Arc<dyn SemanticFactClient>,
    document_uri: String,
    document_version: Option<i64>,
    index: Option<Arc<super::index::EvaluationIndex>>,
    class_index: Option<Arc<dyn mcdev_core::model::ClassIndex>>,
    source_facts_v2: bool,
    annotation_sites: bool,
    annotation_completion_position: Option<mcdev_core::McTextPosition>,
    project_sources: BTreeMap<std::path::PathBuf, (SourceEntry, SourceSnapshot)>,
}

impl HostSemanticProvider {
    pub fn new(
        evaluation: Evaluation,
        facts: CommandFacts,
        document_uri: String,
        document_version: Option<i64>,
    ) -> Result<Self, FactsError> {
        Self::with_client(evaluation, Arc::new(facts), document_uri, document_version)
    }

    pub fn with_client(
        evaluation: Evaluation,
        facts: Arc<dyn SemanticFactClient>,
        document_uri: String,
        document_version: Option<i64>,
    ) -> Result<Self, FactsError> {
        facts.check_active()?;
        if document_uri.is_empty() {
            return Err(FactsError::Protocol("missing current document URI".into()));
        }
        evaluation
            .validate(&mut Registry::default())
            .map_err(FactsError::Protocol)?;
        Ok(Self {
            evaluation,
            facts,
            document_uri,
            document_version,
            index: None,
            class_index: None,
            source_facts_v2: false,
            annotation_sites: false,
            annotation_completion_position: None,
            project_sources: BTreeMap::new(),
        })
    }

    pub fn with_index(mut self, index: Arc<super::index::EvaluationIndex>) -> Self {
        self.class_index = Some(index.clone());
        self.index = Some(index);
        self
    }

    pub fn with_source_facts(mut self, enabled: bool) -> Self {
        self.source_facts_v2 = enabled;
        self
    }

    /// A partial inventory can serve only the exact current completion position.
    /// Other commands retain typed unavailability instead of inferring absence.
    pub fn with_annotation_sites(
        mut self,
        enabled: bool,
        completion_position: Option<mcdev_core::McTextPosition>,
    ) -> Self {
        self.annotation_sites = enabled;
        self.annotation_completion_position = completion_position;
        self
    }

    pub fn with_project_sources(
        mut self,
        sources: Vec<(SourceEntry, SourceSnapshot)>,
    ) -> Result<Self, FactsError> {
        use sha2::{Digest, Sha256};
        for (entry, snapshot) in sources {
            if entry.descriptor != snapshot.descriptor
                || snapshot.sha256 != format!("{:x}", Sha256::digest(snapshot.text.as_bytes()))
            {
                return Err(FactsError::Protocol(
                    "project source snapshot identity or digest mismatch".into(),
                ));
            }
            let Some(uri) = snapshot.descriptor.document_uri.found() else {
                continue;
            };
            let path = mcdev_core::project::normalize_path(
                &mcdev_core::project::uri_to_path(uri).map_err(FactsError::Protocol)?,
            );
            if let Some((_, previous)) = self.project_sources.get(&path) {
                if previous.text != snapshot.text
                    || previous.descriptor.identity != snapshot.descriptor.identity
                {
                    return Err(FactsError::Stale(
                        "conflicting project source snapshots for one URI".into(),
                    ));
                }
            } else {
                self.project_sources.insert(path, (entry, snapshot));
            }
        }
        Ok(self)
    }

    fn read<T: DeserializeOwned>(&self, queries: &[Value]) -> Result<Vec<T>, FactsError> {
        let mut result = Vec::with_capacity(queries.len());
        for batch in queries.chunks(MAX_BATCH) {
            self.facts.check_active()?;
            let response = self.facts.read_facts(&self.evaluation.evaluation, batch)?;
            let facts = response
                .get("facts")
                .and_then(Value::as_array)
                .filter(|facts| facts.len() == batch.len())
                .ok_or_else(|| {
                    FactsError::Protocol("semantic fact response cardinality mismatch".into())
                })?;
            for fact in facts {
                result.push(
                    serde_json::from_value(fact.clone()).map_err(|e| {
                        FactsError::Protocol(format!("malformed semantic fact: {e}"))
                    })?,
                );
            }
        }
        Ok(result)
    }

    fn build(
        &self,
        source: &str,
        uri: &str,
        selected: Option<&SourceEntry>,
    ) -> Result<HostSemanticModel, FactsError> {
        let started = std::time::Instant::now();
        let detailed;
        let e = if self.source_facts_v2 {
            let mut query = json!({"op":"sourceFacts"});
            if let Some(source) = selected {
                query["source"] = json!(source.descriptor.id);
            }
            let facts: SourceFacts = self
                .read(&[query])?
                .pop()
                .ok_or_else(|| FactsError::Protocol("missing detailed source facts".into()))?;
            facts.validate(source, &mut Registry::default())?;
            match selected {
                Some(selected) => {
                    if facts.source.as_ref() != Some(&selected.descriptor) {
                        return Err(FactsError::Protocol(
                            "project sourceFacts returned a foreign snapshot".into(),
                        ));
                    }
                    if facts
                        .semantic_scope
                        .as_ref()
                        .and_then(Fact::found)
                        .is_none()
                    {
                        return Err(FactsError::Unknown(
                            "project source semantic scope is unavailable".into(),
                        ));
                    }
                }
                None if facts.source.is_some() => {
                    return Err(FactsError::Protocol(
                        "owner sourceFacts returned a foreign source context".into(),
                    ))
                }
                None => {}
            }
            detailed = Evaluation {
                evaluation: self.evaluation.evaluation.clone(),
                owner: self.evaluation.owner.clone(),
                source: source.to_owned(),
                configuration: self.evaluation.configuration.clone(),
                declarations: facts.declarations,
            };
            &detailed
        } else {
            &self.evaluation
        };
        let mut registry = Registry::default();
        e.validate(&mut registry).map_err(FactsError::Protocol)?;
        let mut sites = self.read_annotation_sites(source, uri, selected, &mut registry)?;
        let imports = JavaTypeDescriptorResolver::imports_for_uri(&e.source, uri);
        let all_annotations = e
            .declarations
            .iter()
            .flat_map(|declaration| {
                declaration.annotations.iter().chain(
                    declaration
                        .parameters
                        .as_ref()
                        .and_then(Fact::found)
                        .into_iter()
                        .flatten()
                        .flat_map(|parameter| &parameter.annotations),
                )
            })
            .chain(
                sites
                    .iter()
                    .flat_map(|sites| sites.facts.annotations.iter().map(|site| &site.annotation)),
            )
            .collect::<Vec<_>>();
        let supplied = all_annotations
            .iter()
            .map(|a| (a.range.start, a.range.end))
            .collect::<BTreeSet<_>>();
        let nested_arguments = all_annotations
            .iter()
            .flat_map(|annotation| {
                annotation
                    .arguments
                    .iter()
                    .filter(move |argument| contains(&annotation.range, &argument.range))
            })
            .map(|argument| (argument.range.start, argument.range.end))
            .collect::<Vec<_>>();
        // Nested annotation sites belong to an outer argument, not the declaration list.
        let missing_annotation = annotations_with_imports(&e.source, &imports)
            .iter()
            .any(|site| {
                !supplied.contains(&(site.start, site.end))
                    && !nested_arguments
                        .iter()
                        .any(|(start, end)| *start <= site.start && site.end <= *end)
            });
        if missing_annotation
            && sites
                .as_ref()
                .is_none_or(|sites| sites.facts.coverage.found().is_some())
        {
            return Err(FactsError::Unknown(
                "host annotation inventory is incomplete".into(),
            ));
        }
        let partial_annotations = missing_annotation
            || sites
                .as_ref()
                .is_some_and(|sites| sites.facts.coverage.found().is_none());

        let mut annotation_refs = BTreeSet::new();
        for annotation in e
            .declarations
            .iter()
            .flat_map(|declaration| &declaration.annotations)
        {
            collect_annotation_refs(annotation, &mut annotation_refs);
        }
        if self.source_facts_v2 {
            for parameter in e
                .declarations
                .iter()
                .filter_map(|declaration| declaration.parameters.as_ref().and_then(Fact::found))
                .flatten()
            {
                annotation_refs.insert(parameter.type_ref.clone());
                for annotation in &parameter.annotations {
                    collect_annotation_refs(annotation, &mut annotation_refs);
                }
            }
        }
        for site in sites.iter().flat_map(|sites| &sites.facts.annotations) {
            collect_annotation_refs(&site.annotation, &mut annotation_refs);
        }
        if sites.is_some() {
            for annotation in &all_annotations {
                for argument in &annotation.arguments {
                    annotation_sites::collect_value_refs(argument, &mut annotation_refs);
                }
            }
        }
        let source_context = selected.map(|entry| &entry.descriptor);
        let identities = self.resolve_refs(&annotation_refs, &mut registry, source_context)?;
        let annotation_sites_allow_fallback = self.source_facts_v2
            && selected.is_none()
            && imports.is_kotlin
            && !partial_annotations
            && sites.as_ref().is_none_or(|sites| {
                sites.allow_source_kotlin_fallback(
                    source,
                    uri,
                    &e.declarations,
                    self.source_facts_v2,
                    &mut registry,
                    &identities,
                )
            });
        if annotation_sites_allow_fallback
            && source_kotlin_syntax_can_use_host_index(
                source,
                uri,
                &imports,
                &e.declarations,
                &identities,
                partial_annotations,
                sites
                    .as_ref()
                    .map(|sites| sites.facts.annotations.as_slice()),
            )
        {
            return self.source_injector_model(source, uri, started);
        }
        if let Some(sites) = &mut sites {
            sites.reconcile(
                source,
                uri,
                &e.declarations,
                self.source_facts_v2,
                &mut registry,
                &identities,
            )?;
        }
        if self.source_facts_v2
            && selected.is_none()
            && !imports.is_kotlin
            && e.declarations.is_empty()
            && partial_annotations
            && self.annotation_completion_position.is_some()
        {
            if let (Some(sites), Some(index)) = (sites.as_ref(), self.class_index.as_deref()) {
                if sites.supports_partial_java_completion(
                    source,
                    &imports,
                    self.annotation_completion_position.unwrap(),
                    &identities,
                    index,
                ) {
                    return self.source_injector_model(source, uri, started);
                }
            }
        }
        let mut owners = Vec::new();
        for declaration in &e.declarations {
            for annotation in &declaration.annotations {
                let identity = require_type(&identities, &annotation.type_ref)?;
                if MixinAnnotation::from_official_fqn(&identity.qualified_name)
                    == Some(MixinAnnotation::Mixin)
                {
                    owners.push((declaration, annotation));
                }
            }
        }
        if owners.is_empty() && self.source_facts_v2 && selected.is_none() {
            if let Some(sites) = sites.as_ref() {
                if let Some(owner) = sites.current_mixin_owner(&e.declarations, &identities) {
                    owners.push(owner);
                }
            }
        }
        if owners.is_empty() && self.source_facts_v2 {
            if partial_annotations
                || sites.is_some() && self.annotation_completion_position.is_some()
            {
                return Err(FactsError::Unknown(
                    "annotation coverage does not establish a current Mixin owner".into(),
                ));
            }
            let model = mcdev_core::mixin_source::MixinClassModel {
                source_uri: uri.to_owned(),
                parse_source: ParseSource::HostNormalized,
                confidence: ParseConfidence::High,
                debug_info: mcdev_core::mixin_source::SemanticParseDebugInfo {
                    parse_source: ParseSource::HostNormalized,
                    ..Default::default()
                },
                normalized_handler_source: Some(parameters::source_facts(
                    source,
                    uri,
                    "",
                    &[],
                    HostFact::Known(Vec::new()),
                )),
                ..Default::default()
            };
            self.facts.check_active()?;
            return Ok(HostSemanticModel {
                model,
                availability: SemanticAvailability::Normalized,
                parse_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
        }
        if owners.len() != 1 {
            return Err(FactsError::Unknown(if owners.is_empty() {
                "host did not provide a unique Mixin owner or complete annotation coverage".into()
            } else {
                "multiple Mixin owners require a host-selected declaration".into()
            }));
        }
        let (owner, mixin) = owners[0];
        let selector_completions = sites
            .as_ref()
            .map(|sites| {
                self.project_annotation_selectors(
                    sites,
                    owner,
                    &identities,
                    source,
                    selected.is_none(),
                )
            })
            .transpose()?;
        let owned_selector = if partial_annotations && selected.is_none() {
            sites
                .as_ref()
                .map(|_| {
                    self.owned_selector_at_position(&e.declarations, owner, &identities, source)
                })
                .transpose()?
                .unwrap_or(false)
        } else {
            false
        };
        if partial_annotations {
            self.require_local_annotation_completion(
                selector_completions.as_ref(),
                owned_selector,
                selected.is_none(),
            )?;
        }
        let relevant = e
            .declarations
            .iter()
            .filter(|declaration| declaration.owner.as_deref() == Some(owner.id.as_str()))
            .filter(|declaration| {
                declaration.annotations.iter().any(|a| {
                    identities
                        .get(&a.type_ref)
                        .and_then(Fact::found)
                        .is_some_and(|ty| relevant_annotation(&ty.qualified_name))
                })
            })
            .collect::<Vec<_>>();
        let source_only_injectors = source_only_injectors(&relevant, &identities);
        let owner_fact: Fact<TypeFact> = self
            .read(&[json!({"op":"declarationType","declaration":owner.id})])?
            .pop()
            .ok_or_else(|| FactsError::Protocol("missing owner type".into()))?;
        let owner_type = require_fact(owner_fact, "Mixin owner type")?;
        validate_type(&owner_type, &mut registry)?;
        validate_source_context(
            owner_type.read_only,
            owner_type.context_source.as_ref(),
            source_context,
        )?;
        let mut class_refs = BTreeSet::new();
        for annotation in
            std::iter::once(mixin).chain(relevant.iter().flat_map(|d| d.annotations.iter()))
        {
            if relevant_annotation(&require_type(&identities, &annotation.type_ref)?.qualified_name)
            {
                for argument in &annotation.arguments {
                    collect_class_refs(argument, &mut class_refs)?;
                }
            }
        }
        let class_types = self.resolve_refs(&class_refs, &mut registry, source_context)?;
        let mut binary_types = BTreeMap::new();
        let mut type_queries = Vec::new();
        let mut queried_refs = Vec::new();
        for reference in &class_refs {
            let ty = require_type(&class_types, reference)?;
            type_queries.push(json!({"op":"typeJvm","target":ty.id}));
            queried_refs.push(reference);
        }
        let binary_facts: Vec<crate::host_facts::draft::TypeJvm> = self.read(&type_queries)?;
        for (reference, ty) in queried_refs.into_iter().zip(binary_facts) {
            ty.binary_name.validate().map_err(FactsError::Protocol)?;
            let name = ty
                .binary_name
                .found()
                .ok_or_else(|| FactsError::Unknown("JVM class identity is unavailable".into()))?;
            let descriptor = descriptor::parse_field_descriptor(&name.descriptor)
                .map_err(|error| FactsError::Protocol(error.to_string()))?;
            if descriptor != JvmType::ObjectType(name.internal_name.clone()) {
                return Err(FactsError::Protocol(
                    "host JVM name and descriptor disagree".into(),
                ));
            }
            if let Some(index) = self.index.as_ref().filter(|_| selected.is_none()) {
                match index.bind_host_type(&ty) {
                    Ok(())
                    | Err(
                        FactsError::Unknown(_)
                        | FactsError::Unsupported(_)
                        | FactsError::BytecodeUnavailable(_),
                    ) => {}
                    Err(error) => return Err(error),
                }
            }
            binary_types.insert(reference.clone(), name.internal_name.clone());
        }

        let source = &e.source;
        let type_annotation = convert_annotation(mixin, &identities, &binary_types, source)?;
        let mut targets = Vec::new();
        for argument in &mixin.arguments {
            if matches!(
                argument.name.as_deref(),
                None | Some("value" | "targets" | "target")
            ) {
                collect_targets(argument, source, &binary_types, &mut targets)?;
            }
        }
        let queries = relevant
            .iter()
            .map(|declaration| json!({"op":"declarationMember","declaration":declaration.id}))
            .collect::<Vec<_>>();
        let member_facts: Vec<Fact<EditingMember>> = self.read(&queries)?;
        let mut members = Vec::new();
        let mut expected_members = BTreeSet::new();
        let mut expected_injectors = Vec::new();
        let mut normalized_injectors = Vec::new();
        let mut handler_sites = HostFact::Known(Vec::new());
        let mut handler_slices = Vec::new();
        let mut mapped_names = BTreeMap::new();
        for (declaration, fact) in relevant.iter().zip(member_facts) {
            validate_source_injector_member(&fact, declaration, source_context, &mut registry)?;
            if self.source_facts_v2
                && selected.is_none()
                && source_only_injectors
                && source_injector_member_binding_incomplete(&fact)?
            {
                return self.source_injector_model(source, uri, started);
            }
            let item = require_fact(fact, "authored member")?;
            let member = &item.member;
            let signature = require_fact(member.signature.clone(), "authored member signature")?;
            registry
                .register(&signature.id, HandleKind::Signature)
                .map_err(FactsError::Protocol)?;
            if signature.kind != member.kind {
                return Err(FactsError::Protocol(
                    "authored member kind disagrees with signature".into(),
                ));
            }
            let dispatch = require_fact(member.dispatch.clone(), "authored member dispatch")?;
            let visibility = require_fact(item.visibility.clone(), "authored member visibility")?;
            let synthetic = require_fact(item.synthetic.clone(), "authored member synthetic flag")?;
            if synthetic || !item.mismatches.is_empty() {
                return Err(FactsError::Unknown(
                    "authored member binding is not exact".into(),
                ));
            }
            let (is_method, params, result) = signature_types(&member.kind, &signature.descriptor)?;
            let mut annotations = Vec::new();
            let mut selected = None;
            let mut mapped_name = None;
            for annotation in &declaration.annotations {
                let ty = require_type(&identities, &annotation.type_ref)?;
                if !relevant_annotation(&ty.qualified_name) {
                    continue;
                }
                if annotation
                    .use_site_target
                    .as_deref()
                    .is_some_and(|site| site != "field" || is_method)
                {
                    return Err(FactsError::Unknown(
                        "annotation use-site does not identify this authored member".into(),
                    ));
                }
                let converted = convert_annotation(annotation, &identities, &binary_types, source)?;
                if ty.qualified_name == "io.github.diskria.lapis.annotations.MappingName" {
                    if mapped_name.is_some() {
                        return Err(FactsError::Unknown(
                            "multiple mapping names require host selection".into(),
                        ));
                    }
                    mapped_name = Some(
                        converted
                            .member("value")
                            .and_then(HostExpressionFact::constant_string)
                            .ok_or_else(|| {
                                FactsError::Unknown("mapping name is unavailable".into())
                            })?,
                    );
                }
                if let Some(kind) = MixinAnnotation::from_official_fqn(&ty.qualified_name) {
                    if kind.is_injector() {
                        if !is_method {
                            return Err(FactsError::Unknown(
                                "injector declaration is not a method".into(),
                            ));
                        }
                        expected_injectors.push((annotation, kind, signature.descriptor.clone()));
                        if self.source_facts_v2 {
                            normalized_injectors.push(convert_injector(
                                annotation,
                                kind,
                                Some(&signature.descriptor),
                                &identities,
                                source,
                            )?);
                        }
                    }
                    if let Some(member_kind) = member_annotation(kind, is_method) {
                        if selected.replace((annotation, member_kind)).is_some() {
                            return Err(FactsError::Unknown(
                                "multiple member annotations require host selection".into(),
                            ));
                        }
                    }
                }
                annotations.push(converted);
            }
            let start = declaration
                .annotations
                .iter()
                .map(|a| a.range.start)
                .min()
                .unwrap_or(declaration.range.start);
            let after_annotation = declaration
                .annotations
                .iter()
                .map(|a| a.range.end)
                .max()
                .unwrap_or(declaration.range.start);
            let (member_start, member_end, mut modifiers) = if self.source_facts_v2 {
                let kind = declaration.source_kind.as_deref().unwrap_or("");
                if !matches!((kind, is_method), ("Function", true) | ("Property", false)) {
                    return Err(FactsError::Unknown(
                        "authored source kind and JVM member disagree".into(),
                    ));
                }
                let range = declaration.full_range.as_ref().ok_or_else(|| {
                    FactsError::Protocol("missing authored declaration range".into())
                })?;
                (
                    range.start,
                    range.end,
                    declaration_modifiers(declaration, selected.is_some())?,
                )
            } else {
                let header = parse_member_declaration(source, after_annotation, &imports)
                    .ok_or_else(|| {
                        FactsError::Unknown(
                            "authored declaration header range is unavailable".into(),
                        )
                    })?;
                if header.name_start != declaration.range.start
                    || header.name_end != declaration.range.end
                    || header.is_method != is_method
                    || header.name.trim_matches('`') != member.name
                {
                    return Err(FactsError::Unknown(
                        "authored declaration header does not match host identity".into(),
                    ));
                }
                (start, header.end, header.modifiers)
            };
            match dispatch.as_str() {
                "static" => {
                    modifiers.insert(JavaModifier::Static);
                }
                "instance" if !modifiers.contains(&JavaModifier::Static) => {}
                "instance" => {
                    return Err(FactsError::Unknown(
                        "source and host member dispatch disagree".into(),
                    ))
                }
                _ => return Err(FactsError::Protocol("invalid member dispatch".into())),
            }
            for modifier in [
                JavaModifier::Public,
                JavaModifier::Protected,
                JavaModifier::Private,
            ] {
                modifiers.remove(&modifier);
            }
            match visibility.to_ascii_lowercase().as_str() {
                "public" => {
                    modifiers.insert(JavaModifier::Public);
                }
                "protected" => {
                    modifiers.insert(JavaModifier::Protected);
                }
                "private" => {
                    modifiers.insert(JavaModifier::Private);
                }
                "package" | "packageprivate" | "package-private" | "internal" => {}
                _ => {
                    return Err(FactsError::Unknown(
                        "host member visibility is unavailable".into(),
                    ))
                }
            }
            if let Some((_, kind)) = selected {
                let key = (
                    member.name.clone(),
                    kind as u8,
                    text_range(source, &declaration.range)?.start,
                );
                if let Some(mapped_name) = mapped_name {
                    mapped_names.insert(key.clone(), mapped_name);
                }
                expected_members.insert(key);
            }
            if self.source_facts_v2
                && matches!(handler_sites, HostFact::Known(_))
                && declaration.annotations.iter().any(|annotation| {
                    identities
                        .get(&annotation.type_ref)
                        .and_then(Fact::found)
                        .and_then(|ty| MixinAnnotation::from_official_fqn(&ty.qualified_name))
                        .and_then(
                            mcdev_core::extras_model::MixinExtrasAnnotation::from_mixin_annotation,
                        )
                        .is_some_and(|kind| kind.is_sugar_handler_injector())
                })
            {
                match self.normalized_handler_sites(
                    declaration,
                    &params,
                    &result,
                    &dispatch,
                    &identities,
                    &binary_types,
                    &mut registry,
                    source_context,
                    source,
                ) {
                    Ok(sites) => {
                        if let HostFact::Known(all) = &mut handler_sites {
                            for (site, slice) in sites {
                                if let Some(slice) = slice {
                                    handler_slices.push(
                                        mcdev_core::extras_model::HostSliceSelection {
                                            site_index: all.len(),
                                            slice,
                                        },
                                    );
                                }
                                all.push(site);
                            }
                        }
                    }
                    Err(
                        FactsError::Unknown(reason)
                        | FactsError::Unsupported(reason)
                        | FactsError::BytecodeUnavailable(reason),
                    ) => {
                        handler_sites = HostFact::Unavailable { reason };
                    }
                    Err(error) => return Err(error),
                }
            }
            members.push(HostSemanticMember {
                name: member.name.clone(),
                annotations,
                parameter_types: params,
                return_type: result,
                modifiers,
                is_method,
                range: member_start..member_end,
                name_range: declaration.range.start..declaration.range.end,
            });
        }
        let document = HostSemanticDocument {
            package_name: owner_type.package,
            qualified_name: owner_type.qualified_name,
            type_kind: if self.source_facts_v2 {
                declaration_type_kind(owner)
            } else {
                JavaTypeKind::Unknown
            },
            type_annotations: vec![type_annotation],
            members,
        };
        let mut model = project_host_semantic_document(source, uri, &document);
        model.targets = targets;
        model.host_selector_completions = selector_completions;
        model.members.retain(|member| {
            expected_members.contains(&(
                member.java_name.clone(),
                member.annotation_kind as u8,
                member.name_range.start,
            ))
        });
        if model.members.len() != expected_members.len() {
            return Err(FactsError::Unknown(
                "normalized member projection is incomplete".into(),
            ));
        }
        for member in &mut model.members {
            let key = (
                member.java_name.clone(),
                member.annotation_kind as u8,
                member.name_range.start,
            );
            if let Some(name) = mapped_names.get(&key) {
                member.explicit_target_name = Some(name.clone());
            } else if member.explicit_target_name.is_none()
                && matches!(
                    member.annotation_kind,
                    MixinMemberAnnotationKind::Shadow | MixinMemberAnnotationKind::Overwrite
                )
            {
                member.explicit_target_name = Some(member.java_name.clone());
            }
        }
        // Lexical evidence may reject an omitted mapping, never resolve one.
        let lexical_members = if self.source_facts_v2 {
            Vec::new()
        } else {
            MixinMemberDeclarationParser::parse_members_with_imports(source, None, &imports)
        };
        for member in &model.members {
            let key = (
                member.java_name.clone(),
                member.annotation_kind as u8,
                member.name_range.start,
            );
            let lexical = lexical_members.iter().find(|candidate| {
                candidate.name_range == member.name_range
                    && candidate.annotation_kind == member.annotation_kind
            });
            if !mapped_names.contains_key(&key)
                && lexical.is_some_and(|candidate| {
                    candidate.explicit_target_name != member.explicit_target_name
                })
            {
                return Err(FactsError::Unknown(
                    "source and host member target disagree".into(),
                ));
            }
        }
        if self.source_facts_v2 {
            model.injectors = normalized_injectors;
            model.normalized_handler_source = Some(parameters::source_facts(
                source,
                uri,
                &model.qualified_name,
                &model
                    .targets
                    .iter()
                    .map(|target| target.internal_name.clone())
                    .collect::<Vec<_>>(),
                handler_sites,
            ));
            model
                .normalized_handler_source
                .as_mut()
                .unwrap()
                .slice_selections = handler_slices;
            model.parse_source = ParseSource::HostNormalized;
            model.confidence = ParseConfidence::High;
            model.debug_info.parse_source = ParseSource::HostNormalized;
            model.debug_info.fallback_reason = None;
            for member in &mut model.members {
                member.parse_source = ParseSource::HostNormalized;
                member.confidence = ParseConfidence::High;
            }
            if partial_annotations {
                model.confidence = ParseConfidence::Medium;
                model.warnings.push("Annotation coverage is unavailable; only the current positive selector completion is authorized".into());
                if let Some(handler) = &mut model.normalized_handler_source {
                    handler.sites = HostFact::Unavailable {
                        reason: "annotation inventory coverage is unavailable".into(),
                    };
                }
            }
            self.facts.check_active()?;
            return Ok(HostSemanticModel {
                model,
                availability: SemanticAvailability::Normalized,
                parse_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
        }
        model.injectors.retain(|injector| {
            expected_injectors.iter().any(|(annotation, kind, _)| {
                injector.annotation == *kind
                    && text_range(source, &annotation.range).ok() == Some(injector.range)
            })
        });
        if model.injectors.len() != expected_injectors.len() {
            return Err(FactsError::Unknown(
                "normalized injector projection is incomplete".into(),
            ));
        }
        for (annotation, kind, descriptor) in expected_injectors {
            let expected = string_values(annotation, "method")?;
            let actual = model
                .injectors
                .iter_mut()
                .find(|injector| {
                    injector.annotation == kind
                        && text_range(source, &annotation.range).ok() == Some(injector.range)
                })
                .ok_or_else(|| FactsError::Unknown("injector range is unavailable".into()))?;
            if actual
                .method_selectors
                .iter()
                .map(|selector| &selector.value)
                .ne(expected.iter())
                || !actual.at_selectors.is_empty()
            {
                return Err(FactsError::Unknown(
                    "injector selectors lack normalized host authority".into(),
                ));
            }
            actual.handler_method_descriptor = Some(descriptor);
        }
        // The wire lacks declaration coverage and full declaration ranges; the
        // existing source parser supplies these boundaries without proving them.
        model.parse_source = ParseSource::HandWrittenFallback;
        model.confidence = ParseConfidence::Low;
        model.debug_info.parse_source = ParseSource::HandWrittenFallback;
        model.debug_info.fallback_reason =
            Some("host declaration coverage and ranges are unavailable".into());
        model.warnings.push(
            "Host declaration coverage and ranges are unavailable; source fallback was used".into(),
        );
        for member in &mut model.members {
            member.parse_source = ParseSource::HandWrittenFallback;
            member.confidence = ParseConfidence::Low;
        }
        self.facts.check_active()?;
        Ok(HostSemanticModel {
            model,
            availability: SemanticAvailability::StableFallback,
            parse_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }

    fn resolve_refs(
        &self,
        refs: &BTreeSet<String>,
        registry: &mut Registry,
        source_context: Option<&SourceDescriptor>,
    ) -> Result<BTreeMap<String, Fact<TypeFact>>, FactsError> {
        let queries = refs
            .iter()
            .map(|reference| json!({"op":"resolveTypeRef","reference":reference}))
            .collect::<Vec<_>>();
        let values: Vec<Fact<TypeFact>> = self.read(&queries)?;
        let mut result = BTreeMap::new();
        for (reference, value) in refs.iter().zip(values) {
            value.validate().map_err(FactsError::Protocol)?;
            if let Some(ty) = value.found() {
                validate_type(ty, registry)?;
                validate_source_context(ty.read_only, ty.context_source.as_ref(), source_context)?;
            }
            result.insert(reference.clone(), value);
        }
        Ok(result)
    }

    fn source_injector_model(
        &self,
        source: &str,
        uri: &str,
        started: std::time::Instant,
    ) -> Result<HostSemanticModel, FactsError> {
        let index = self
            .class_index
            .as_deref()
            .ok_or_else(|| FactsError::Unknown("Mixin target index is unavailable".into()))?;
        let imports = JavaTypeDescriptorResolver::imports_for_uri(source, uri);
        let mut model = MixinSemanticModelParser::parse_with_lookup(source, uri, Some(index));
        if model.targets.is_empty() {
            return Err(FactsError::Unknown(
                "Mixin target syntax is unavailable".into(),
            ));
        }
        model.targets = model
            .targets
            .into_iter()
            .map(|target| {
                let internal_name = MixinTargetResolver::resolve_target(
                    &target.internal_name,
                    index,
                    Some(&imports),
                )
                .ok_or_else(|| {
                    FactsError::Unknown(
                        "Mixin target is unresolved or ambiguous in the selected project".into(),
                    )
                })?;
                Ok(MixinTargetRef {
                    internal_name,
                    range: target.range,
                })
            })
            .collect::<Result<Vec<_>, FactsError>>()?;
        let reason = "source syntax model used; handler JVM declaration facts are not required";
        model.parse_source = ParseSource::HandWrittenFallback;
        model.confidence = ParseConfidence::Low;
        model.warnings.push(reason.into());
        model.debug_info = SemanticParseDebugInfo {
            parse_source: ParseSource::HandWrittenFallback,
            fallback_reason: Some(reason.into()),
            ..Default::default()
        };
        for member in &mut model.members {
            member.parse_source = ParseSource::HandWrittenFallback;
            member.confidence = ParseConfidence::Low;
        }
        self.facts.check_active()?;
        Ok(HostSemanticModel {
            model,
            availability: SemanticAvailability::StableFallback,
            parse_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

fn validate_source_context(
    read_only: bool,
    context: Option<&SourceDescriptor>,
    expected: Option<&SourceDescriptor>,
) -> Result<(), FactsError> {
    match (read_only, context, expected) {
        (false, None, None) => Ok(()),
        (true, Some(context), Some(expected))
            if context.identity == expected.identity
                && context.document_uri == expected.document_uri
                && context.language_id == expected.language_id =>
        {
            Ok(())
        }
        _ => Err(FactsError::Protocol(
            "semantic identity has a foreign source context".into(),
        )),
    }
}

impl HostReadValidator for HostSemanticProvider {
    fn validate_current_reads(
        &self,
        _: HostGenerations,
        _: &HostReadSet,
    ) -> Result<bool, HostSourceError> {
        self.facts.check_active().map_err(FactsError::core)?;
        Ok(false) // The host has no cross-evaluation generation identity.
    }
}
impl HostSemanticModelProvider for HostSemanticProvider {
    fn parse(&self, request: &HostSemanticRequest<'_>) -> HostSourceResult<HostSemanticModel> {
        self.facts.check_active().map_err(FactsError::core)?;
        if request.project.is_some() {
            return Ok(HostAnswer::unavailable(
                "cross-evaluation semantic caching is unavailable",
            ));
        }
        let selected;
        let same_owner_uri = request.source_uri == self.document_uri
            || mcdev_core::project::uri_to_path(request.source_uri)
                .ok()
                .zip(mcdev_core::project::uri_to_path(&self.document_uri).ok())
                .is_some_and(|(request, owner)| {
                    mcdev_core::project::normalize_path(&request)
                        == mcdev_core::project::normalize_path(&owner)
                });
        if same_owner_uri {
            if request.source != self.evaluation.source
                || request.document_version.is_some()
                    && request.document_version != self.document_version
            {
                return Ok(HostAnswer::unavailable(
                    "semantic source, URI or version is stale",
                ));
            }
            selected = None;
        } else {
            let context = mcdev_core::project::uri_to_path(request.source_uri)
                .ok()
                .map(|path| mcdev_core::project::normalize_path(&path))
                .and_then(|path| self.project_sources.get(&path));
            let Some((entry, snapshot)) = context.filter(|_| self.source_facts_v2) else {
                return Ok(HostAnswer::unavailable(
                    "semantic source, URI or version is stale",
                ));
            };
            if request.source != snapshot.text
                || request.document_version.is_some()
                    && request.document_version != snapshot.descriptor.document_version
            {
                return Ok(HostAnswer::unavailable(
                    "project semantic source or editor version is stale or unavailable",
                ));
            }
            selected = Some(entry);
        }
        match self.build(request.source, request.source_uri, selected) {
            Ok(model) => Ok(HostAnswer::known(model, vec![])),
            Err(FactsError::Cancelled(message)) => Err(HostSourceError::Cancelled(message)),
            Err(error) => Ok(HostAnswer::unavailable(error.to_string())),
        }
    }
}

fn relevant_annotation(fqn: &str) -> bool {
    MixinAnnotation::from_official_fqn(fqn).is_some()
        || fqn.starts_with("com.llamalad7.mixinextras.")
        || fqn == "io.github.diskria.lapis.annotations.MappingName"
}

fn source_kotlin_syntax_can_use_host_index(
    source: &str,
    uri: &str,
    imports: &JavaSourceImports,
    declarations: &[Declaration],
    identities: &BTreeMap<String, Fact<TypeFact>>,
    partial_annotations: bool,
    annotation_sites: Option<&[crate::host_facts::inputs::AnnotationSite]>,
) -> bool {
    if !imports.is_kotlin || partial_annotations {
        return false;
    }
    let parsed = MixinSemanticModelParser::parse(source, uri);
    if parsed.targets.is_empty() || parsed.injectors.is_empty() || !parsed.members.is_empty() {
        return false;
    }
    let sites = annotations_with_imports(source, imports);
    let mixins = sites
        .iter()
        .filter(|site| site.annotation == MixinAnnotation::Mixin)
        .collect::<Vec<_>>();
    let injectors = sites
        .iter()
        .filter(|site| site.annotation.is_injector())
        .collect::<Vec<_>>();
    if mixins.len() != 1 || injectors.is_empty() {
        return false;
    }
    let mixin = mixins[0];
    let Some(owner) = declarations.iter().find(|declaration| {
        declaration.owner.is_none()
            && declaration.full_range.as_ref().is_some_and(|range| {
                contains(
                    range,
                    &ByteRange {
                        start: mixin.start,
                        end: mixin.end,
                    },
                )
            })
    }) else {
        return false;
    };
    let injector_ranges = injectors
        .iter()
        .map(|site| (site.start, site.end))
        .collect::<BTreeSet<_>>();
    let mut matched_injectors = BTreeSet::new();
    let mut owner_mixin_count = 0;
    let mut incomplete_type = false;
    let mut incomplete_arguments = false;
    if !kotlin_annotation_types_allow_fallback(
        &sites,
        declarations,
        identities,
        annotation_sites,
        &mut incomplete_type,
    ) {
        return false;
    }
    for declaration in declarations.iter().filter(|declaration| {
        declaration.id == owner.id || declaration.owner.as_deref() == Some(owner.id.as_str())
    }) {
        for annotation in &declaration.annotations {
            let Some(site) = sites.iter().find(|site| {
                site.start == annotation.range.start && site.end == annotation.range.end
            }) else {
                return false;
            };
            if declaration.id == owner.id {
                if site.annotation != MixinAnnotation::Mixin {
                    return false;
                }
                owner_mixin_count += 1;
            } else {
                if !site.annotation.is_injector() {
                    return false;
                }
                matched_injectors.insert((site.start, site.end));
            }
            match identities.get(&annotation.type_ref) {
                Some(Fact::Found { value }) => {
                    if MixinAnnotation::from_official_fqn(&value.qualified_name)
                        != Some(site.annotation)
                    {
                        return false;
                    }
                }
                Some(Fact::Unknown { .. } | Fact::DefinitelyAbsent) => {
                    incomplete_type = true;
                }
                None => return false,
            }
            incomplete_arguments |= annotation_arguments_unavailable(annotation);
        }
    }
    owner_mixin_count == 1
        && matched_injectors == injector_ranges
        && (incomplete_type || incomplete_arguments)
}

fn kotlin_annotation_types_allow_fallback(
    source_sites: &[mcdev_core::mixin_source::AnnotationSite],
    declarations: &[Declaration],
    identities: &BTreeMap<String, Fact<TypeFact>>,
    annotation_sites: Option<&[crate::host_facts::inputs::AnnotationSite]>,
    incomplete_type: &mut bool,
) -> bool {
    for declaration in declarations {
        for annotation in &declaration.annotations {
            if !kotlin_annotation_type_binding_matches(
                annotation,
                source_sites,
                identities,
                incomplete_type,
            ) {
                return false;
            }
        }
        for parameter in declaration
            .parameters
            .as_ref()
            .and_then(Fact::found)
            .into_iter()
            .flatten()
        {
            for annotation in &parameter.annotations {
                if !kotlin_annotation_type_binding_matches(
                    annotation,
                    source_sites,
                    identities,
                    incomplete_type,
                ) {
                    return false;
                }
            }
        }
    }
    if let Some(annotation_sites) = annotation_sites {
        for site in annotation_sites {
            if !kotlin_annotation_type_binding_matches(
                &site.annotation,
                source_sites,
                identities,
                incomplete_type,
            ) {
                return false;
            }
        }
    }
    true
}

fn kotlin_annotation_type_binding_matches(
    annotation: &Annotation,
    source_sites: &[mcdev_core::mixin_source::AnnotationSite],
    identities: &BTreeMap<String, Fact<TypeFact>>,
    incomplete_type: &mut bool,
) -> bool {
    if let Some(site) = source_sites
        .iter()
        .find(|site| site.start == annotation.range.start && site.end == annotation.range.end)
    {
        match identities.get(&annotation.type_ref) {
            Some(Fact::Found { value }) => {
                if MixinAnnotation::from_official_fqn(&value.qualified_name)
                    != Some(site.annotation)
                {
                    return false;
                }
            }
            Some(Fact::Unknown { .. } | Fact::DefinitelyAbsent) => *incomplete_type = true,
            None => return false,
        }
    }
    annotation.arguments.iter().all(|argument| {
        kotlin_annotation_argument_type_binding_matches(
            argument,
            source_sites,
            identities,
            incomplete_type,
        )
    })
}

fn kotlin_annotation_argument_type_binding_matches(
    argument: &Argument,
    source_sites: &[mcdev_core::mixin_source::AnnotationSite],
    identities: &BTreeMap<String, Fact<TypeFact>>,
    incomplete_type: &mut bool,
) -> bool {
    match argument.value.found() {
        Some(AnnotationValue::Annotation { annotation }) => kotlin_annotation_type_binding_matches(
            annotation,
            source_sites,
            identities,
            incomplete_type,
        ),
        Some(AnnotationValue::Array { values }) => values.iter().all(|value| {
            kotlin_annotation_argument_type_binding_matches(
                value,
                source_sites,
                identities,
                incomplete_type,
            )
        }),
        _ => true,
    }
}

fn source_only_injectors(
    declarations: &[&Declaration],
    identities: &BTreeMap<String, Fact<TypeFact>>,
) -> bool {
    !declarations.is_empty()
        && declarations.iter().all(|declaration| {
            declaration.annotations.iter().all(|annotation| {
                identities
                    .get(&annotation.type_ref)
                    .and_then(Fact::found)
                    .and_then(|ty| MixinAnnotation::from_official_fqn(&ty.qualified_name))
                    .is_some_and(MixinAnnotation::is_injector)
            })
        })
}

fn annotation_arguments_unavailable(annotation: &Annotation) -> bool {
    fn value_unavailable(argument: &Argument) -> bool {
        match &argument.value {
            Fact::Found {
                value: AnnotationValue::Array { values },
            } => values.iter().any(value_unavailable),
            Fact::Found {
                value: AnnotationValue::Annotation { annotation },
            } => annotation_arguments_unavailable(annotation),
            Fact::Found { .. } => false,
            Fact::Unknown { .. } | Fact::DefinitelyAbsent => true,
        }
    }
    annotation.arguments.iter().any(value_unavailable)
}

fn source_injector_member_binding_incomplete(
    fact: &Fact<EditingMember>,
) -> Result<bool, FactsError> {
    fact.validate().map_err(FactsError::Protocol)?;
    let Fact::Found { value } = fact else {
        return Ok(true);
    };
    if !value.mismatches.is_empty() {
        return Ok(false);
    }
    Ok(value.member.signature.found().is_none()
        || value.member.dispatch.found().is_none()
        || value.visibility.found().is_none()
        || value.synthetic.found().is_none())
}

fn validate_source_injector_member(
    fact: &Fact<EditingMember>,
    declaration: &Declaration,
    source_context: Option<&SourceDescriptor>,
    registry: &mut Registry,
) -> Result<(), FactsError> {
    fact.validate().map_err(FactsError::Protocol)?;
    let Fact::Found { value } = fact else {
        return Ok(());
    };
    let member = &value.member;
    validate_source_context(
        member.read_only,
        member.context_source.as_ref(),
        source_context,
    )?;
    if member.name != declaration.name.as_deref().unwrap_or("").trim_matches('`') {
        return Err(FactsError::Protocol(
            "authored member name disagrees with declaration".into(),
        ));
    }
    registry
        .register(&member.id, HandleKind::Member)
        .map_err(FactsError::Protocol)?;
    member.signature.validate().map_err(FactsError::Protocol)?;
    if let Some(signature) = member.signature.found() {
        registry
            .register(&signature.id, HandleKind::Signature)
            .map_err(FactsError::Protocol)?;
        if signature.kind != member.kind {
            return Err(FactsError::Protocol(
                "editing member signature kind mismatch".into(),
            ));
        }
        match member.kind.as_str() {
            "method" => {
                descriptor::parse_method_descriptor(&signature.descriptor)
                    .map_err(|error| FactsError::Protocol(error.to_string()))?;
            }
            "field" => {
                descriptor::parse_field_descriptor(&signature.descriptor)
                    .map_err(|error| FactsError::Protocol(error.to_string()))?;
            }
            _ => return Err(FactsError::Protocol("unknown editing member kind".into())),
        }
    } else if !matches!(member.kind.as_str(), "method" | "field") {
        return Err(FactsError::Protocol("unknown editing member kind".into()));
    }
    member.dispatch.validate().map_err(FactsError::Protocol)?;
    if member
        .dispatch
        .found()
        .is_some_and(|value| value != "instance" && value != "static")
    {
        return Err(FactsError::Protocol("unknown dispatch value".into()));
    }
    value.visibility.validate().map_err(FactsError::Protocol)?;
    value.synthetic.validate().map_err(FactsError::Protocol)?;
    Ok(())
}

fn validate_type(ty: &TypeFact, registry: &mut Registry) -> Result<(), FactsError> {
    if ty.qualified_name.is_empty() {
        return Err(FactsError::Protocol("empty host type name".into()));
    }
    registry
        .register(&ty.id, HandleKind::Type)
        .map_err(FactsError::Protocol)
}
fn require_type<'a>(
    types: &'a BTreeMap<String, Fact<TypeFact>>,
    reference: &str,
) -> Result<&'a TypeFact, FactsError> {
    let fact = types
        .get(reference)
        .ok_or_else(|| FactsError::Protocol("unknown type reference handle".into()))?;
    fact.found()
        .ok_or_else(|| FactsError::Unknown("annotation or class type is unavailable".into()))
}
fn require_fact<T>(fact: Fact<T>, description: &str) -> Result<T, FactsError> {
    fact.validate().map_err(FactsError::Protocol)?;
    match fact {
        Fact::Found { value } => Ok(value),
        Fact::Unknown { .. } | Fact::DefinitelyAbsent => {
            Err(FactsError::Unknown(format!("{description} is unavailable")))
        }
    }
}
fn text_range(source: &str, range: &ByteRange) -> Result<McTextRange, FactsError> {
    range.text(source).map_err(FactsError::Protocol)?;
    Ok(McTextRange {
        start: offset_to_position(source, range.start)
            .ok_or_else(|| FactsError::Protocol("invalid UTF-8 range start".into()))?,
        end: offset_to_position(source, range.end)
            .ok_or_else(|| FactsError::Protocol("invalid UTF-8 range end".into()))?,
    })
}
fn collect_class_refs(
    argument: &Argument,
    output: &mut BTreeSet<String>,
) -> Result<(), FactsError> {
    argument.value.validate().map_err(FactsError::Protocol)?;
    match &argument.value {
        Fact::Found {
            value: AnnotationValue::Class { type_ref } | AnnotationValue::Enum { type_ref, .. },
        } => {
            output.insert(type_ref.clone());
        }
        Fact::Found {
            value: AnnotationValue::Array { values },
        } => {
            for value in values {
                collect_class_refs(value, output)?;
            }
        }
        Fact::Found {
            value: AnnotationValue::Annotation { annotation },
        } => {
            for argument in &annotation.arguments {
                collect_class_refs(argument, output)?;
            }
        }
        Fact::Found { .. } => {}
        _ => {
            return Err(FactsError::Unknown(
                "annotation argument is unavailable".into(),
            ))
        }
    }
    Ok(())
}

fn collect_annotation_refs(annotation: &Annotation, refs: &mut BTreeSet<String>) {
    fn visit(argument: &Argument, refs: &mut BTreeSet<String>) {
        match argument.value.found() {
            Some(AnnotationValue::Annotation { annotation }) => {
                collect_annotation_refs(annotation, refs)
            }
            Some(AnnotationValue::Array { values }) => {
                for value in values {
                    visit(value, refs);
                }
            }
            _ => {}
        }
    }
    refs.insert(annotation.type_ref.clone());
    for argument in &annotation.arguments {
        visit(argument, refs);
    }
}

fn declaration_modifiers(
    declaration: &Declaration,
    complete: bool,
) -> Result<BTreeSet<JavaModifier>, FactsError> {
    let facts = declaration
        .modifiers
        .as_ref()
        .ok_or_else(|| FactsError::Protocol("missing authored declaration modifiers".into()))?;
    let mut modifiers = BTreeSet::new();
    for (fact, modifier) in [
        (&facts.is_static, JavaModifier::Static),
        (&facts.is_final, JavaModifier::Final),
        (&facts.is_abstract, JavaModifier::Abstract),
    ] {
        // Handler signatures do not consume implicit finality or abstractness.
        if !complete && fact.found().is_none() {
            continue;
        }
        if require_fact(fact.clone(), "authored declaration modifier")? {
            modifiers.insert(modifier);
        }
    }
    let visibility = require_fact(facts.visibility.clone(), "authored declaration visibility")?;
    match visibility.to_ascii_lowercase().as_str() {
        "public" => {
            modifiers.insert(JavaModifier::Public);
        }
        "protected" => {
            modifiers.insert(JavaModifier::Protected);
        }
        "private" => {
            modifiers.insert(JavaModifier::Private);
        }
        "package" | "packageprivate" | "package-private" | "internal" => {}
        _ => return Err(FactsError::Protocol("invalid authored visibility".into())),
    }
    Ok(modifiers)
}

fn declaration_type_kind(declaration: &Declaration) -> JavaTypeKind {
    match declaration.source_kind.as_deref() {
        Some("Class" | "Object") => JavaTypeKind::Class,
        Some("Interface") => JavaTypeKind::Interface,
        Some("Enum" | "EnumClass") => JavaTypeKind::Enum,
        Some("Annotation") => JavaTypeKind::Annotation,
        Some("Record") => JavaTypeKind::Record,
        _ => JavaTypeKind::Unknown,
    }
}

fn annotation_argument<'a>(
    annotation: &'a Annotation,
    name: &str,
) -> Result<Option<&'a Argument>, FactsError> {
    let mut matching = annotation
        .arguments
        .iter()
        .filter(|argument| argument.name.as_deref().unwrap_or("value") == name);
    let result = matching.next();
    if matching.next().is_some() {
        return Err(FactsError::Unknown(
            "duplicate annotation attributes".into(),
        ));
    }
    Ok(result)
}

fn normalized_strings<'a>(
    argument: &'a Argument,
    output: &mut Vec<(&'a str, &'a ByteRange)>,
) -> Result<(), FactsError> {
    match argument.value.found() {
        Some(AnnotationValue::String {
            value,
            content_range,
        }) => output.push((value, content_range)),
        Some(AnnotationValue::MappedString { value, .. }) => output.push((value, &argument.range)),
        Some(AnnotationValue::Array { values }) => {
            for value in values {
                normalized_strings(value, output)?;
            }
        }
        _ => {
            return Err(FactsError::Unknown(
                "normalized string annotation value is unavailable".into(),
            ))
        }
    }
    Ok(())
}

fn convert_injector(
    annotation: &Annotation,
    kind: MixinAnnotation,
    descriptor: Option<&str>,
    identities: &BTreeMap<String, Fact<TypeFact>>,
    source: &str,
) -> Result<InjectorModel, FactsError> {
    fn at_selectors(
        argument: &Argument,
        identities: &BTreeMap<String, Fact<TypeFact>>,
        source: &str,
        output: &mut Vec<AtSelectorModel>,
    ) -> Result<(), FactsError> {
        match argument.value.found() {
            Some(AnnotationValue::Array { values }) => {
                for value in values {
                    at_selectors(value, identities, source, output)?;
                }
            }
            Some(AnnotationValue::Annotation { annotation }) => {
                if require_type(identities, &annotation.type_ref)?.qualified_name
                    == "org.spongepowered.asm.mixin.injection.Slice"
                {
                    return Ok(());
                }
                if require_type(identities, &annotation.type_ref)?.qualified_name
                    == "org.spongepowered.asm.mixin.injection.At"
                {
                    let value_arg = annotation_argument(annotation, "value")?
                        .ok_or_else(|| FactsError::Unknown("At value is unavailable".into()))?;
                    let mut values = Vec::new();
                    normalized_strings(value_arg, &mut values)?;
                    let [(value, _)] = values.as_slice() else {
                        return Err(FactsError::Unknown("At requires one value".into()));
                    };
                    let mut target = None;
                    let mut target_range = None;
                    if let Some(argument) = annotation_argument(annotation, "target")? {
                        let mut targets = Vec::new();
                        normalized_strings(argument, &mut targets)?;
                        let [(value, range)] = targets.as_slice() else {
                            return Err(FactsError::Unknown("At requires one target".into()));
                        };
                        target = Some((*value).to_owned());
                        target_range = Some(text_range(source, range)?);
                    }
                    let ordinal_arg = annotation_argument(annotation, "ordinal")?;
                    let ordinal = ordinal_arg
                        .map(|argument| match argument.value.found() {
                            Some(AnnotationValue::Integer {
                                integer_type: IntegerType::Int,
                                value,
                            }) => i32::try_from(*value)
                                .map_err(|_| FactsError::Protocol("At ordinal exceeds Int".into())),
                            _ => Err(FactsError::Unknown("At ordinal is unavailable".into())),
                        })
                        .transpose()?;
                    output.push(AtSelectorModel {
                        value: (*value).to_owned(),
                        target,
                        ordinal,
                        range: text_range(source, &annotation.range)?,
                        target_range,
                        ordinal_range: ordinal_arg
                            .map(|argument| text_range(source, &argument.range))
                            .transpose()?,
                    });
                }
                for argument in &annotation.arguments {
                    at_selectors(argument, identities, source, output)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut selectors = Vec::new();
    if let Some(argument) = annotation_argument(annotation, "method")? {
        normalized_strings(argument, &mut selectors)?;
    }
    let method_selectors = selectors
        .into_iter()
        .map(|(value, range)| {
            let (name, descriptor) = value.split_once('(').map_or_else(
                || (value.to_owned(), None),
                |(name, rest)| (name.to_owned(), Some(format!("({rest}"))),
            );
            Ok(MethodSelector {
                value: value.into(),
                name,
                descriptor,
                range: text_range(source, range)?,
            })
        })
        .collect::<Result<Vec<_>, FactsError>>()?;
    let mut ats = Vec::new();
    for argument in &annotation.arguments {
        at_selectors(argument, identities, source, &mut ats)?;
    }
    Ok(InjectorModel {
        annotation: kind,
        annotation_kind: InjectorKind::from_annotation(kind)
            .ok_or_else(|| FactsError::Protocol("invalid injector annotation kind".into()))?,
        method_selectors,
        at_selectors: ats,
        handler_method_descriptor: descriptor.map(str::to_owned),
        range: text_range(source, &annotation.range)?,
    })
}
fn convert_annotation(
    annotation: &Annotation,
    types: &BTreeMap<String, Fact<TypeFact>>,
    binaries: &BTreeMap<String, String>,
    source: &str,
) -> Result<HostAnnotationFact, FactsError> {
    let ty = require_type(types, &annotation.type_ref)?;
    let raw = annotation
        .range
        .text(source)
        .map_err(FactsError::Protocol)?;
    let body_range = raw.find('(').and_then(|start| {
        raw.rfind(')')
            .filter(|end| *end > start)
            .map(|end| (annotation.range.start + start + 1)..(annotation.range.start + end))
    });
    let mut members = Vec::new();
    for argument in &annotation.arguments {
        if !contains(&annotation.range, &argument.range) {
            return Err(FactsError::Protocol(
                "annotation argument is outside its annotation".into(),
            ));
        }
        members.push((
            argument.name.clone().unwrap_or_else(|| "value".into()),
            convert_expression(argument, types, binaries, source)?,
        ));
    }
    Ok(HostAnnotationFact {
        source_name: ty.qualified_name.rsplit('.').next().unwrap_or("").into(),
        qualified_name: Some(ty.qualified_name.clone()),
        validity: HostNodeValidity::default(),
        exists: true,
        members,
        members_available: true,
        source_range: Some(annotation.range.start..annotation.range.end),
        body_range,
    })
}
fn convert_expression(
    argument: &Argument,
    types: &BTreeMap<String, Fact<TypeFact>>,
    binaries: &BTreeMap<String, String>,
    source: &str,
) -> Result<HostExpressionFact, FactsError> {
    let text = argument.range.text(source).map_err(FactsError::Protocol)?;
    let kind = match &argument.value {
        Fact::Found {
            value:
                AnnotationValue::String {
                    value,
                    content_range,
                },
        } => {
            content_range.text(source).map_err(FactsError::Protocol)?;
            if content_range.start < argument.range.start || content_range.end > argument.range.end
            {
                return Err(FactsError::Protocol(
                    "string content range is outside its argument".into(),
                ));
            }
            HostExpressionKind::Constant(HostConstant::String(value.encode_utf16().collect()))
        }
        Fact::Found {
            value: AnnotationValue::Boolean { value },
        } => HostExpressionKind::Constant(HostConstant::Boolean(*value)),
        Fact::Found {
            value: AnnotationValue::MappedString { value, .. },
        } => HostExpressionKind::Constant(HostConstant::String(value.encode_utf16().collect())),
        Fact::Found {
            value:
                AnnotationValue::Integer {
                    integer_type,
                    value,
                },
        } => HostExpressionKind::Constant(match integer_type {
            IntegerType::Int => HostConstant::Int(i32::try_from(*value).map_err(|_| {
                FactsError::Protocol("integer annotation value exceeds Int".into())
            })?),
            IntegerType::Long => HostConstant::Long(*value),
        }),
        Fact::Found {
            value: AnnotationValue::Enum { type_ref, name },
        } => HostExpressionKind::Enum {
            owner: HostTypeFact::Class {
                internal_name: binaries.get(type_ref).cloned().ok_or_else(|| {
                    FactsError::Unknown("enum JVM identity is unavailable".into())
                })?,
            },
            name: name.clone(),
        },
        Fact::Found {
            value: AnnotationValue::Annotation { annotation },
        } => HostExpressionKind::Annotation(Box::new(convert_annotation(
            annotation, types, binaries, source,
        )?)),
        Fact::Found {
            value: AnnotationValue::Class { type_ref },
        } => HostExpressionKind::TypeLiteral(HostTypeFact::Class {
            internal_name: binaries.get(type_ref).cloned().ok_or_else(|| {
                FactsError::Unknown("class literal JVM identity is unavailable".into())
            })?,
        }),
        Fact::Found {
            value: AnnotationValue::Array { values },
        } => {
            if values
                .iter()
                .any(|value| !contains(&argument.range, &value.range))
            {
                return Err(FactsError::Protocol(
                    "array element is outside its annotation argument".into(),
                ));
            }
            HostExpressionKind::Array(
                values
                    .iter()
                    .map(|value| convert_expression(value, types, binaries, source).map(Some))
                    .collect::<Result<_, _>>()?,
            )
        }
        _ => {
            return Err(FactsError::Unknown(
                "annotation value is unavailable".into(),
            ))
        }
    };
    Ok(HostExpressionFact {
        validity: HostNodeValidity::default(),
        kind,
        source_text: text.into(),
        source_range: Some(argument.range.start..argument.range.end),
    })
}

fn contains(parent: &ByteRange, child: &ByteRange) -> bool {
    parent.start <= child.start && child.start <= child.end && child.end <= parent.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcdev_core::{
        host_source::{
            HostFact, HostSemanticEnvironment, HostSemanticModelProvider, HostSemanticRequest,
        },
        model::{ClassIndex, ClassIndexEntry, FieldIndexEntry, MethodIndexEntry},
    };
    use sha2::Digest;
    use std::sync::Mutex;

    fn found(value: Value) -> Value {
        json!({"state":"found","value":value})
    }
    fn unknown() -> Value {
        json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
    }
    fn span(source: &str, text: &str) -> Value {
        let start = source.find(text).unwrap();
        json!({"start":start,"end":start+text.len()})
    }
    fn modifiers(visibility: &str) -> Value {
        json!({"visibility":found(json!(visibility)),"static":found(json!(false)),
            "final":found(json!(false)),"abstract":found(json!(false))})
    }
    fn annotation(
        source: &str,
        text: &str,
        name: &str,
        reference: &str,
        arguments: Value,
    ) -> Value {
        let start = source.find(text).unwrap();
        let name_start = start + text.find(name).unwrap();
        json!({"typeRef":reference,"useSiteTarget":null,"range":span(source,text),
            "nameRange":{"start":name_start,"end":name_start+name.len()},"arguments":arguments})
    }
    fn kotlin_fixture(target_known: bool) -> (String, Value) {
        let source = r#"import io.github.diskria.lapis.annotations.KMixin
import org.spongepowered.asm.mixin.injection.Inject
import org.spongepowered.asm.mixin.injection.At as AtAlias
import fixture.DrillTarget as Drill

@KMixin(target = Drill::class)
class DrillMixin {
    @Inject(method = ["useIt"], at = [AtAlias("HEAD")], cancellable = true)
    fun Drill.onUse(cir: CallbackInfoReturnable<ItemInteractionResult>) {}
}"#
        .to_owned();
        let mixin_text = "@KMixin(target = Drill::class)";
        let inject_text =
            "@Inject(method = [\"useIt\"], at = [AtAlias(\"HEAD\")], cancellable = true)";
        let mixin = annotation(
            &source,
            mixin_text,
            "KMixin",
            "mixin-ref",
            json!([{"name":"target","range":span(&source,"Drill::class"),"value":if target_known {
                found(json!({"kind":"class","typeRef":"target-ref"}))
            } else {
                unknown()
            }}]),
        );
        let at = annotation(
            &source,
            "AtAlias(\"HEAD\")",
            "AtAlias",
            "at-ref",
            json!([{"name":null,"range":span(&source,"\"HEAD\""),
                "value":found(json!({"kind":"string","value":"HEAD",
                    "contentRange":span(&source,"HEAD")}))}]),
        );
        let inject = annotation(
            &source,
            inject_text,
            "Inject",
            "inject-ref",
            json!([
                {"name":"at","range":span(&source,"AtAlias(\"HEAD\")"),
                    "value":found(json!({"kind":"annotation","annotation":at}))}
            ]),
        );
        let class_start = source.find(mixin_text).unwrap();
        let class_body = source.find("class DrillMixin {").unwrap() + "class DrillMixin".len();
        let method_start = source.find(inject_text).unwrap();
        let method_body = source.find("{}").unwrap();
        let declarations = json!([
            {"id":"class-declaration","owner":null,"name":"DrillMixin","range":span(&source,"DrillMixin"),
                "annotations":[mixin],"sourceKind":"Class","fullRange":{"start":class_start,"end":source.len()},
                "headerRange":{"start":class_start,"end":class_body},"nameRange":span(&source,"DrillMixin"),
                "bodyRange":{"start":class_body,"end":source.len()},"modifiers":modifiers("Package")},
            {"id":"method-declaration","owner":"class-declaration","name":"onUse","range":span(&source,"onUse"),
                "annotations":[inject],"sourceKind":"Function","fullRange":{"start":method_start,"end":method_body+2},
                "headerRange":{"start":method_start,"end":method_body},"nameRange":span(&source,"onUse"),
                "bodyRange":{"start":method_body,"end":method_body+2},"modifiers":modifiers("Package")}
        ]);
        (source, declarations)
    }

    struct Host {
        source: String,
        declarations: Value,
        queries: Mutex<Vec<Value>>,
        member_unknown: bool,
        member_foreign: bool,
        annotation_types_unknown: bool,
        annotation_site_mode: u8,
    }
    impl SemanticFactClient for Host {
        fn check_active(&self) -> Result<(), FactsError> {
            Ok(())
        }
        fn read_facts(&self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
            assert_eq!(evaluation, "evaluation");
            self.queries.lock().unwrap().extend_from_slice(queries);
            let facts = queries
                .iter()
                .map(|query| match query["op"].as_str().unwrap() {
                    "sourceFacts" => json!({"declarations":self.declarations}),
                    "annotationSites" => {
                        assert_ne!(self.annotation_site_mode, 0);
                        let declarations = self.declarations.as_array().unwrap();
                        let mut mixin = declarations[0]["annotations"][0].clone();
                        if self.annotation_site_mode == 2 {
                            mixin["typeRef"] = json!("wrong-ref");
                        }
                        let inject = declarations[1]["annotations"][0].clone();
                        let at = inject["arguments"][0]["value"]["value"]["annotation"].clone();
                        let site = |owner: Option<&str>, annotation: Value| {
                            let mut site = json!({"owner":owner});
                            site.as_object_mut()
                                .unwrap()
                                .extend(annotation.as_object().unwrap().clone());
                            site
                        };
                        json!({
                            "source": {
                                "documentUri": found(json!("file:///project/DrillMixin.kt")),
                                "identity": {"kind":"source","revision":"test"},
                                "sha256": format!("{:x}", sha2::Sha256::digest(self.source.as_bytes()))
                            },
                            "annotations": [
                                site(Some("class-declaration"), mixin),
                                site(Some("method-declaration"), inject),
                                site(None, at)
                            ],
                            "coverage": found(Value::Null)
                        })
                    }
                    "resolveTypeRef" => {
                        let (id, name) = match query["reference"].as_str().unwrap() {
                            "mixin-ref" => {
                                ("type-mixin", "io.github.diskria.lapis.annotations.KMixin")
                            }
                            "inject-ref" => (
                                "type-inject",
                                "org.spongepowered.asm.mixin.injection.Inject",
                            ),
                            "at-ref" => ("type-at", "org.spongepowered.asm.mixin.injection.At"),
                            "target-ref" => ("type-target", "fixture.DrillTarget"),
                            "wrong-ref" => ("type-wrong", "fixture.NotMixin"),
                            other => panic!("unexpected Kotlin source type reference {other}"),
                        };
                        if self.annotation_types_unknown
                            && query["reference"] != "target-ref"
                            && query["reference"] != "wrong-ref"
                        {
                            unknown()
                        } else {
                            found(json!({"id":id,"qualifiedName":name,"package":"fixture"}))
                        }
                    }
                    "declarationType" => found(json!({
                        "id":"owner-type","qualifiedName":"fixture.DrillMixin","package":"fixture"
                    })),
                    "declarationMember" if self.member_unknown => unknown(),
                    "declarationMember" if self.member_foreign => found(json!({
                        "id":"member-id","name":"onUse","kind":"method",
                        "signature":unknown(),"dispatch":unknown(),"readOnly":true,
                        "contextSource":null,"visibility":unknown(),"synthetic":unknown(),
                        "mismatches":[]
                    })),
                    "declarationMember" => {
                        panic!("source-only Kotlin injector must not query {query}")
                    }
                    "typeJvm" => json!({
                        "binaryName":found(json!({"internalName":"fixture/DrillTarget",
                            "descriptor":"Lfixture/DrillTarget;"})),
                        "classFlags":unknown(),
                        "declaredMembers":{"members":[],"coverage":unknown()},
                        "abi":unknown(),"jvmAbiCoverage":unknown()
                    }),
                    other => panic!("unexpected Kotlin source fact operation {other}"),
                })
                .collect::<Vec<_>>();
            Ok(json!({"facts":facts}))
        }
    }

    struct TargetIndex {
        target: Option<ClassIndexEntry>,
        alternatives: Vec<ClassIndexEntry>,
    }
    impl ClassIndex for TargetIndex {
        fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
            self.alternatives
                .iter()
                .filter(|entry| {
                    entry.internal_name.starts_with(prefix)
                        || entry.fqn().starts_with(prefix)
                        || entry.simple_name.starts_with(prefix)
                })
                .take(limit)
                .cloned()
                .collect()
        }
        fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry> {
            self.target
                .as_ref()
                .filter(|entry| entry.internal_name == internal_name)
                .cloned()
        }
        fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry> {
            self.target
                .as_ref()
                .filter(|entry| entry.fqn() == fqn)
                .cloned()
        }
        fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
            vec![]
        }
        fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
            vec![]
        }
    }

    fn provider(
        index: Option<TargetIndex>,
        target_known: bool,
        member_unknown: bool,
        member_foreign: bool,
        annotation_types_unknown: bool,
        annotation_site_mode: u8,
    ) -> (HostSemanticProvider, Arc<Host>, String) {
        let (source, declarations) = kotlin_fixture(target_known);
        let host = Arc::new(Host {
            source: source.clone(),
            declarations,
            queries: Mutex::new(Vec::new()),
            member_unknown,
            member_foreign,
            annotation_types_unknown,
            annotation_site_mode,
        });
        let evaluation: Evaluation = serde_json::from_value(json!({
            "evaluation":"evaluation","owner":"owner","source":source,
            "configuration":{},"declarations":[]
        }))
        .unwrap();
        let mut provider = HostSemanticProvider::with_client(
            evaluation,
            host.clone(),
            "file:///project/DrillMixin.kt".into(),
            Some(3),
        )
        .unwrap()
        .with_source_facts(true)
        .with_annotation_sites(annotation_site_mode != 0, None);
        if let Some(index) = index {
            provider.class_index = Some(Arc::new(index));
        }
        (provider, host, source)
    }

    fn target_index() -> TargetIndex {
        TargetIndex {
            target: Some(ClassIndexEntry {
                simple_name: "DrillTarget".into(),
                package_name: "fixture".into(),
                internal_name: "fixture/DrillTarget".into(),
            }),
            alternatives: vec![],
        }
    }

    fn parse(provider: &HostSemanticProvider, source: &str) -> HostFact<HostSemanticModel> {
        provider
            .parse(&HostSemanticRequest {
                source,
                source_uri: "file:///project/DrillMixin.kt",
                document_version: Some(3),
                environment: &HostSemanticEnvironment::default(),
                project: None,
            })
            .unwrap()
            .fact
    }

    #[test]
    fn kotlin_injector_uses_source_model_without_unused_argument_or_handler_facts() {
        let (provider, host, source) = provider(Some(target_index()), false, true, false, false, 0);
        let answer = parse(&provider, &source);
        let HostFact::Known(result) = answer else {
            panic!("expected source-level Kotlin model: {answer:?}")
        };
        assert_eq!(result.availability, SemanticAvailability::StableFallback);
        assert_eq!(result.model.parse_source, ParseSource::HandWrittenFallback);
        assert_eq!(result.model.targets[0].internal_name, "fixture/DrillTarget");
        assert_eq!(result.model.injectors[0].method_selectors[0].name, "useIt");
        assert_eq!(result.model.injectors[0].at_selectors[0].value, "HEAD");
        let queries = host.queries.lock().unwrap();
        let operations = queries
            .iter()
            .filter_map(|query| query["op"].as_str())
            .collect::<Vec<_>>();
        assert!(operations.contains(&"sourceFacts"));
        assert!(operations.contains(&"resolveTypeRef"));
        assert!(!operations
            .iter()
            .any(|op| matches!(*op, "declarationMember" | "typeJvm")));
    }

    #[test]
    fn kotlin_injector_falls_back_when_only_its_handler_member_fact_is_unknown() {
        let (provider, host, source) = provider(Some(target_index()), true, true, false, false, 0);
        let HostFact::Known(result) = parse(&provider, &source) else {
            panic!("expected source-level Kotlin model")
        };
        assert_eq!(result.model.parse_source, ParseSource::HandWrittenFallback);
        assert_eq!(result.model.targets[0].internal_name, "fixture/DrillTarget");
        let queries = host.queries.lock().unwrap();
        let operations = queries
            .iter()
            .filter_map(|query| query["op"].as_str())
            .collect::<Vec<_>>();
        assert!(operations.contains(&"typeJvm"));
        assert!(operations.contains(&"declarationMember"));
    }

    #[test]
    fn kotlin_injector_falls_back_when_mc_annotation_type_facts_are_unknown() {
        let (provider, host, source) = provider(Some(target_index()), false, true, false, true, 1);
        let HostFact::Known(result) = parse(&provider, &source) else {
            panic!("expected source-level Kotlin model")
        };
        assert_eq!(result.model.parse_source, ParseSource::HandWrittenFallback);
        assert_eq!(result.model.targets[0].internal_name, "fixture/DrillTarget");
        let queries = host.queries.lock().unwrap();
        assert!(queries.iter().any(|query| query["op"] == "resolveTypeRef"));
        assert!(!queries
            .iter()
            .any(|query| query["op"] == "declarationMember"));
    }

    #[test]
    fn kotlin_injector_does_not_hide_known_non_mc_annotation_site() {
        let (provider, _, source) = provider(Some(target_index()), false, true, false, true, 2);
        let answer = parse(&provider, &source);
        assert!(matches!(
            answer,
            HostFact::Unavailable { reason }
                if reason.contains("annotation reference alias")
        ));
    }

    #[test]
    fn source_only_injectors_accepts_wrap_operation() {
        let declaration = Declaration {
            id: "method-declaration".into(),
            owner: Some("class-declaration".into()),
            range: ByteRange { start: 0, end: 1 },
            name: Some("onPlace".into()),
            annotations: vec![Annotation {
                type_ref: "wrap-ref".into(),
                use_site_target: None,
                range: ByteRange { start: 0, end: 1 },
                name_range: None,
                arguments: vec![],
            }],
            source_kind: Some("Function".into()),
            full_range: None,
            header_range: None,
            name_range: None,
            body_range: None,
            modifiers: None,
            parameters: None,
        };
        let mut identities = BTreeMap::new();
        identities.insert(
            "wrap-ref".into(),
            Fact::Found {
                value: TypeFact {
                    id: "wrap-type".into(),
                    qualified_name:
                        "com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation".into(),
                    package: "com.llamalad7.mixinextras.injector.wrapoperation".into(),
                    read_only: false,
                    context_source: None,
                },
            },
        );
        assert!(source_only_injectors(&[&declaration], &identities));
    }

    #[test]
    fn kotlin_injector_does_not_bypass_foreign_found_member_context() {
        let (provider, _, source) = provider(Some(target_index()), true, false, true, false, 0);
        let answer = parse(&provider, &source);
        assert!(matches!(
            answer,
            HostFact::Unavailable { reason } if reason.contains("foreign source context")
        ));
    }

    #[test]
    fn kotlin_injector_requires_a_unique_selected_target_index() {
        for index in [
            None,
            Some(TargetIndex {
                target: None,
                alternatives: vec![
                    ClassIndexEntry {
                        simple_name: "DrillTarget".into(),
                        package_name: "fixture".into(),
                        internal_name: "fixture/DrillTarget".into(),
                    },
                    ClassIndexEntry {
                        simple_name: "DrillTarget".into(),
                        package_name: "other".into(),
                        internal_name: "other/DrillTarget".into(),
                    },
                ],
            }),
        ] {
            let (provider, _, source) = provider(index, false, true, false, false, 0);
            let answer = parse(&provider, &source);
            assert!(
                matches!(
                    &answer,
                    HostFact::Unavailable { reason }
                        if reason.contains("Mixin target")
                ),
                "unexpected answer: {answer:?}"
            );
        }
    }
}
fn collect_targets(
    argument: &Argument,
    source: &str,
    binaries: &BTreeMap<String, String>,
    targets: &mut Vec<MixinTargetRef>,
) -> Result<(), FactsError> {
    match &argument.value {
        Fact::Found {
            value: AnnotationValue::Array { values },
        } => {
            for value in values {
                collect_targets(value, source, binaries, targets)?;
            }
        }
        Fact::Found {
            value: AnnotationValue::Class { type_ref },
        } => targets.push(MixinTargetRef {
            internal_name: binaries.get(type_ref).cloned().ok_or_else(|| {
                FactsError::Unknown("class target JVM identity is unavailable".into())
            })?,
            range: text_range(source, &argument.range)?,
        }),
        Fact::Found {
            value:
                AnnotationValue::String {
                    value,
                    content_range,
                },
        } => {
            if value.is_empty() {
                return Err(FactsError::Unknown("empty Mixin target name".into()));
            }
            targets.push(MixinTargetRef {
                internal_name: value.replace('.', "/"),
                range: text_range(source, content_range)?,
            });
        }
        Fact::Found {
            value: AnnotationValue::MappedString { value, .. },
        } => {
            if value.is_empty() {
                return Err(FactsError::Unknown("empty Mixin target name".into()));
            }
            targets.push(MixinTargetRef {
                internal_name: value.replace('.', "/"),
                range: text_range(source, &argument.range)?,
            });
        }
        _ => return Err(FactsError::Unknown("Mixin target is unavailable".into())),
    }
    Ok(())
}
fn member_annotation(
    annotation: MixinAnnotation,
    is_method: bool,
) -> Option<MixinMemberAnnotationKind> {
    match annotation {
        MixinAnnotation::Shadow => Some(MixinMemberAnnotationKind::Shadow),
        MixinAnnotation::Accessor if is_method => Some(MixinMemberAnnotationKind::Accessor),
        MixinAnnotation::Invoker if is_method => Some(MixinMemberAnnotationKind::Invoker),
        MixinAnnotation::Overwrite if is_method => Some(MixinMemberAnnotationKind::Overwrite),
        _ => None,
    }
}
fn signature_types(
    kind: &str,
    value: &str,
) -> Result<(bool, Vec<HostTypeFact>, HostTypeFact), FactsError> {
    match kind {
        "method" => {
            let signature = descriptor::parse_method_descriptor(value)
                .map_err(|e| FactsError::Protocol(e.to_string()))?;
            Ok((
                true,
                signature.parameters.into_iter().map(host_type).collect(),
                host_type(signature.return_type),
            ))
        }
        "field" => Ok((
            false,
            vec![],
            host_type(
                descriptor::parse_field_descriptor(value)
                    .map_err(|e| FactsError::Protocol(e.to_string()))?,
            ),
        )),
        _ => Err(FactsError::Protocol("unknown authored member kind".into())),
    }
}
pub(super) fn host_type(value: JvmType) -> HostTypeFact {
    match value {
        JvmType::ByteType => HostTypeFact::Primitive(HostPrimitive::Byte),
        JvmType::CharType => HostTypeFact::Primitive(HostPrimitive::Char),
        JvmType::DoubleType => HostTypeFact::Primitive(HostPrimitive::Double),
        JvmType::FloatType => HostTypeFact::Primitive(HostPrimitive::Float),
        JvmType::IntType => HostTypeFact::Primitive(HostPrimitive::Int),
        JvmType::LongType => HostTypeFact::Primitive(HostPrimitive::Long),
        JvmType::ShortType => HostTypeFact::Primitive(HostPrimitive::Short),
        JvmType::BooleanType => HostTypeFact::Primitive(HostPrimitive::Boolean),
        JvmType::VoidType => HostTypeFact::Primitive(HostPrimitive::Void),
        JvmType::ObjectType(internal_name) => HostTypeFact::Class { internal_name },
        JvmType::ArrayType(element) => HostTypeFact::Array {
            element: Box::new(host_type(*element)),
            dimensions: 1,
        },
    }
}
fn string_values(annotation: &Annotation, name: &str) -> Result<Vec<String>, FactsError> {
    fn append(argument: &Argument, values: &mut Vec<String>) -> Result<(), FactsError> {
        match &argument.value {
            Fact::Found {
                value:
                    AnnotationValue::String { value, .. } | AnnotationValue::MappedString { value, .. },
            } => values.push(value.clone()),
            Fact::Found {
                value: AnnotationValue::Array { values: children },
            } => {
                for child in children {
                    append(child, values)?;
                }
            }
            _ => {
                return Err(FactsError::Unknown(
                    "injector method selector is unavailable".into(),
                ))
            }
        }
        Ok(())
    }
    let mut values = Vec::new();
    for argument in annotation
        .arguments
        .iter()
        .filter(|argument| argument.name.as_deref() == Some(name))
    {
        append(argument, &mut values)?;
    }
    Ok(values)
}
