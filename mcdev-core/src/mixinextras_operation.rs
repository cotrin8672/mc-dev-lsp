use crate::descriptor::{parse_field_descriptor, render};
use crate::extras_model::{
    HandlerMethodDeclaration, HandlerSignatureSpec, MixinExtrasAnnotation,
    MixinExtrasAnnotationSite,
};
use crate::mixin_source::{matching, source_range, source_tokens, SourceTokenKind};
use crate::ClassIndex;
use crate::{position_to_offset, McTextRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationCallIssue {
    pub argument_range: McTextRange,
    pub expected_names: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationCallExpectedArg {
    pub name: String,
    pub descriptor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationCallValidationLayout {
    pub operation_parameter_name: String,
    pub expected_call_args: Vec<OperationCallExpectedArg>,
    pub handler_parameter_descriptors: BTreeMap<String, Option<String>>,
    pub handler_range: McTextRange,
}
impl OperationCallValidationLayout {
    pub fn expected_call_arg_names(&self) -> Vec<String> {
        self.expected_call_args
            .iter()
            .map(|a| a.name.clone())
            .collect()
    }
}
pub trait OperationCallArgumentTypeResolver {
    fn resolve_type(&self, expression: &str) -> Option<String>;
}
impl<F: Fn(&str) -> Option<String>> OperationCallArgumentTypeResolver for F {
    fn resolve_type(&self, expression: &str) -> Option<String> {
        self(expression)
    }
}
pub struct OperationCallValidator;
impl OperationCallValidator {
    pub fn resolve_operation_call_layout(
        site: &MixinExtrasAnnotationSite,
        handler: &HandlerMethodDeclaration,
        expected: Option<&HandlerSignatureSpec>,
    ) -> Option<OperationCallValidationLayout> {
        let expected = expected?;
        if !matches!(
            site.annotation,
            MixinExtrasAnnotation::WrapOperation | MixinExtrasAnnotation::WrapMethod
        ) {
            return None;
        }
        let operations = handler
            .parameters
            .iter()
            .filter(|p| p.is_operation)
            .collect::<Vec<_>>();
        if operations.len() != 1 {
            return None;
        }
        if let Some(first) = handler.parameters.iter().position(|p| p.is_sugar) {
            if handler.parameters[first + 1..].iter().any(|p| !p.is_sugar) {
                return None;
            }
        }
        let non_sugar = handler
            .parameters
            .iter()
            .filter(|p| !p.is_sugar)
            .collect::<Vec<_>>();
        let operation_index = non_sugar.iter().position(|p| p.is_operation)?;
        let expected_operation_index = expected.parameters.iter().rposition(|p| p.is_operation)?;
        match site.annotation {
            MixinExtrasAnnotation::WrapMethod => {
                if operation_index + 1 != non_sugar.len()
                    || non_sugar.len() != expected.parameters.len()
                    || operation_index != expected_operation_index
                {
                    return None;
                }
            }
            MixinExtrasAnnotation::WrapOperation => {
                if non_sugar.len() < expected.parameters.len()
                    || operation_index + 1 != expected.parameters.len()
                    || !non_sugar[expected.parameters.len() - 1].is_operation
                {
                    return None;
                }
            }
            _ => return None,
        }
        let args = &non_sugar[..operation_index];
        if args.len() != expected.operation_call_args.len()
            || args.iter().any(|p| !java_identifier(&p.name))
        {
            return None;
        }
        Some(OperationCallValidationLayout {
            operation_parameter_name: operations[0].name.clone(),
            expected_call_args: args
                .iter()
                .map(|p| OperationCallExpectedArg {
                    name: p.name.clone(),
                    descriptor: p.type_descriptor.clone(),
                })
                .collect(),
            handler_parameter_descriptors: non_sugar
                .iter()
                .map(|p| (p.name.clone(), p.type_descriptor.clone()))
                .collect(),
            handler_range: handler.range,
        })
    }
    pub fn validate(
        source: &str,
        site: &MixinExtrasAnnotationSite,
        layout: Option<&OperationCallValidationLayout>,
        argument_type_resolver: Option<&dyn OperationCallArgumentTypeResolver>,
    ) -> Vec<OperationCallIssue> {
        let Some(layout) = layout.filter(|_| {
            site.handler_method.is_some()
                && matches!(
                    site.annotation,
                    MixinExtrasAnnotation::WrapOperation | MixinExtrasAnnotation::WrapMethod
                )
        }) else {
            return vec![];
        };
        let Some(header_end) = position_to_offset(source, layout.handler_range.end) else {
            return vec![];
        };
        let tokens = source_tokens(source, false);
        let Some(start) = tokens.iter().position(|t| {
            t.start >= header_end
                && t.kind == SourceTokenKind::Symbol
                && matches!(t.text.as_str(), "{" | ";")
        }) else {
            return vec![];
        };
        if tokens[start].text == ";" {
            return vec![];
        }
        let Some(body_end) = matching(&tokens, start) else {
            return vec![];
        };
        let mut i = start + 1;
        let mut issues = vec![];
        while i < body_end {
            if tokens[i].text == "@" {
                let mut end = i + 1;
                while end < body_end
                    && (tokens[end].kind == SourceTokenKind::Identifier || tokens[end].text == ".")
                {
                    end += 1;
                }
                if tokens.get(end).is_some_and(|t| t.text == "(")
                    && tokens[end - 1].end == tokens[end].start
                {
                    if let Some(close) = matching(&tokens, end) {
                        i = close + 1;
                        continue;
                    }
                }
                i = end;
                continue;
            }
            if tokens[i].kind == SourceTokenKind::Identifier
                && tokens[i].text == layout.operation_parameter_name
                && unqualified_receiver(source, tokens[i].start)
                && tokens.get(i + 1).is_some_and(|t| t.text == ".")
                && tokens.get(i + 2).is_some_and(|t| t.text == "call")
                && tokens.get(i + 3).is_some_and(|t| t.text == "(")
                && tokens[i..i + 4]
                    .windows(2)
                    .all(|pair| source[pair[0].end..pair[1].start].trim().is_empty())
            {
                let open = i + 3;
                if let Some(close) = matching(&tokens, open).filter(|c| *c < body_end) {
                    let content_start = tokens[open].end;
                    let content_end = tokens[close].start;
                    let args = split_call_arguments(&source[content_start..content_end]);
                    let mut incompatible = false;
                    if args.len() == layout.expected_call_args.len() {
                        for (expected, actual) in layout.expected_call_args.iter().zip(&args) {
                            let Some(expected) = expected.descriptor.as_deref() else {
                                continue;
                            };
                            let actual_descriptor = if java_identifier(actual) {
                                layout
                                    .handler_parameter_descriptors
                                    .get(*actual)
                                    .cloned()
                                    .flatten()
                            } else {
                                argument_type_resolver.and_then(|r| r.resolve_type(actual))
                            };
                            if actual_descriptor
                                .as_deref()
                                .is_some_and(|actual| descriptor_incompatible(expected, actual))
                            {
                                incompatible = true;
                            }
                        }
                    }
                    if args.len() != layout.expected_call_args.len() || incompatible {
                        let content = &source[content_start..content_end];
                        let trimmed = content.trim();
                        let (first, last) = if trimmed.is_empty() {
                            (content_start, content_start)
                        } else {
                            let first = content_start + content.len() - content.trim_start().len();
                            (first, first + trimmed.len())
                        };
                        issues.push(OperationCallIssue {
                            argument_range: source_range(source, first, last),
                            expected_names: layout.expected_call_arg_names(),
                        });
                    }
                    i = close + 1;
                    continue;
                }
            }
            i += 1;
        }
        issues
    }
}
fn java_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}
fn unqualified_receiver(source: &str, start: usize) -> bool {
    source[..start]
        .trim_end()
        .chars()
        .next_back()
        .is_none_or(|c| !matches!(c, '.' | ')' | ']' | '>' | '('))
}
fn primitive(descriptor: &str) -> bool {
    descriptor.len() == 1
        && matches!(
            descriptor.as_bytes()[0],
            b'Z' | b'B' | b'C' | b'S' | b'I' | b'J' | b'F' | b'D'
        )
}
fn wrapper(descriptor: &str) -> Option<&'static str> {
    Some(match descriptor {
        "Z" => "Ljava/lang/Boolean;",
        "B" => "Ljava/lang/Byte;",
        "C" => "Ljava/lang/Character;",
        "S" => "Ljava/lang/Short;",
        "I" => "Ljava/lang/Integer;",
        "J" => "Ljava/lang/Long;",
        "F" => "Ljava/lang/Float;",
        "D" => "Ljava/lang/Double;",
        _ => return None,
    })
}
fn descriptor_incompatible(expected: &str, actual: &str) -> bool {
    if expected == actual || wrapper(expected) == Some(actual) || wrapper(actual) == Some(expected)
    {
        return false;
    }
    let family = |value: &str| primitive(value) || (value.starts_with('L') && value.ends_with(';'));
    family(expected) || family(actual)
}
fn generic_pair(value: &str, open: usize, close: usize) -> bool {
    let before = value[..open].trim_end();
    let Some(ch) = before.chars().next_back() else {
        return false;
    };
    if matches!(ch, '.' | ',' | '?' | '>') {
        return true;
    }
    if ch.is_alphanumeric() || ch == '_' || ch == '$' {
        let word = before
            .rsplit(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
            .next()
            .unwrap_or("");
        if word == "new" {
            return true;
        }
        if before.len() != open {
            return false;
        }
        return value[close + 1..]
            .trim_start()
            .chars()
            .next()
            .is_none_or(|c| matches!(c, '(' | ')' | '.' | '[' | ';'));
    }
    false
}
fn has_generic_close(value: &str, open: usize) -> bool {
    let tokens = source_tokens(value, false);
    let mut depth = 0;
    for t in tokens
        .iter()
        .filter(|t| t.start >= open && t.kind == SourceTokenKind::Symbol)
    {
        if t.text == "<" {
            depth += 1;
        } else if t.text == ">" {
            depth -= 1;
            if depth == 0 {
                return generic_pair(value, open, t.start);
            }
        }
    }
    false
}
pub fn split_call_arguments(value: &str) -> Vec<&str> {
    let tokens = source_tokens(value, false);
    let (mut angle, mut paren, mut brace, mut bracket) = (0i32, 0i32, 0i32, 0i32);
    let mut start = 0;
    let mut result = vec![];
    for t in tokens
        .into_iter()
        .filter(|t| t.kind == SourceTokenKind::Symbol)
    {
        match t.text.as_str() {
            "<" if has_generic_close(value, t.start) => angle += 1,
            ">" if angle > 0 => angle -= 1,
            "(" => paren += 1,
            ")" => paren = (paren - 1).max(0),
            "{" => brace += 1,
            "}" => brace = (brace - 1).max(0),
            "[" => bracket += 1,
            "]" => bracket = (bracket - 1).max(0),
            "," if angle == 0 && paren == 0 && brace == 0 && bracket == 0 => {
                let segment = value[start..t.start].trim();
                if !segment.is_empty() {
                    result.push(segment);
                }
                start = t.end;
            }
            _ => {}
        }
    }
    let segment = value[start..].trim();
    if !segment.is_empty() {
        result.push(segment);
    }
    result
}

pub struct OperationSignatureRenderer;
impl OperationSignatureRenderer {
    pub fn render_operation_type(return_type_descriptor: &str) -> String {
        let generic = match return_type_descriptor {
            "B" => "Byte",
            "C" => "Character",
            "D" => "Double",
            "F" => "Float",
            "I" => "Integer",
            "J" => "Long",
            "S" => "Short",
            "Z" => "Boolean",
            "V" => "Void",
            other => return format!("Operation<{}>", Self::readable_type(other)),
        };
        format!("Operation<{generic}>")
    }
    pub fn readable_type(descriptor: &str) -> String {
        if descriptor == "V" {
            "void".into()
        } else {
            parse_field_descriptor(descriptor)
                .map(|v| render(&v))
                .unwrap_or_else(|_| descriptor.into())
        }
    }
    pub fn descriptor_from_type_name(
        type_name: &str,
        class_index: &dyn ClassIndex,
    ) -> Option<String> {
        let name = type_name.trim();
        if name.is_empty() {
            return None;
        }
        let primitive = match name {
            "byte" => Some("B"),
            "char" => Some("C"),
            "double" => Some("D"),
            "float" => Some("F"),
            "int" => Some("I"),
            "long" => Some("J"),
            "short" => Some("S"),
            "boolean" => Some("Z"),
            "void" => Some("V"),
            _ => None,
        };
        if let Some(value) = primitive {
            return Some(value.into());
        }
        let fqn = name.strip_suffix("[]").unwrap_or(name);
        let array = if name.ends_with("[]") { "[" } else { "" };
        let entry = class_index.find_class_by_fqn(fqn).or_else(|| {
            let simple = fqn.rsplit('.').next().unwrap_or(fqn);
            let mut matches = class_index
                .find_classes(simple, 10)
                .into_iter()
                .filter(|c| c.simple_name == simple);
            let first = matches.next()?;
            if matches.next().is_some() {
                None
            } else {
                Some(first)
            }
        })?;
        Some(format!("{array}L{};", entry.internal_name))
    }
}
