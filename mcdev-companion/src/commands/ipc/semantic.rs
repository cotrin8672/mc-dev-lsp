//! Convert facts from one live ktlsp evaluation into the existing MC semantic model.
//!
//! Values and opaque identities in this module are owned by the evaluation;
//! nothing here caches or re-resolves a source name as a JVM binary name.
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
        annotations_with_imports, parse_member_declaration, AtSelectorModel, InjectorKind,
        InjectorModel, JavaModifier, JavaTypeDescriptorResolver, JavaTypeKind, MethodSelector,
        MixinAnnotation, MixinMemberAnnotationKind, MixinMemberDeclarationParser, MixinTargetRef,
        ParseConfidence, ParseSource,
    },
    offset_to_position, McTextRange,
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
    source_facts_v2: bool,
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
            source_facts_v2: false,
            project_sources: BTreeMap::new(),
        })
    }

    pub fn with_index(mut self, index: Arc<super::index::EvaluationIndex>) -> Self {
        self.index = Some(index);
        self
    }

    pub fn with_source_facts(mut self, enabled: bool) -> Self {
        self.source_facts_v2 = enabled;
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
        if annotations_with_imports(&e.source, &imports)
            .iter()
            .any(|site| {
                !supplied.contains(&(site.start, site.end))
                    && !nested_arguments
                        .iter()
                        .any(|(start, end)| *start <= site.start && site.end <= *end)
            })
        {
            return Err(FactsError::Unknown(
                "host annotation inventory is incomplete".into(),
            ));
        }

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
        let source_context = selected.map(|entry| &entry.descriptor);
        let identities = self.resolve_refs(&annotation_refs, &mut registry, source_context)?;
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
        if owners.is_empty() && self.source_facts_v2 {
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
            let item = require_fact(fact, "authored member")?;
            let member = &item.member;
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
                                &signature.descriptor,
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
    descriptor: &str,
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
        handler_method_descriptor: Some(descriptor.into()),
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
