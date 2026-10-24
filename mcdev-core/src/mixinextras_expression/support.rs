use super::*;
use crate::{
    expression_engine as engine, extras_model::*, mixin_edits::MixinImportEditBuilder,
    mixin_source::*, *,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FeatureAnnotationCompletion {
    pub items: Vec<McCompletionItem>,
    pub replacement_range: Option<McCompletionReplacementRange>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeatureSnippet {
    trigger: String,
    label: String,
    insert_text: String,
    imports: Vec<String>,
    detail: String,
    source: String,
}
#[derive(Default)]
pub struct ExpressionSupport<'a> {
    pub member_completion_service: Option<&'a ExpressionMemberCompletionService<'a>>,
    pub class_index: Option<&'a dyn ClassIndex>,
}
impl<'a> ExpressionSupport<'a> {
    pub fn new(
        member_completion_service: Option<&'a ExpressionMemberCompletionService<'a>>,
        class_index: Option<&'a dyn ClassIndex>,
    ) -> Self {
        Self {
            member_completion_service,
            class_index,
        }
    }
    pub fn complete_at_value(&self, context: &AnnotationContext) -> Vec<McCompletionItem> {
        if context.annotation != MixinAnnotation::At
            || context.slot != AnnotationSlot::Value
            || context.parent_injector_annotation == Some(MixinAnnotation::ModifyReturnValue)
        {
            return vec![];
        }
        let value = "MIXINEXTRAS:EXPRESSION";
        if !starts_ignore_case(value, context.partial_value.trim_matches('"')) {
            return vec![];
        }
        vec![item(
            value,
            "MixinExtras expression injection point",
            "Use @Definition and @Expression annotations to define expression slices",
            value,
            value,
            McCompletionKind::Value,
            &format!("0310_{value}"),
            "mixinextras.expressionAtValue",
            value,
        )]
    }
    pub fn complete_expression_annotations(
        &self,
        context: &AnnotationContext,
    ) -> Vec<McCompletionItem> {
        self.feature_snippets(context.partial_value.trim(), "")
    }
    pub fn complete_expression_value(
        &self,
        context: &AnnotationContext,
        source: &str,
        resolved: &[ResolvedMixinExtrasContext],
        owners: &[String],
        cancellation: &dyn engine::CancellationChecker,
    ) -> Result<Vec<McCompletionItem>, engine::Cancellation> {
        if !matches!(
            context.annotation,
            MixinAnnotation::Expression | MixinAnnotation::Expressions
        ) || context.slot != AnnotationSlot::Value
        {
            return Ok(vec![]);
        }
        let Some(position) = context.expression_completion_position else {
            return Ok(vec![]);
        };
        let prefix = &context.partial_value;
        let keywords: &[&str] = match position {
            ExpressionCompletionPosition::StatementStart => &[
                "return", "throw", "this", "super", "true", "false", "null", "new",
            ],
            ExpressionCompletionPosition::ValueStart => {
                &["this", "super", "true", "false", "null", "new"]
            }
            ExpressionCompletionPosition::AfterMethodReference => &["new"],
            _ => &[],
        };
        let mut items: Vec<_> = keywords
            .iter()
            .filter(|k| starts_ignore_case(k, prefix))
            .enumerate()
            .map(|(i, k)| {
                item(
                    k,
                    "Expression keyword",
                    k,
                    k,
                    k,
                    McCompletionKind::Keyword,
                    &format!("0320_{i:02}_{k}"),
                    "mixinextras.expressionValue",
                    k,
                )
            })
            .collect();
        if matches!(
            position,
            ExpressionCompletionPosition::StatementStart | ExpressionCompletionPosition::ValueStart
        ) && !source.is_empty()
        {
            if let Some(site) =
                ExpressionContextResolver::find_enclosing_site(source, context.value_start_offset)
            {
                if let Some(expression_context) =
                    ExpressionContextResolver::resolve_expression_context_for_site(
                        source, &site, resolved,
                    )
                {
                    let mut seen = BTreeSet::new();
                    let count = items.len();
                    for id in expression_context
                        .definition_index
                        .definitions
                        .iter()
                        .filter_map(|d| d.id.as_ref())
                        .filter(|id| seen.insert((*id).clone()))
                        .filter(|id| starts_ignore_case(id, prefix))
                    {
                        let n = items.len() - count;
                        items.push(item(
                            id,
                            "Definition ID",
                            id,
                            id,
                            id,
                            McCompletionKind::Value,
                            &format!("0321_{:02}_{id}", count + n),
                            "mixinextras.definitionId",
                            id,
                        ));
                    }
                }
            }
        }
        if position == ExpressionCompletionPosition::AfterDot {
            items.extend(self.after_dot(context, source, resolved, owners, cancellation)?);
        }
        Ok(items)
    }
    pub fn complete_feature_annotations(
        &self,
        source: &str,
        line: usize,
        character: usize,
    ) -> Vec<McCompletionItem> {
        self.complete_feature_annotations_with_range(source, line, character)
            .items
    }
    pub fn complete_feature_annotations_with_range(
        &self,
        source: &str,
        line: usize,
        character: usize,
    ) -> FeatureAnnotationCompletion {
        let Some(offset) = position_to_offset(source, McTextPosition { line, character }) else {
            return FeatureAnnotationCompletion::default();
        };
        let start = source[..offset].rfind('\n').map_or(0, |n| n + 1);
        let masked = AnnotationContextExtractor::mask_non_code(source);
        let before = &masked[start..offset];
        let Some(marker) = before.rfind('@') else {
            return FeatureAnnotationCompletion::default();
        };
        let partial = &before[marker + 1..];
        if partial
            .chars()
            .any(|c| c.is_whitespace() || "().".contains(c))
        {
            return FeatureAnnotationCompletion::default();
        }
        let end = offset
            + masked[offset..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .map(char::len_utf8)
                .sum::<usize>();
        if masked[end..].trim_start().starts_with('(') {
            return FeatureAnnotationCompletion::default();
        }
        let items = self.feature_snippets(partial, source);
        if items.is_empty() {
            return FeatureAnnotationCompletion::default();
        }
        FeatureAnnotationCompletion {
            items,
            replacement_range: Some(McCompletionReplacementRange {
                start_offset: start + marker + 1,
                end_offset: end,
            }),
        }
    }
    pub fn is_expression_at_value(value: Option<&str>) -> bool {
        value.is_some_and(|v| v.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION"))
    }
    fn after_dot(
        &self,
        context: &AnnotationContext,
        source: &str,
        resolved: &[ResolvedMixinExtrasContext],
        owners: &[String],
        cancellation: &dyn engine::CancellationChecker,
    ) -> Result<Vec<McCompletionItem>, engine::Cancellation> {
        let Some(service) = self.member_completion_service else {
            return Ok(vec![]);
        };
        let ExpressionMemberCompletionServiceResult::Available { candidates } =
            service.complete_members(source, context, owners, resolved, cancellation)?
        else {
            return Ok(vec![]);
        };
        if source.is_empty() {
            return Ok(vec![]);
        }
        let Some(site) =
            ExpressionContextResolver::find_enclosing_site(source, context.value_start_offset)
        else {
            return Ok(vec![]);
        };
        let Some(expression_context) =
            ExpressionContextResolver::resolve_expression_context_for_site(source, &site, resolved)
        else {
            return Ok(vec![]);
        };
        let mut seen = BTreeSet::new();
        let mut items = vec![];
        for candidate in candidates {
            let key = (
                candidate.kind,
                candidate.owner_internal_name.clone(),
                candidate.name.clone(),
                candidate.descriptor.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            let ExpressionDefinitionEditPlanResult::Available {
                insert_text,
                additional_edits,
            } = ExpressionDefinitionEditPlanner::plan(
                context,
                context.annotation_start_offset as isize,
                source,
                &expression_context.definition_index,
                &candidate,
            )
            else {
                continue;
            };
            let field = candidate.kind == engine::MemberKind::Field;
            let label = if field {
                format!("{}:{}", candidate.name, candidate.descriptor)
            } else {
                format!("{}{}", candidate.name, candidate.descriptor)
            };
            let mut value = item(
                &label,
                &candidate.owner_internal_name,
                &candidate.descriptor,
                &format!("{} {}", candidate.name, candidate.descriptor),
                &insert_text,
                if field {
                    McCompletionKind::Field
                } else {
                    McCompletionKind::Method
                },
                &format!(
                    "0322_{}_{}_{}",
                    if field { 0 } else { 1 },
                    candidate.name,
                    candidate.descriptor
                ),
                "mixinextras.expressionMember",
                &candidate.name,
            );
            value.metadata.owner = Some(candidate.owner_internal_name);
            value.metadata.descriptor = Some(candidate.descriptor);
            value.additional_edits = additional_edits;
            items.push(value);
        }
        items.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
        Ok(items)
    }
    fn feature_snippets(&self, partial: &str, source: &str) -> Vec<McCompletionItem> {
        let mut snippets: Vec<FeatureSnippet> =
            serde_json::from_str(include_str!("feature_snippets.json"))
                .expect("static feature snippets");
        for snippet in &mut snippets {
            let mut imports = vec![];
            for name in &snippet.imports {
                if let Some(annotation) = name.strip_prefix("@Mixin:") {
                    let normalized = annotation.replace('_', "").to_ascii_lowercase();
                    if let Some(a) = MixinAnnotation::ALL
                        .into_iter()
                        .find(|a| a.simple_name().to_ascii_lowercase() == normalized)
                    {
                        imports.extend(a.official_fqns().iter().map(|s| s.to_string()));
                    }
                } else {
                    imports.push(name.clone());
                }
            }
            snippet.imports = imports;
        }
        if let Some(index) = self.class_index {
            for fqn in MixinAnnotation::WrapWithCondition.official_fqns() {
                if index.find_class_by_fqn(&fqn).is_some() {
                    snippets.push(FeatureSnippet{trigger:"wrapwithcondition".into(),label:format!("WrapWithCondition ({})",if fqn.ends_with(".v2.WrapWithCondition"){"v2"}else{"v1"}),insert_text:"WrapWithCondition(method = \"${1}\", at = @At(value = \"INVOKE\", target = \"${2}\"))$0".into(),imports:std::iter::once(fqn.to_string()).chain(MixinAnnotation::At.official_fqns().iter().map(|s|s.to_string())).collect(),detail:"MixinExtras feature".into(),source:"mixinextras.feature".into()});
                }
            }
        }
        snippets
            .into_iter()
            .filter(|s| {
                starts_ignore_case(&s.trigger, partial) || starts_ignore_case(&s.label, partial)
            })
            .map(|snippet| {
                let additional: BTreeSet<_> = snippet
                    .imports
                    .iter()
                    .map(|f| f.replace('.', "/"))
                    .collect();
                let mut references: Vec<_> = snippet
                    .imports
                    .iter()
                    .map(|f| {
                        (
                            f.clone(),
                            MixinImportEditBuilder::reference_for_internal_name(
                                source,
                                &f.replace('.', "/"),
                                &additional,
                            ),
                        )
                    })
                    .collect();
                let mut text = snippet.insert_text.clone();
                references
                    .sort_by_key(|(f, _)| std::cmp::Reverse(f.rsplit('.').next().unwrap().len()));
                for (f, reference) in &references {
                    text =
                        replace_identifier(&text, f.rsplit('.').next().unwrap(), &reference.text);
                }
                let mut seen = BTreeSet::new();
                let mut edits: Vec<McTextEdit> = vec![];
                if !source.is_empty() {
                    for fqn in snippet
                        .imports
                        .iter()
                        .filter_map(|f| {
                            references
                                .iter()
                                .find(|(name, _)| name == f)
                                .and_then(|(_, r)| r.import_fqn.as_ref())
                        })
                        .filter(|f| seen.insert((*f).clone()))
                    {
                        if let Some(edit) = MixinImportEditBuilder::build_import_edit(source, fqn) {
                            if let Some(previous) = edits.iter_mut().find(|e| {
                                e.start_offset == edit.start_offset
                                    && e.end_offset == edit.end_offset
                            }) {
                                previous.new_text.push_str(
                                    edit.new_text.strip_prefix('\n').unwrap_or(&edit.new_text),
                                );
                            } else {
                                edits.push(edit);
                            }
                        }
                    }
                }
                let mut result = item(
                    &snippet.label,
                    &snippet.detail,
                    &text,
                    &format!("{} {}", snippet.trigger, snippet.label),
                    &text,
                    McCompletionKind::Keyword,
                    &format!("0311_{}", snippet.trigger),
                    &snippet.source,
                    &snippet.trigger,
                );
                result.metadata.owner = snippet.imports.first().map(|f| f.replace('.', "/"));
                result.additional_edits = edits;
                result.insert_text_format = McCompletionInsertTextFormat::Snippet;
                result
            })
            .collect()
    }
}
fn starts_ignore_case(value: &str, prefix: &str) -> bool {
    value.to_lowercase().starts_with(&prefix.to_lowercase())
}
fn item(
    label: &str,
    detail: &str,
    documentation: &str,
    filter: &str,
    insert: &str,
    kind: McCompletionKind,
    sort: &str,
    source: &str,
    name: &str,
) -> McCompletionItem {
    McCompletionItem {
        label: label.into(),
        detail: Some(detail.into()),
        documentation: Some(documentation.into()),
        filter_text: filter.into(),
        insert_text: insert.into(),
        kind,
        sort_key: sort.into(),
        metadata: McCompletionMetadata {
            source: source.into(),
            name: Some(name.into()),
            ..Default::default()
        },
        additional_edits: vec![],
        insert_text_format: McCompletionInsertTextFormat::PlainText,
    }
}
fn replace_identifier(text: &str, name: &str, replacement: &str) -> String {
    if name == replacement {
        return text.into();
    }
    let mut result = String::new();
    let mut previous = 0;
    for (at, _) in text.match_indices(name) {
        let end = at + name.len();
        let before = text[..at].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
            || after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        result.push_str(&text[previous..at]);
        result.push_str(replacement);
        previous = end;
    }
    result.push_str(&text[previous..]);
    result
}
