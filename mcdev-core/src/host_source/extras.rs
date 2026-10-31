use super::*;
use crate::extras_model::*;
use crate::McTextRange;
use std::ops::Range;

const EXPRESSION: &str = "com.llamalad7.mixinextras.expression.Expression";
const EXPRESSIONS: &str = "com.llamalad7.mixinextras.expression.Expressions";
const DEFINITION: &str = "com.llamalad7.mixinextras.expression.Definition";
const DEFINITIONS: &str = "com.llamalad7.mixinextras.expression.Definitions";
const LOCAL: &str = "com.llamalad7.mixinextras.sugar.Local";
const HANDLERS: [&str; 13] = [
    "com.llamalad7.mixinextras.injector.ModifyExpressionValue",
    "com.llamalad7.mixinextras.injector.ModifyReturnValue",
    "com.llamalad7.mixinextras.injector.ModifyReceiver",
    "com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation",
    "com.llamalad7.mixinextras.injector.WrapWithCondition",
    "com.llamalad7.mixinextras.injector.v2.WrapWithCondition",
    "com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod",
    "org.spongepowered.asm.mixin.injection.Inject",
    "org.spongepowered.asm.mixin.injection.Redirect",
    "org.spongepowered.asm.mixin.injection.ModifyArg",
    "org.spongepowered.asm.mixin.injection.ModifyArgs",
    "org.spongepowered.asm.mixin.injection.ModifyVariable",
    "org.spongepowered.asm.mixin.injection.ModifyConstant",
];
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostResolvedContextResult {
    NotHandler,
    Resolved(ResolvedMixinExtrasContext),
    Unavailable { reason: String },
}
fn unavailable(reason: &str) -> HostResolvedContextResult {
    HostResolvedContextResult::Unavailable {
        reason: reason.into(),
    }
}
fn simple(a: &HostAnnotationFact) -> &str {
    a.source_name.rsplit('.').next().unwrap_or("")
}
/// Projects already resolved annotations/constants; it never resolves source names
/// or parses language syntax independently of the host.
pub fn extract_resolved_extras_context(
    annotations: &[HostAnnotationFact],
    handler_range: McTextRange,
) -> HostResolvedContextResult {
    let mut has_handler = false;
    for annotation in annotations {
        if !HANDLERS
            .iter()
            .any(|f| f.rsplit('.').next() == Some(simple(annotation)))
        {
            continue;
        }
        match annotation.qualified_name.as_deref() {
            None => return unavailable("unresolved handler annotation binding"),
            Some(fqn) if HANDLERS.contains(&fqn) => {
                if !annotation.validity.is_usable() {
                    return unavailable("recovered or malformed handler annotation");
                }
                has_handler = true;
            }
            _ => {}
        }
    }
    if !has_handler {
        return HostResolvedContextResult::NotHandler;
    }
    let mut expressions = vec![];
    let mut definitions = vec![];
    for annotation in annotations {
        match annotation.qualified_name.as_deref() {
            Some(EXPRESSION) => match expression(annotation) {
                Some(e) => expressions.push(e),
                None => return unavailable("unresolved @Expression member"),
            },
            Some(EXPRESSIONS) => {
                if !annotation.validity.is_usable() || !annotation.has_unique_members() {
                    return unavailable("unresolved @Expressions member");
                }
                let Some(nested) = annotation
                    .member("value")
                    .and_then(HostExpressionFact::nested_annotations)
                else {
                    return unavailable("unresolved @Expressions member");
                };
                for nested in nested {
                    match nested.qualified_name.as_deref() {
                        Some(EXPRESSION) => match expression(&nested) {
                            Some(e) => expressions.push(e),
                            None => return unavailable("unresolved @Expressions member"),
                        },
                        None if simple(&nested) == "Expression" => {
                            return unavailable("unresolved @Expressions member")
                        }
                        _ => {}
                    }
                }
            }
            Some(DEFINITION) => definitions.push(definition(annotation)),
            Some(DEFINITIONS) => definitions.extend(definitions_container(annotation)),
            None if [
                "Expression",
                "Expressions",
                "Definition",
                "Definitions",
                "Local",
            ]
            .contains(&simple(annotation)) =>
            {
                return unavailable("unresolved context annotation binding")
            }
            _ => {}
        }
    }
    HostResolvedContextResult::Resolved(ResolvedMixinExtrasContext {
        handler_range,
        context: ExpressionContext {
            expression_index: MixinExtrasExpressionIndex { expressions },
            definition_index: MixinExtrasDefinitionIndex { definitions },
        },
    })
}
fn expression(annotation: &HostAnnotationFact) -> Option<MixinExtrasExpression> {
    if !annotation.validity.is_usable() || !annotation.has_unique_members() {
        return None;
    }
    let id = match annotation.member("id") {
        Some(e) => e.constant_string()?,
        None => String::new(),
    };
    Some(MixinExtrasExpression::from_utf16(
        id,
        annotation.member("value")?.string_array_utf16()?,
    ))
}
fn malformed(message: &str, range: Option<Range<usize>>) -> MixinExtrasDefinition {
    let len = range.as_ref().map_or(0, |r| r.end.saturating_sub(r.start));
    MixinExtrasDefinition {
        source_range: range,
        parse_issues: vec![DefinitionAnnotationParseIssue {
            attribute: "id".into(),
            raw_value: String::new(),
            message: message.into(),
            body_range: 0..len,
        }],
        ..Default::default()
    }
}
fn issue(
    attribute: &str,
    node: &HostExpressionFact,
    message: &str,
    body: Option<&Range<usize>>,
) -> DefinitionAnnotationParseIssue {
    let range = match (body, node.source_range.as_ref()) {
        (Some(body), Some(range)) => {
            let start = range.start.max(body.start).min(body.end);
            let end = range.end.min(body.end).max(start);
            (start - body.start)..(end - body.start)
        }
        _ => 0..0,
    };
    DefinitionAnnotationParseIssue {
        attribute: attribute.into(),
        raw_value: node.source_text.clone(),
        message: message.into(),
        body_range: range,
    }
}
fn definition(annotation: &HostAnnotationFact) -> MixinExtrasDefinition {
    if !annotation.validity.is_usable() || !annotation.has_unique_members() {
        return malformed(
            "unresolved @Definition member",
            annotation.body_range.clone(),
        );
    }
    let mut result = MixinExtrasDefinition {
        source_range: annotation.body_range.clone(),
        ..Default::default()
    };
    for (name, node) in &annotation.members {
        let failed = match name.as_str() {
            "id" => {
                result.id = node.constant_string();
                result.id.is_none().then_some("expected a string literal")
            }
            "method" => match node.string_array() {
                Some(v) => {
                    result.raw_method_references = v;
                    None
                }
                None => Some("expected a string or string array"),
            },
            "field" => match node.string_array() {
                Some(v) => {
                    result.raw_field_references = v;
                    None
                }
                None => Some("expected a string or string array"),
            },
            "type" => match node.type_array_source_names() {
                Some(v) => {
                    result.class_literal_type_names = v;
                    None
                }
                None => Some("expected a class literal or class literal array"),
            },
            "local" => match local_array(node) {
                Some(v) => {
                    result.local_specs = v;
                    None
                }
                None => Some("expected @Local or @Local array"),
            },
            "remap" => {
                result.remap = node.constant_boolean();
                result
                    .remap
                    .is_none()
                    .then_some("expected a boolean literal")
            }
            _ => None,
        };
        if let Some(message) = failed {
            result
                .parse_issues
                .push(issue(name, node, message, annotation.body_range.as_ref()));
        }
    }
    // Legacy processes attributes in this fixed order regardless of pair order.
    result.parse_issues.sort_by_key(|i| {
        ["id", "method", "field", "type", "local", "remap"]
            .iter()
            .position(|n| *n == i.attribute)
            .unwrap_or(6)
    });
    result
}
fn local_array(node: &HostExpressionFact) -> Option<Vec<LocalSugarSpec>> {
    let nested = node.nested_annotations()?;
    let mut result = vec![];
    for annotation in nested {
        match annotation.qualified_name.as_deref() {
            Some(LOCAL) => result.push(local(&annotation)?),
            None if simple(&annotation) == "Local" => return None,
            _ => {}
        }
    }
    Some(result)
}
pub(super) fn local(annotation: &HostAnnotationFact) -> Option<LocalSugarSpec> {
    if !annotation.validity.is_usable() || !annotation.has_unique_members() {
        return None;
    }
    let boolean = |name| -> Option<bool> {
        match annotation.member(name) {
            Some(n) => n.constant_boolean(),
            None => Some(false),
        }
    };
    let integer = |name| -> Option<Option<i32>> {
        match annotation.member(name) {
            Some(n) => Some(match n.constant_int()? {
                -1 => None,
                v => Some(v),
            }),
            None => Some(None),
        }
    };
    Some(LocalSugarSpec {
        args_only: boolean("argsOnly")?,
        index: integer("index")?,
        ordinal: integer("ordinal")?,
        names: match annotation.member("name") {
            Some(n) => n.string_array()?.into_iter().collect(),
            None => Default::default(),
        },
        print: boolean("print")?,
        type_class_name: match annotation.member("type") {
            Some(n) => Some(n.type_literal_source_name()?),
            None => None,
        },
    })
}
fn definitions_container(annotation: &HostAnnotationFact) -> Vec<MixinExtrasDefinition> {
    let malformed_container = || {
        vec![malformed(
            "unresolved @Definitions member",
            annotation.body_range.clone(),
        )]
    };
    if !annotation.validity.is_usable() || !annotation.has_unique_members() {
        return malformed_container();
    }
    let Some(value) = annotation.member("value") else {
        return malformed_container();
    };
    if !value.validity.is_usable() {
        return malformed_container();
    }
    let elements: Vec<Option<&HostExpressionFact>> = match &value.kind {
        HostExpressionKind::Array(v) => v.iter().map(Option::as_ref).collect(),
        _ => vec![Some(value)],
    };
    let mut result = vec![];
    for node in elements {
        let Some(node) = node else {
            result.push(malformed("expected @Definition annotation", None));
            continue;
        };
        match &node.kind {
            HostExpressionKind::Annotation(a) => match a.qualified_name.as_deref() {
                Some(DEFINITION) => result.push(definition(a)),
                None if simple(a) == "Definition" => result.push(malformed(
                    "unresolved @Definition member",
                    a.body_range.clone(),
                )),
                _ => {}
            },
            _ => result.push(malformed(
                "expected @Definition annotation",
                node.source_range.clone(),
            )),
        }
    }
    result
}
