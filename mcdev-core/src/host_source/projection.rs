use super::*;
use crate::extras_model::ResolvedMixinExtrasContext;
use crate::mixin_source::*;
use crate::{offset_to_position, McTextRange};
use std::{collections::BTreeSet, ops::Range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSemanticMember {
    pub name: String,
    pub annotations: Vec<HostAnnotationFact>,
    pub parameter_types: Vec<HostTypeFact>,
    pub return_type: HostTypeFact,
    pub modifiers: BTreeSet<JavaModifier>,
    pub is_method: bool,
    pub range: Range<usize>,
    pub name_range: Range<usize>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostSemanticDocument {
    pub package_name: String,
    pub qualified_name: String,
    pub type_kind: JavaTypeKind,
    pub type_annotations: Vec<HostAnnotationFact>,
    pub members: Vec<HostSemanticMember>,
}
fn text_range(source: &str, range: &Range<usize>) -> Option<McTextRange> {
    if range.end < range.start {
        return None;
    }
    Some(McTextRange {
        start: offset_to_position(source, range.start)?,
        end: offset_to_position(source, range.end)?,
    })
}
/// The host has already resolved binding identity. Source imports are a fallback
/// only for unavailable bindings, matching the shared annotation identity policy.
pub fn host_mixin_annotation(
    annotation: &HostAnnotationFact,
    imports: &JavaSourceImports,
) -> Option<MixinAnnotation> {
    if let Some(fqn) = &annotation.qualified_name {
        return MixinAnnotation::from_official_fqn(fqn);
    }
    let token = &annotation.source_name;
    if token.contains('.') {
        return MixinAnnotation::from_official_fqn(token);
    }
    if let Some(fqn) = imports.explicit.get(token) {
        return MixinAnnotation::from_official_fqn(fqn);
    }
    if imports.ambiguous_explicit.contains_key(token) {
        return None;
    }
    MixinAnnotation::from_simple_name(token)
}
pub fn resolved_method_descriptor_parts(
    parameters: &[HostTypeFact],
    return_type: &HostTypeFact,
) -> (Vec<String>, Option<String>) {
    let parameters: Option<Vec<String>> = parameters.iter().map(HostTypeFact::descriptor).collect();
    let Some(parameters) = parameters else {
        return (vec![], None);
    };
    let Some(ret) = return_type.descriptor() else {
        return (vec![], None);
    };
    let descriptor = format!("({}){ret}", parameters.concat());
    if crate::descriptor::parse_method_descriptor(&descriptor).is_err() {
        return (vec![], None);
    }
    (parameters, Some(descriptor))
}
pub fn host_mixin_targets(
    source: &str,
    annotation: &HostAnnotationFact,
    imports: &JavaSourceImports,
) -> Vec<MixinTargetRef> {
    let mut result = vec![];
    for (key, node) in &annotation.members {
        if !["value", "target", "targets"].contains(&key.as_str()) {
            continue;
        }
        let elements: Vec<&HostExpressionFact> = match &node.kind {
            HostExpressionKind::Array(v) => v.iter().filter_map(Option::as_ref).collect(),
            _ => vec![node],
        };
        for element in elements {
            let raw = element
                .source_text
                .strip_suffix(".class")
                .unwrap_or(&element.source_text)
                .trim_matches('"');
            let internal = match &element.kind {
                HostExpressionKind::TypeLiteral(ty) => match ty.jvm_type() {
                    Some(crate::descriptor::JvmType::ObjectType(name)) => name,
                    _ => imports
                        .explicit
                        .get(raw)
                        .map(|s| s.replace('.', "/"))
                        .unwrap_or_else(|| raw.into()),
                },
                _ => element
                    .constant_string()
                    .unwrap_or_else(|| raw.into())
                    .replace('.', "/"),
            };
            let range = element
                .source_range
                .as_ref()
                .and_then(|r| text_range(source, r))
                .unwrap_or_default();
            result.push(MixinTargetRef {
                internal_name: internal,
                range,
            });
        }
    }
    result
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostCollectedContexts {
    pub contexts: Vec<ResolvedMixinExtrasContext>,
    pub warnings: Vec<String>,
}
pub fn collect_host_extras_contexts(
    source: &str,
    members: &[HostSemanticMember],
) -> HostCollectedContexts {
    let mut collected = HostCollectedContexts::default();
    for member in members.iter().filter(|m| m.is_method) {
        let Some(range) = text_range(source, &member.range) else {
            collected.warnings.push(format!(
                "Host source range unavailable for method {}",
                member.name
            ));
            continue;
        };
        match extract_resolved_extras_context(&member.annotations, range) {
            HostResolvedContextResult::NotHandler => {}
            HostResolvedContextResult::Resolved(context) => collected.contexts.push(context),
            HostResolvedContextResult::Unavailable { reason } => {
                collected.warnings.push(if member.name.is_empty() {
                    format!("MixinExtras context unavailable: {reason}")
                } else {
                    format!(
                        "MixinExtras context unavailable for method {}: {reason}",
                        member.name
                    )
                })
            }
        }
    }
    collected
}
/// Merge a host normalized document with the existing MC source/domain parser.
/// No compilation unit, SDK/classpath scanner or second language parser is built.
pub fn project_host_semantic_document(
    source: &str,
    uri: &str,
    document: &HostSemanticDocument,
) -> MixinClassModel {
    let imports = JavaTypeDescriptorResolver::imports_for_uri(source, uri);
    let mut model = MixinSemanticModelParser::parse(source, uri);
    model.parse_source = ParseSource::HostNormalized;
    model.confidence = ParseConfidence::High;
    model.package_name = document.package_name.clone();
    model.qualified_name = document.qualified_name.clone();
    model.type_kind = document.type_kind;
    let targets: Vec<_> = document
        .type_annotations
        .iter()
        .filter(|a| host_mixin_annotation(a, &imports) == Some(MixinAnnotation::Mixin))
        .flat_map(|a| host_mixin_targets(source, a, &imports))
        .collect();
    if !targets.is_empty() {
        model.targets = targets;
    }
    let mut members = vec![];
    let mut resolved = 0;
    let mut failed = 0;
    for fact in &document.members {
        let selected = fact.annotations.iter().find_map(|a| {
            let kind = match host_mixin_annotation(a, &imports)? {
                MixinAnnotation::Accessor if fact.is_method => MixinMemberAnnotationKind::Accessor,
                MixinAnnotation::Invoker if fact.is_method => MixinMemberAnnotationKind::Invoker,
                MixinAnnotation::Shadow => MixinMemberAnnotationKind::Shadow,
                MixinAnnotation::Overwrite if fact.is_method => {
                    MixinMemberAnnotationKind::Overwrite
                }
                _ => return None,
            };
            Some((kind, a))
        });
        let Some((kind, annotation)) = selected else {
            continue;
        };
        let Some(range) = text_range(source, &fact.range) else {
            model.warnings.push(format!(
                "Host source range unavailable for member {}",
                fact.name
            ));
            continue;
        };
        let Some(name_range) = text_range(source, &fact.name_range) else {
            model.warnings.push(format!(
                "Host name range unavailable for member {}",
                fact.name
            ));
            continue;
        };
        let annotation_range = annotation
            .source_range
            .as_ref()
            .and_then(|r| text_range(source, r))
            .unwrap_or_default();
        let return_descriptor = fact.return_type.descriptor();
        let (parameter_descriptors, method_descriptor) = if fact.is_method {
            resolved_method_descriptor_parts(&fact.parameter_types, &fact.return_type)
        } else {
            (vec![], None)
        };
        for ty in fact
            .parameter_types
            .iter()
            .chain(std::iter::once(&fact.return_type))
        {
            if ty.descriptor().is_some() {
                resolved += 1;
            } else {
                failed += 1;
            }
        }
        let complete = if fact.is_method {
            method_descriptor.is_some()
        } else {
            return_descriptor.is_some()
        };
        let warnings = if complete {
            vec![]
        } else {
            vec![if fact.is_method {
                format!(
                    "Host semantic facts could not resolve descriptor for method {}",
                    fact.name
                )
            } else {
                "Host semantic facts could not resolve descriptor for field declaration".into()
            }]
        };
        let explicit_target_name = if fact.is_method {
            annotation
                .members
                .iter()
                .find(|(k, _)| ["value", "targets", "target"].contains(&k.as_str()))
                .map(|(_, n)| n.source_text.trim_matches('"').to_string())
        } else {
            Some(fact.name.clone())
        };
        members.push(MixinMemberModel {
            annotation_kind: kind,
            java_name: fact.name.clone(),
            explicit_target_name,
            return_descriptor,
            parameter_descriptors,
            method_descriptor,
            modifiers: fact.modifiers.clone(),
            range,
            annotation_range,
            name_range,
            parse_source: ParseSource::HostNormalized,
            confidence: if complete {
                ParseConfidence::High
            } else {
                ParseConfidence::Low
            },
            warnings,
            is_method: Some(fact.is_method),
        });
    }
    if !members.is_empty() {
        model.members = members;
    }
    let contexts = collect_host_extras_contexts(source, &document.members);
    model.resolved_mixin_extras_contexts = contexts.contexts;
    model.warnings.extend(contexts.warnings);
    for member in &model.members {
        model.warnings.extend(member.warnings.clone());
    }
    let mut seen = BTreeSet::new();
    model.warnings.retain(|w| seen.insert(w.clone()));
    if failed > 0
        || model
            .members
            .iter()
            .any(|m| m.confidence != ParseConfidence::High)
    {
        model.confidence = ParseConfidence::Medium;
    }
    model.debug_info = SemanticParseDebugInfo {
        parse_source: ParseSource::HostNormalized,
        binding_resolved_count: resolved,
        binding_failed_count: failed,
        ..Default::default()
    };
    model
}
