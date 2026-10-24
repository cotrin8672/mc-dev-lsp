use super::*;
use crate::{
    expression_engine::{MemberCandidate, MemberKind},
    mixin_edits::MixinImportEditBuilder,
    mixin_source::*,
    McTextEdit,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpressionDefinitionEditPlanResult {
    Available {
        insert_text: String,
        additional_edits: Vec<McTextEdit>,
    },
    Unavailable {
        reason: String,
    },
}
pub struct ExpressionDefinitionEditPlanner;
impl ExpressionDefinitionEditPlanner {
    pub fn plan(
        context: &AnnotationContext,
        annotation_offset: isize,
        source: &str,
        index: &MixinExtrasDefinitionIndex,
        candidate: &MemberCandidate,
    ) -> ExpressionDefinitionEditPlanResult {
        use ExpressionDefinitionEditPlanResult::*;
        let unavailable = |reason: &str| Unavailable {
            reason: reason.into(),
        };
        if !matches!(
            context.annotation,
            MixinAnnotation::Expression | MixinAnnotation::Expressions
        ) {
            return unavailable("annotation is not Expression");
        }
        if context.slot != AnnotationSlot::Value {
            return unavailable("expression completion context is not in VALUE slot");
        }
        if context.expression_completion_position != Some(ExpressionCompletionPosition::AfterDot) {
            return unavailable("expression is not in AFTER_DOT completion state");
        }
        if annotation_offset < 0 || annotation_offset as usize >= source.len() {
            return unavailable("invalid expression annotation offset");
        }
        let annotation_offset = annotation_offset as usize;
        if source.as_bytes()[annotation_offset] != b'@' {
            return unavailable("expression annotation offset does not point at @");
        }
        if annotation_offset != context.annotation_start_offset {
            return unavailable("expression annotation offset mismatch");
        }
        let Some(selector) = Self::canonical_exact_selector(candidate) else {
            return unavailable("candidate cannot be represented as exact selector");
        };
        let reused = Self::find_reusable_definition_id(index, candidate.kind, &selector);
        let id = reused
            .clone()
            .unwrap_or_else(|| Self::allocate_definition_id(index, &candidate.name));
        let insert_text = if candidate.kind == MemberKind::Method {
            format!("{id}()")
        } else {
            id.clone()
        };
        if reused.is_some() {
            return Available {
                insert_text,
                additional_edits: vec![],
            };
        }
        let line_start = source[..annotation_offset].rfind('\n').map_or(0, |n| n + 1);
        let indent_end = source[line_start..annotation_offset]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map_or(annotation_offset, |(n, _)| line_start + n);
        let indent = &source[line_start..indent_end];
        let newline = line_ending(source, annotation_offset);
        let attribute = if candidate.kind == MemberKind::Field {
            "field"
        } else {
            "method"
        };
        let text =
            format!("@Definition(id = \"{id}\", {attribute} = \"{selector}\"){newline}{indent}");
        let mut edits = vec![McTextEdit {
            start_offset: annotation_offset,
            end_offset: annotation_offset,
            new_text: text,
        }];
        if let Some(import) = MixinImportEditBuilder::build_import_edit(
            source,
            "com.llamalad7.mixinextras.expression.Definition",
        ) {
            edits.push(import);
        }
        edits.sort_by_key(|e| e.start_offset);
        for (i, current) in edits.iter().enumerate() {
            for other in &edits[i + 1..] {
                if current.start_offset == other.start_offset
                    && current.end_offset == other.end_offset
                    || current.end_offset > other.start_offset
                {
                    return unavailable("planned additional edits overlap or collide");
                }
            }
        }
        Available {
            insert_text,
            additional_edits: edits,
        }
    }
    pub fn canonical_exact_selector(candidate: &MemberCandidate) -> Option<String> {
        if candidate.owner_internal_name.trim().is_empty()
            || candidate.name.trim().is_empty()
            || candidate.descriptor.trim().is_empty()
        {
            return None;
        }
        Some(match candidate.kind {
            MemberKind::Field => format!(
                "L{};{}:{}",
                candidate.owner_internal_name, candidate.name, candidate.descriptor
            ),
            MemberKind::Method => format!(
                "L{};{}{}",
                candidate.owner_internal_name, candidate.name, candidate.descriptor
            ),
        })
    }
    pub fn find_reusable_definition_id(
        index: &MixinExtrasDefinitionIndex,
        kind: MemberKind,
        selector: &str,
    ) -> Option<String> {
        let mut ids = vec![];
        for d in &index.definitions {
            let Some(id) = &d.id else {
                continue;
            };
            let refs = if kind == MemberKind::Field {
                &d.raw_field_references
            } else {
                &d.raw_method_references
            };
            if refs.iter().any(|r| r == selector) && !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        if ids.len() != 1 {
            return None;
        }
        let id = ids.pop()?;
        (index
            .definitions
            .iter()
            .filter(|d| d.id.as_ref() == Some(&id))
            .count()
            == 1)
            .then_some(id)
    }
    pub fn allocate_definition_id(index: &MixinExtrasDefinitionIndex, name: &str) -> String {
        if name.is_empty()
            || !name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return "member".into();
        }
        let mut candidate = name.to_string();
        let mut suffix = 2;
        while index
            .definitions
            .iter()
            .any(|d| d.id.as_deref() == Some(candidate.as_str()))
        {
            candidate = format!("{name}{suffix}");
            suffix += 1;
        }
        candidate
    }
}
fn line_ending(source: &str, offset: usize) -> &'static str {
    if let Some(n) = source[..offset].rfind('\n') {
        if n > 0 && source.as_bytes()[n - 1] == b'\r' {
            return "\r\n";
        }
    }
    let bytes = source.as_bytes();
    let mut index = offset.saturating_sub(256);
    let end = (offset + 256).min(bytes.len());
    let (mut crlf, mut lf) = (0, 0);
    while index < end {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            crlf += 1;
            index += 2;
        } else {
            if bytes[index] == b'\n' {
                lf += 1;
            }
            index += 1;
        }
    }
    if crlf > lf {
        "\r\n"
    } else {
        "\n"
    }
}
