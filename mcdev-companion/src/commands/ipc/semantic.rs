//! Convert facts from one live ktlsp evaluation into the existing MC semantic model.
//!
//! Values and opaque identities in this module are owned by the evaluation;
//! nothing here caches or re-resolves a source name as a JVM binary name.

use crate::{
    host_facts::{draft::EditingMember, FactsError},
    provider_commands::CommandFacts,
    wire::{
        Annotation, AnnotationValue, Argument, ByteRange, Evaluation, Fact, HandleKind, Registry,
        TypeFact, MAX_BATCH,
    },
};
use mcdev_core::{
    descriptor::{self, JvmType},
    host_source::{
        project_host_semantic_document, HostAnnotationFact, HostAnswer, HostConstant,
        HostExpressionFact, HostExpressionKind, HostGenerations, HostNodeValidity, HostPrimitive,
        HostReadSet, HostReadValidator, HostSemanticDocument, HostSemanticMember,
        HostSemanticModel, HostSemanticModelProvider, HostSemanticRequest, HostSourceError,
        HostSourceResult, HostTypeFact, SemanticAvailability,
    },
    mixin_source::{
        annotations_with_imports, parse_member_declaration, JavaModifier,
        JavaTypeDescriptorResolver, JavaTypeKind, MixinAnnotation, MixinMemberAnnotationKind,
        MixinMemberDeclarationParser, MixinTargetRef, ParseConfidence, ParseSource,
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
        })
    }

    pub fn with_index(mut self, index: Arc<super::index::EvaluationIndex>) -> Self {
        self.index = Some(index);
        self
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

    fn build(&self) -> Result<HostSemanticModel, FactsError> {
        let started = std::time::Instant::now();
        let e = &self.evaluation;
        let mut registry = Registry::default();
        e.validate(&mut registry).map_err(FactsError::Protocol)?;
        let imports = JavaTypeDescriptorResolver::imports_for_uri(&e.source, &self.document_uri);
        let supplied = e
            .declarations
            .iter()
            .flat_map(|d| &d.annotations)
            .map(|a| (a.range.start, a.range.end))
            .collect::<BTreeSet<_>>();
        let nested_arguments = e
            .declarations
            .iter()
            .flat_map(|d| &d.annotations)
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

        let annotation_refs: BTreeSet<_> = e
            .declarations
            .iter()
            .flat_map(|declaration| declaration.annotations.iter().map(|a| a.type_ref.clone()))
            .collect();
        let identities = self.resolve_refs(&annotation_refs, &mut registry)?;
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
        let class_types = self.resolve_refs(&class_refs, &mut registry)?;
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
            if let Some(index) = &self.index {
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
        let mut mapped_names = BTreeMap::new();
        for (declaration, fact) in relevant.iter().zip(member_facts) {
            let item = require_fact(fact, "authored member")?;
            let member = &item.member;
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
            let header =
                parse_member_declaration(source, after_annotation, &imports).ok_or_else(|| {
                    FactsError::Unknown("authored declaration header range is unavailable".into())
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
            let mut modifiers = header.modifiers;
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
            members.push(HostSemanticMember {
                name: member.name.clone(),
                annotations,
                parameter_types: params,
                return_type: result,
                modifiers,
                is_method,
                range: start..header.end,
                name_range: declaration.range.start..declaration.range.end,
            });
        }
        let document = HostSemanticDocument {
            package_name: owner_type.package,
            qualified_name: owner_type.qualified_name,
            type_kind: JavaTypeKind::Unknown,
            type_annotations: vec![type_annotation],
            members,
        };
        let mut model = project_host_semantic_document(source, &self.document_uri, &document);
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
        let lexical_members =
            MixinMemberDeclarationParser::parse_members_with_imports(source, None, &imports);
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
            }
            result.insert(reference.clone(), value);
        }
        Ok(result)
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
        if request.source != self.evaluation.source
            || request.source_uri != self.document_uri
            || request.document_version.is_some()
                && request.document_version != self.document_version
        {
            return Ok(HostAnswer::unavailable(
                "semantic source, URI or version is stale",
            ));
        }
        match self.build() {
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
            value: AnnotationValue::Class { type_ref },
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
        Fact::Found { .. } => {}
        _ => {
            return Err(FactsError::Unknown(
                "annotation argument is unavailable".into(),
            ))
        }
    }
    Ok(())
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
            convert_expression(argument, binaries, source)?,
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
                    .map(|value| convert_expression(value, binaries, source).map(Some))
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
fn host_type(value: JvmType) -> HostTypeFact {
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
                value: AnnotationValue::String { value, .. },
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
