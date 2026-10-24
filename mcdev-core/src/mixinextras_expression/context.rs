use super::parsing::whitespace;
use super::*;
use crate::{
    bytecode::{self, AtTargetKind, BytecodeIndex, OccurrenceResultClassification},
    descriptor, expression_engine as engine,
    extras_model::*,
    mixin_source::*,
    mixinextras_handler::HandlerSignatureService,
    *,
};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimpleOperationLayout {
    pub argument_descriptors: Vec<String>,
    pub parameter_names: Vec<String>,
    pub return_descriptor: String,
    pub argument_int_like: Vec<bool>,
    pub return_int_like: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionValueTypeInfo {
    pub descriptor: String,
    pub int_like: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifyReceiverLayout {
    pub receiver_owner_internal_name: String,
    pub parameter_descriptors: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapWithConditionTargetKind {
    Invoke,
    Field,
    Other,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrapWithConditionLayout {
    pub owner_internal_name: String,
    pub parameter_descriptors: Vec<String>,
    pub is_static: bool,
    pub target_kind: WrapWithConditionTargetKind,
    pub result_classification: OccurrenceResultClassification,
}
pub struct ExpressionContextResolver;
#[derive(Clone)]
pub(super) struct SourceAnnotation {
    pub start: usize,
    pub end: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub name: String,
}
pub(super) fn scan_annotations(source: &str, imports: &JavaSourceImports) -> Vec<SourceAnnotation> {
    let tokens = source_tokens(source, imports.is_kotlin);
    let mut out = vec![];
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].text != "@" {
            i += 1;
            continue;
        }
        let start = tokens[i].start;
        let mut n = i + 1;
        let name_start = tokens.get(n).map_or(source.len(), |t| t.start);
        let mut name_end = name_start;
        while n < tokens.len() {
            if n == i + 1
                || tokens[n].text == "."
                || tokens.get(n.wrapping_sub(1)).is_some_and(|t| t.text == ".")
            {
                if matches!(tokens[n].kind, SourceTokenKind::Identifier) || tokens[n].text == "." {
                    name_end = tokens[n].end;
                    n += 1;
                    continue;
                }
            }
            break;
        }
        if name_end == name_start {
            i += 1;
            continue;
        }
        let mut name = source[name_start..name_end].to_string();
        if imports.is_kotlin {
            if imports.ambiguous_explicit.contains_key(&name) {
                i += 1;
                continue;
            }
            if let Some(fqn) = imports.explicit.get(&name) {
                name = fqn.clone();
            } else if !name.contains('.')
                && !imports
                    .wildcard_packages
                    .iter()
                    .any(|p| p == "com.llamalad7.mixinextras.expression")
            {
                name = format!("unresolved.{name}");
            }
        }
        let (body_start, body_end, end) = if tokens.get(n).is_some_and(|t| t.text == "(") {
            let Some(close) = matching(&tokens, n) else {
                i += 1;
                continue;
            };
            let result = (tokens[n].end, tokens[close].start, tokens[close].end);
            n = close + 1;
            result
        } else {
            (name_end, name_end, name_end)
        };
        out.push(SourceAnnotation {
            start,
            end,
            body_start,
            body_end,
            name,
        });
        i = n;
    }
    out
}
fn handler_region_range(source: &str, site: &MixinExtrasAnnotationSite) -> Option<Range<usize>> {
    let injector_end = position_to_offset(source, site.annotation_range.end)?;
    let imports = JavaTypeDescriptorResolver::imports_for(source);
    let anns = scan_annotations(
        source,
        &JavaSourceImports {
            is_kotlin: imports.is_kotlin,
            ..Default::default()
        },
    );
    let handler_start = if let Some(handler) = &site.handler_method {
        position_to_offset(source, handler.range.start)?
    } else {
        let mut at = whitespace(source, injector_end);
        while let Some(a) = anns.iter().find(|a| a.start == at) {
            at = whitespace(source, a.end);
        }
        at
    };
    let mut start = handler_start;
    loop {
        let previous = anns
            .iter()
            .filter(|a| a.end <= start && whitespace(source, a.end) == start)
            .max_by_key(|a| a.start);
        if let Some(a) = previous {
            start = a.start;
        } else {
            break;
        }
    }
    (start < handler_start).then_some(start..handler_start)
}
impl ExpressionContextResolver {
    pub fn resolve_expression_context(
        source: &str,
        site: &MixinExtrasAnnotationSite,
    ) -> ExpressionContext {
        let Some(range) = handler_region_range(source, site) else {
            return ExpressionContext::default();
        };
        Self::parse_handler_annotations(
            &source[range.clone()],
            range.start,
            &JavaTypeDescriptorResolver::imports_for(source),
        )
    }
    pub fn resolve_expression_context_for_site(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        resolved: &[ResolvedMixinExtrasContext],
    ) -> Option<ExpressionContext> {
        if resolved.is_empty() {
            Some(Self::resolve_expression_context(source, site))
        } else {
            select_resolved_mixin_extras_context(site, resolved).map(|r| r.context.clone())
        }
    }
    pub fn find_enclosing_site(source: &str, offset: usize) -> Option<MixinExtrasAnnotationSite> {
        if offset >= source.len() {
            return None;
        }
        HandlerSignatureService::find_sugar_handler_annotation_sites(source)
            .into_iter()
            .filter_map(|site| {
                let range = handler_region_range(source, &site)?;
                range.contains(&offset).then_some((site, range.len()))
            })
            .min_by_key(|v| v.1)
            .map(|v| v.0)
    }
    pub fn handler_region(source: &str, site: &MixinExtrasAnnotationSite) -> String {
        handler_region_range(source, site).map_or(String::new(), |r| source[r].into())
    }
    pub fn parse_handler_annotations(
        region: &str,
        base: usize,
        imports: &JavaSourceImports,
    ) -> ExpressionContext {
        let mut result = ExpressionContext::default();
        for a in scan_annotations(region, imports) {
            let name = a
                .name
                .strip_prefix("com.llamalad7.mixinextras.expression.")
                .unwrap_or(&a.name);
            let body = &region[a.body_start..a.body_end];
            match name {
                "Expression" => {
                    if let Some(value) = ExpressionAnnotationParser::parse(body, Some(imports)) {
                        result.expression_index.expressions.push(value);
                    }
                }
                "Expressions" => result
                    .expression_index
                    .expressions
                    .extend(ExpressionsAnnotationParser::parse(body, Some(imports)).expressions),
                "Definition" => {
                    if let Some(value) = DefinitionsAnnotationParser::parse_definition_at(
                        region,
                        a.start,
                        base,
                        Some(imports),
                    ) {
                        result.definition_index.definitions.push(value);
                    }
                }
                "Definitions" => result.definition_index.definitions.extend(
                    DefinitionsAnnotationParser::parse(body, base + a.body_start, Some(imports))
                        .definitions,
                ),
                _ => {}
            }
        }
        result
    }
    pub fn infer_expression_value_type(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        target: &MethodIndexEntry,
        owners: &[String],
        bytecode: &dyn BytecodeIndex,
        index: &dyn ClassIndex,
        resolved: Option<&ExpressionContext>,
    ) -> Option<String> {
        Self::infer_expression_value_type_info(
            source, site, target, owners, bytecode, index, resolved,
        )
        .map(|v| v.descriptor)
    }
    pub fn infer_expression_value_type_info(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        target: &MethodIndexEntry,
        owners: &[String],
        bytecode: &dyn BytecodeIndex,
        index: &dyn ClassIndex,
        resolved: Option<&ExpressionContext>,
    ) -> Option<ExpressionValueTypeInfo> {
        let fallback;
        let context = if let Some(context) = resolved {
            context
        } else {
            fallback = Self::resolve_expression_context(source, site);
            &fallback
        };
        let expressions = expression_values(context, site);
        if expressions.is_empty() {
            return None;
        }
        if site
            .at_value
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION"))
        {
            let mut unique = vec![];
            for owner in owners {
                if !unique.contains(owner) {
                    unique.push(owner.clone());
                }
            }
            match official_matches(
                &expressions,
                target,
                &unique,
                bytecode,
                &context.definition_index,
                engine::MatchContextType::ModifyExpressionValue,
                &ClassLiteralTypeNameResolver::for_source(source, index),
            ) {
                Matches::Available(matches) => {
                    let infos: Vec<_> = matches.iter().map(|(_, m)| match_type_info(m)).collect();
                    let first = infos.first()?;
                    if infos.iter().all(|v| v.descriptor == first.descriptor) {
                        return Some(ExpressionValueTypeInfo {
                            descriptor: first.descriptor.clone(),
                            int_like: infos.iter().all(|v| v.int_like),
                        });
                    }
                    return None;
                }
                Matches::NoMatch => return None,
                Matches::Unavailable => {
                    if resolved.is_some() {
                        return None;
                    }
                }
            }
        }
        let owner = owners.first()?;
        let mut agreed = None;
        for expression in expressions {
            let scalar = String::from_utf16(&expression).ok()?;
            let value = Self::infer_from_expression(&scalar, owner, target, bytecode, index)?;
            if agreed.as_ref().is_some_and(|a| a != &value) {
                return None;
            }
            agreed = Some(value);
        }
        agreed.map(|descriptor| ExpressionValueTypeInfo {
            descriptor,
            int_like: false,
        })
    }
    pub fn infer_simple_operation_layout(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        target: &MethodIndexEntry,
        owners: &[String],
        bytecode: &dyn BytecodeIndex,
        index: &dyn ClassIndex,
        resolved: Option<&ExpressionContext>,
    ) -> Option<SimpleOperationLayout> {
        let matches = infer_matches(
            source,
            site,
            target,
            owners,
            bytecode,
            index,
            resolved,
            engine::MatchContextType::WrapOperation,
        )?;
        let mut agreed = None;
        for (_, m) in matches {
            let layout = simple_operation_layout(&m)?;
            if agreed.as_ref().is_some_and(|a| a != &layout) {
                return None;
            }
            agreed = Some(layout);
        }
        agreed
    }
    pub fn infer_modify_receiver_layout(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        target: &MethodIndexEntry,
        owners: &[String],
        bytecode: &dyn BytecodeIndex,
        index: &dyn ClassIndex,
        resolved: Option<&ExpressionContext>,
    ) -> Option<ModifyReceiverLayout> {
        let matches = infer_matches(
            source,
            site,
            target,
            owners,
            bytecode,
            index,
            resolved,
            engine::MatchContextType::ModifyReceiver,
        )?;
        let mut agreed = None;
        for (owner, m) in matches {
            let layout = modify_receiver_layout(&owner, &m)?;
            if agreed.as_ref().is_some_and(|a| a != &layout) {
                return None;
            }
            agreed = Some(layout);
        }
        agreed
    }
    pub fn infer_wrap_with_condition_layouts(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        target: &MethodIndexEntry,
        owners: &[String],
        bytecode: &dyn BytecodeIndex,
        index: &dyn ClassIndex,
        resolved: Option<&ExpressionContext>,
    ) -> Option<Vec<WrapWithConditionLayout>> {
        let mut matches = infer_matches(
            source,
            site,
            target,
            owners,
            bytecode,
            index,
            resolved,
            engine::MatchContextType::WrapWithCondition,
        )?;
        if let Some(ordinal) = site.at_ordinal {
            let ordinal = usize::try_from(ordinal).ok()?;
            let mut selected = vec![];
            for owner in MixinTargetResolver::resolve_targets(owners, index, None) {
                let mut indices: Vec<_> = matches
                    .iter()
                    .filter(|(o, _)| o == &owner)
                    .map(|(_, m)| m.original_instruction_index)
                    .collect();
                indices.sort();
                indices.dedup();
                let selected_index = *indices.get(ordinal)?;
                selected.extend(
                    matches
                        .iter()
                        .filter(|(o, m)| {
                            o == &owner && m.original_instruction_index == selected_index
                        })
                        .cloned(),
                );
            }
            matches = selected;
        }
        let layouts: Option<Vec<_>> = matches
            .iter()
            .map(|(owner, m)| wrap_condition_layout(owner, m, target, bytecode))
            .collect();
        layouts.filter(|v| !v.is_empty())
    }
    pub fn infer_from_expression(
        expression: &str,
        owner: &str,
        target: &MethodIndexEntry,
        bytecode: &dyn BytecodeIndex,
        index: &dyn ClassIndex,
    ) -> Option<String> {
        let expression = expression.trim();
        let expression = expression
            .strip_prefix("this.")
            .filter(|rest| rest.contains(['.', '(']))
            .unwrap_or(expression);
        if let Some(open) = expression.find('(') {
            if !expression.ends_with(')')
                || expression[open + 1..expression.len() - 1].contains(')')
            {
                return None;
            }
            let invocation = expression[..open].trim_end();
            if !invocation
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
            {
                return None;
            }
            let (receiver, name) = invocation
                .rsplit_once('.')
                .map_or((None, invocation), |(r, n)| (Some(r), n));
            if name.is_empty() {
                return None;
            }
            let receiver_owner = receiver.filter(|v| v.contains('.')).map(|r| {
                index
                    .find_class_by_fqn(r)
                    .map_or_else(|| r.replace('.', "/"), |c| c.internal_name)
            });
            bytecode
                .get_at_target_candidates(owner, &target.name, Some(&target.descriptor), "INVOKE")
                .into_iter()
                .find(|c| {
                    c.kind == AtTargetKind::Invoke
                        && c.name == name
                        && receiver_owner.as_ref().is_none_or(|o| &c.owner == o)
                })
                .and_then(|c| {
                    c.descriptor
                        .split_once(')')
                        .map(|v| v.1.to_string())
                        .filter(|v| !v.is_empty())
                })
        } else {
            let (receiver, name) = expression.rsplit_once('.')?;
            if receiver.contains('.')
                || !receiver
                    .chars()
                    .chain(name.chars())
                    .all(|c| c.is_alphanumeric() || c == '_')
            {
                return None;
            }
            bytecode
                .get_at_target_candidates(owner, &target.name, Some(&target.descriptor), "FIELD")
                .into_iter()
                .find(|c| c.kind == AtTargetKind::Field && c.name == name)
                .map(|c| c.descriptor)
                .or_else(|| {
                    index
                        .get_fields(owner)
                        .into_iter()
                        .find(|f| f.name == name)
                        .map(|f| f.descriptor)
                })
        }
    }
}
pub fn trim_expression_utf16(units: &[u16]) -> &[u16] {
    let whitespace = |u: u16| char::from_u32(u as u32).is_some_and(char::is_whitespace);
    let start = units
        .iter()
        .position(|u| !whitespace(*u))
        .unwrap_or(units.len());
    let end = units
        .iter()
        .rposition(|u| !whitespace(*u))
        .map_or(start, |i| i + 1);
    &units[start..end]
}
fn expression_values(
    context: &ExpressionContext,
    site: &MixinExtrasAnnotationSite,
) -> Vec<Vec<u16>> {
    context
        .values_for_at_id_utf16(site.at_id.as_deref())
        .into_iter()
        .map(|s| trim_expression_utf16(&s).to_vec())
        .filter(|s| !s.is_empty())
        .collect()
}
pub fn definition_inputs(index: &MixinExtrasDefinitionIndex) -> Vec<engine::DefinitionInput> {
    index
        .definitions
        .iter()
        .map(|d| engine::DefinitionInput {
            id: d.id.clone(),
            raw_method_references: d.raw_method_references.clone(),
            raw_field_references: d.raw_field_references.clone(),
            class_literal_type_names: d.class_literal_type_names.clone(),
            local_specs: d
                .local_specs
                .iter()
                .map(|l| engine::LocalSpec {
                    args_only: l.args_only,
                    index: l.index,
                    ordinal: l.ordinal,
                    names: l.names.clone(),
                    print: l.print,
                    type_class_name: l.type_class_name.clone(),
                })
                .collect(),
            parse_issues: d
                .parse_issues
                .iter()
                .map(|p| engine::DefinitionParseIssue {
                    attribute: p.attribute.clone(),
                    raw_value: p.raw_value.clone(),
                    message: p.message.clone(),
                    body_range: engine::SourceSpan {
                        start: p.body_range.start,
                        end: p.body_range.end,
                    },
                })
                .collect(),
            source_range: d.source_range.as_ref().map(|r| engine::SourceSpan {
                start: r.start,
                end: r.end,
            }),
        })
        .collect()
}
pub struct OfficialExpressionIdentifierPoolBuilder;
impl OfficialExpressionIdentifierPoolBuilder {
    pub fn build(
        index: &MixinExtrasDefinitionIndex,
        resolver: &ClassLiteralTypeNameResolver<'_>,
    ) -> engine::IdentifierPoolBuildResult {
        engine::build_identifier_pool(&definition_inputs(index), &|name| resolver.resolve(name))
    }
}
enum Matches {
    Available(Vec<(String, engine::ExpressionMatch)>),
    Unavailable,
    NoMatch,
}
fn official_matches(
    expressions: &[Vec<u16>],
    target: &MethodIndexEntry,
    owners: &[String],
    bytecode: &dyn BytecodeIndex,
    definitions: &MixinExtrasDefinitionIndex,
    context_type: engine::MatchContextType,
    resolver: &ClassLiteralTypeNameResolver,
) -> Matches {
    if owners.is_empty() {
        return Matches::Unavailable;
    }
    let pool = engine::build_identifier_pool(&definition_inputs(definitions), &|name| {
        resolver.resolve(name)
    })
    .pool;
    let common = |left: &str, right: &str| bytecode.resolve_common_super_class(left, right);
    let cancellation = engine::current_cancellation();
    let context = engine::MatchContext {
        context_type,
        identifier_pool: &pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &common,
        cancellation: cancellation.as_ref(),
    };
    let mut all = vec![];
    for owner in owners {
        let Some(bytes) = bytecode.get_class_bytes(owner) else {
            return Matches::Unavailable;
        };
        let result = match engine::match_expressions_utf16(
            &bytes,
            &target.name,
            &target.descriptor,
            expressions,
            &context,
        ) {
            Ok(value) => value,
            Err(cancel) => std::panic::panic_any(cancel),
        };
        match result {
            engine::MatchResult::Unavailable { .. } => return Matches::Unavailable,
            engine::MatchResult::Available(matches) => {
                if (0..expressions.len()).any(|i| !matches.iter().any(|m| m.expression_index == i))
                {
                    return Matches::NoMatch;
                }
                all.extend(matches.into_iter().map(|m| (owner.clone(), m)));
            }
        }
    }
    Matches::Available(all)
}
fn infer_matches(
    source: &str,
    site: &MixinExtrasAnnotationSite,
    target: &MethodIndexEntry,
    owners: &[String],
    bytecode: &dyn BytecodeIndex,
    index: &dyn ClassIndex,
    resolved: Option<&ExpressionContext>,
    kind: engine::MatchContextType,
) -> Option<Vec<(String, engine::ExpressionMatch)>> {
    let fallback;
    let context = if let Some(context) = resolved {
        context
    } else {
        fallback = ExpressionContextResolver::resolve_expression_context(source, site);
        &fallback
    };
    let values = expression_values(context, site);
    if values.is_empty() {
        return None;
    }
    let owners = MixinTargetResolver::resolve_targets(owners, index, None);
    match official_matches(
        &values,
        target,
        &owners,
        bytecode,
        &context.definition_index,
        kind,
        &ClassLiteralTypeNameResolver::for_source(source, index),
    ) {
        Matches::Available(matches) => Some(matches),
        _ => None,
    }
}
fn constraint_descriptor(c: &engine::TypeConstraint) -> String {
    match c {
        engine::TypeConstraint::Exact(d) => d.clone(),
        engine::TypeConstraint::IntLike => "I".into(),
    }
}
fn match_type_info(m: &engine::ExpressionMatch) -> ExpressionValueTypeInfo {
    let constraint = match m
        .decorations
        .get(&engine::DecorationKey::SimpleExpressionType)
    {
        Some(engine::DecorationValue::TypeConstraint(c)) => c,
        _ => &m.captured_type,
    };
    ExpressionValueTypeInfo {
        descriptor: constraint_descriptor(constraint),
        int_like: matches!(constraint, engine::TypeConstraint::IntLike),
    }
}
fn simple_operation_layout(m: &engine::ExpressionMatch) -> Option<SimpleOperationLayout> {
    let Some(engine::DecorationValue::TypeConstraints(args)) = m
        .decorations
        .get(&engine::DecorationKey::SimpleOperationArgs)
    else {
        return None;
    };
    let Some(engine::DecorationValue::ParamNames(names)) = m
        .decorations
        .get(&engine::DecorationKey::SimpleOperationParamNames)
    else {
        return None;
    };
    let Some(engine::DecorationValue::TypeConstraint(ret)) = m
        .decorations
        .get(&engine::DecorationKey::SimpleOperationReturnType)
    else {
        return None;
    };
    if args.len() != names.len() {
        return None;
    }
    let arguments: Vec<_> = args.iter().map(constraint_descriptor).collect();
    let ret_descriptor = constraint_descriptor(ret);
    if arguments
        .iter()
        .any(|d| descriptor::parse_field_descriptor(d).is_err())
        || ret_descriptor != "V" && descriptor::parse_field_descriptor(&ret_descriptor).is_err()
    {
        return None;
    }
    Some(SimpleOperationLayout {
        argument_descriptors: arguments,
        parameter_names: names.clone(),
        return_descriptor: ret_descriptor,
        argument_int_like: args
            .iter()
            .map(|c| matches!(c, engine::TypeConstraint::IntLike))
            .collect(),
        return_int_like: matches!(ret, engine::TypeConstraint::IntLike),
    })
}
fn modify_receiver_layout(
    owner: &str,
    m: &engine::ExpressionMatch,
) -> Option<ModifyReceiverLayout> {
    if owner.is_empty() {
        return None;
    }
    match m.instruction_metadata.as_ref()? {
        engine::InstructionMetadata::MethodInvocation {
            owner_internal_name,
            name,
            descriptor,
            ..
        } => {
            if owner_internal_name.is_empty() || name.is_empty() || name == "<init>" {
                return None;
            }
            let receiver = match m.original_instruction_opcode {
                0xb6 | 0xb9 => owner_internal_name.as_str(),
                0xb7 => owner,
                _ => return None,
            };
            let method = descriptor::parse_method_descriptor(descriptor).ok()?;
            Some(ModifyReceiverLayout {
                receiver_owner_internal_name: receiver.into(),
                parameter_descriptors: method.parameters.iter().map(descriptor::render).collect(),
            })
        }
        engine::InstructionMetadata::FieldAccess {
            owner_internal_name,
            name,
            descriptor,
        } => {
            if owner_internal_name.is_empty()
                || name.is_empty()
                || descriptor::parse_field_descriptor(descriptor).is_err()
            {
                return None;
            }
            Some(ModifyReceiverLayout {
                receiver_owner_internal_name: owner_internal_name.clone(),
                parameter_descriptors: match m.original_instruction_opcode {
                    0xb4 => vec![],
                    0xb5 => vec![descriptor.clone()],
                    _ => return None,
                },
            })
        }
        _ => None,
    }
}
fn wrap_condition_layout(
    owner: &str,
    m: &engine::ExpressionMatch,
    target: &MethodIndexEntry,
    bytecode: &dyn BytecodeIndex,
) -> Option<WrapWithConditionLayout> {
    let other = || WrapWithConditionLayout {
        owner_internal_name: String::new(),
        parameter_descriptors: vec![],
        is_static: true,
        target_kind: WrapWithConditionTargetKind::Other,
        result_classification: OccurrenceResultClassification::NotApplicable,
    };
    match &m.instruction_metadata {
        Some(engine::InstructionMetadata::MethodInvocation {
            owner_internal_name,
            name,
            descriptor,
            ..
        }) => {
            let parsed = descriptor::parse_method_descriptor(descriptor).ok()?;
            if owner_internal_name.is_empty() || name.is_empty() {
                return None;
            }
            if name == "<init>" {
                return Some(other());
            }
            let return_descriptor = descriptor::render(&parsed.return_type);
            let classification = if return_descriptor == "V" {
                OccurrenceResultClassification::Void
            } else {
                let bytes = bytecode.get_class_bytes(owner)?;
                bytecode::extract_instructions(&bytes, &target.name, &target.descriptor)
                    .ok()?
                    .into_iter()
                    .find(|c| {
                        c.instruction_occurrence_index == m.original_instruction_index as i32
                            && c.owner == *owner_internal_name
                            && c.name == *name
                            && c.descriptor == *descriptor
                            && c.kind == AtTargetKind::Invoke
                    })?
                    .occurrence_result_classification
            };
            Some(WrapWithConditionLayout {
                owner_internal_name: owner_internal_name.clone(),
                parameter_descriptors: parsed.parameters.iter().map(descriptor::render).collect(),
                is_static: m.original_instruction_opcode == 0xb8,
                target_kind: WrapWithConditionTargetKind::Invoke,
                result_classification: classification,
            })
        }
        Some(engine::InstructionMetadata::FieldAccess {
            owner_internal_name,
            name,
            descriptor,
        }) => {
            if owner_internal_name.is_empty()
                || name.is_empty()
                || descriptor::parse_field_descriptor(descriptor).is_err()
            {
                return None;
            }
            let put = matches!(m.original_instruction_opcode, 0xb3 | 0xb5);
            Some(WrapWithConditionLayout {
                owner_internal_name: owner_internal_name.clone(),
                parameter_descriptors: if put {
                    vec![descriptor.clone()]
                } else {
                    vec![]
                },
                is_static: matches!(m.original_instruction_opcode, 0xb2 | 0xb3),
                target_kind: WrapWithConditionTargetKind::Field,
                result_classification: if put {
                    OccurrenceResultClassification::Void
                } else {
                    OccurrenceResultClassification::Retained
                },
            })
        }
        _ => Some(other()),
    }
}
