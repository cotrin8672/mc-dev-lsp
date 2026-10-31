//! Minecraft annotation source fallback. Host supplied facts should take precedence.
//! Every offset is a UTF-8 byte boundary; public text ranges use LSP UTF-16 units.
use crate::{offset_to_position, position_to_offset, ClassIndex, McTextPosition, McTextRange};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MixinAnnotation {
    Mixin,
    Final,
    Mutable,
    Pseudo,
    SoftOverride,
    Shadow,
    Accessor,
    Invoker,
    Inject,
    Redirect,
    ModifyArg,
    ModifyArgs,
    ModifyVariable,
    ModifyConstant,
    ModifyExpressionValue,
    ModifyReturnValue,
    ModifyReceiver,
    WrapOperation,
    WrapWithCondition,
    WrapMethod,
    Overwrite,
    Surrogate,
    At,
    Constant,
    Slice,
    Local,
    Share,
    Definition,
    Definitions,
    Expression,
    Expressions,
}
impl MixinAnnotation {
    pub const ALL: [Self; 31] = [
        Self::Mixin,
        Self::Final,
        Self::Mutable,
        Self::Pseudo,
        Self::SoftOverride,
        Self::Shadow,
        Self::Accessor,
        Self::Invoker,
        Self::Inject,
        Self::Redirect,
        Self::ModifyArg,
        Self::ModifyArgs,
        Self::ModifyVariable,
        Self::ModifyConstant,
        Self::ModifyExpressionValue,
        Self::ModifyReturnValue,
        Self::ModifyReceiver,
        Self::WrapOperation,
        Self::WrapWithCondition,
        Self::WrapMethod,
        Self::Overwrite,
        Self::Surrogate,
        Self::At,
        Self::Constant,
        Self::Slice,
        Self::Local,
        Self::Share,
        Self::Definition,
        Self::Definitions,
        Self::Expression,
        Self::Expressions,
    ];
    pub fn simple_name(self) -> &'static str {
        match self {
            Self::Mixin => "Mixin",
            Self::Final => "Final",
            Self::Mutable => "Mutable",
            Self::Pseudo => "Pseudo",
            Self::SoftOverride => "SoftOverride",
            Self::Shadow => "Shadow",
            Self::Accessor => "Accessor",
            Self::Invoker => "Invoker",
            Self::Inject => "Inject",
            Self::Redirect => "Redirect",
            Self::ModifyArg => "ModifyArg",
            Self::ModifyArgs => "ModifyArgs",
            Self::ModifyVariable => "ModifyVariable",
            Self::ModifyConstant => "ModifyConstant",
            Self::ModifyExpressionValue => "ModifyExpressionValue",
            Self::ModifyReturnValue => "ModifyReturnValue",
            Self::ModifyReceiver => "ModifyReceiver",
            Self::WrapOperation => "WrapOperation",
            Self::WrapWithCondition => "WrapWithCondition",
            Self::WrapMethod => "WrapMethod",
            Self::Overwrite => "Overwrite",
            Self::Surrogate => "Surrogate",
            Self::At => "At",
            Self::Constant => "Constant",
            Self::Slice => "Slice",
            Self::Local => "Local",
            Self::Share => "Share",
            Self::Definition => "Definition",
            Self::Definitions => "Definitions",
            Self::Expression => "Expression",
            Self::Expressions => "Expressions",
        }
    }
    pub fn from_simple_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.simple_name() == name)
    }
    pub fn official_fqns(self) -> Vec<String> {
        let package = match self {
            Self::Accessor | Self::Invoker => "org.spongepowered.asm.mixin.gen",
            Self::Inject
            | Self::Redirect
            | Self::ModifyArg
            | Self::ModifyArgs
            | Self::ModifyVariable
            | Self::ModifyConstant
            | Self::Surrogate
            | Self::At
            | Self::Constant
            | Self::Slice => "org.spongepowered.asm.mixin.injection",
            Self::ModifyExpressionValue
            | Self::ModifyReturnValue
            | Self::ModifyReceiver
            | Self::WrapWithCondition => "com.llamalad7.mixinextras.injector",
            Self::WrapOperation => "com.llamalad7.mixinextras.injector.wrapoperation",
            Self::WrapMethod => "com.llamalad7.mixinextras.injector.wrapmethod",
            Self::Local | Self::Share => "com.llamalad7.mixinextras.sugar",
            Self::Definition | Self::Definitions | Self::Expression | Self::Expressions => {
                "com.llamalad7.mixinextras.expression"
            }
            _ => "org.spongepowered.asm.mixin",
        };
        let mut names = vec![format!("{package}.{}", self.simple_name())];
        match self {
            Self::Mixin => names.push("io.github.diskria.lapis.annotations.KMixin".into()),
            Self::Shadow => names.push("io.github.diskria.lapis.annotations.KShadow".into()),
            Self::WrapWithCondition => {
                names.push("com.llamalad7.mixinextras.injector.v2.WrapWithCondition".into())
            }
            _ => {}
        }
        names
    }
    pub fn from_official_fqn(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|a| a.official_fqns().iter().any(|f| f == name))
    }
    pub fn is_injector(self) -> bool {
        matches!(
            self,
            Self::Inject
                | Self::Redirect
                | Self::ModifyArg
                | Self::ModifyArgs
                | Self::ModifyVariable
                | Self::ModifyConstant
                | Self::ModifyExpressionValue
                | Self::ModifyReturnValue
                | Self::ModifyReceiver
                | Self::WrapOperation
                | Self::WrapWithCondition
                | Self::WrapMethod
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationSlot {
    Attribute,
    Class,
    Targets,
    Method,
    Handler,
    Value,
    Target,
    AccessorValue,
    InvokerValue,
    ShadowMember,
    OverwriteMethod,
    Prefix,
    Remap,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedExpressionPrefix {
    pub prefix: String,
    pub cursor: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedExpressionPrefixUtf16 {
    pub prefix: Vec<u16>,
    pub cursor: usize,
}
/// A display/lexing view only. JVM values always remain in the original unit vector.
/// The map translates byte boundaries in this view back to original UTF-16 units.
pub struct Utf16SourceView {
    pub text: String,
    byte_to_unit: Vec<usize>,
}
impl Utf16SourceView {
    pub fn new(units: &[u16]) -> Self {
        let mut text = String::new();
        let mut map = vec![0];
        let mut at = 0;
        while at < units.len() {
            let (ch, len) = if (0xd800..=0xdbff).contains(&units[at])
                && units
                    .get(at + 1)
                    .is_some_and(|u| (0xdc00..=0xdfff).contains(u))
            {
                (
                    char::from_u32(
                        0x10000
                            + ((units[at] as u32 - 0xd800) << 10)
                            + (units[at + 1] as u32 - 0xdc00),
                    )
                    .unwrap(),
                    2,
                )
            } else {
                (char::from_u32(units[at] as u32).unwrap_or('\u{fffd}'), 1)
            };
            let before = text.len();
            text.push(ch);
            map.resize(text.len() + 1, at);
            at += len;
            map[text.len()] = at;
            debug_assert!(text.len() > before);
        }
        Self {
            text,
            byte_to_unit: map,
        }
    }
    pub fn unit(&self, byte: usize) -> usize {
        self.byte_to_unit[byte]
    }
    pub fn byte(&self, unit: usize) -> usize {
        self.text
            .char_indices()
            .map(|(at, _)| at)
            .chain(std::iter::once(self.text.len()))
            .find(|at| self.unit(*at) >= unit)
            .unwrap_or(self.text.len())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpressionCompletionPosition {
    StatementStart,
    ValueStart,
    AfterDot,
    AfterMethodReference,
    None,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionCompletionLexResult {
    pub position: ExpressionCompletionPosition,
    pub token_start: usize,
    pub token_end: usize,
}
impl ExpressionCompletionLexResult {
    pub fn has_identifier_token(&self) -> bool {
        self.token_end > self.token_start
    }
}
pub struct ExpressionCompletionLexer;
impl ExpressionCompletionLexer {
    pub fn lex(source: &str, cursor: usize) -> ExpressionCompletionLexResult {
        let cursor = cursor.min(source.len());
        if !source.is_char_boundary(cursor) {
            return ExpressionCompletionLexResult {
                position: ExpressionCompletionPosition::None,
                token_start: cursor,
                token_end: cursor,
            };
        }
        let code = AnnotationContextExtractor::mask_non_code(source);
        if cursor > 0
            && source[..cursor]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_whitespace())
            && code.as_bytes()[cursor - 1] == b' '
        {
            return ExpressionCompletionLexResult {
                position: ExpressionCompletionPosition::None,
                token_start: cursor,
                token_end: cursor,
            };
        }
        let before = code[..cursor].trim_end();
        let position_at = |at: usize| {
            let p = code[..at].trim_end();
            if p.ends_with("::") {
                ExpressionCompletionPosition::AfterMethodReference
            } else if p.ends_with('.') && !p.ends_with(":.") && !source[at..].starts_with("class") {
                ExpressionCompletionPosition::AfterDot
            } else if p.chars().filter(|c| *c == '{').count()
                > p.chars().filter(|c| *c == '}').count()
                && p.ends_with(['{', ';'])
            {
                ExpressionCompletionPosition::StatementStart
            } else if p.is_empty()
                || p.ends_with([
                    '+', '-', '*', '/', '%', '&', '|', '^', '!', '=', '<', '>', '?', ':', ',', '(',
                    '[', '@',
                ])
            {
                ExpressionCompletionPosition::ValueStart
            } else {
                ExpressionCompletionPosition::None
            }
        };
        if before.ends_with("::") || before.ends_with('.') {
            return ExpressionCompletionLexResult {
                position: position_at(cursor),
                token_start: cursor,
                token_end: cursor,
            };
        }
        let mut start = cursor;
        while let Some((at, ch)) = source[..start].char_indices().next_back() {
            if !ident(ch) {
                break;
            }
            start = at;
        }
        let mut end = cursor;
        while end < source.len() {
            let ch = source[end..].chars().next().unwrap();
            if !ident(ch) {
                break;
            }
            end += ch.len_utf8();
        }
        ExpressionCompletionLexResult {
            position: position_at(start),
            token_start: start,
            token_end: end,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnotationContext {
    pub annotation: MixinAnnotation,
    pub slot: AnnotationSlot,
    pub partial_value: String,
    pub value_start_offset: usize,
    pub value_end_offset: usize,
    pub annotation_start_offset: usize,
    pub annotation_end_offset: usize,
    pub mixin_target_internal_names: Vec<String>,
    pub inject_method_name: Option<String>,
    pub at_value: Option<String>,
    pub shadow_prefix: Option<String>,
    pub shadow_remap: bool,
    pub shadow_member_is_method: Option<bool>,
    pub existing_attributes: BTreeSet<String>,
    pub parent_injector_annotation: Option<MixinAnnotation>,
    pub at_inside_slice: bool,
    pub expression_completion_position: Option<ExpressionCompletionPosition>,
    pub decoded_partial_value: Option<String>,
    pub decoded_expression_prefix: Option<DecodedExpressionPrefix>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded_expression_prefix_utf16: Option<DecodedExpressionPrefixUtf16>,
    pub attribute_name: Option<String>,
    pub resolved_annotation_fqn: Option<String>,
    pub is_kotlin: bool,
}
impl AnnotationContext {
    pub fn new(
        annotation: MixinAnnotation,
        slot: AnnotationSlot,
        partial_value: impl Into<String>,
    ) -> Self {
        Self {
            annotation,
            slot,
            partial_value: partial_value.into(),
            value_start_offset: 0,
            value_end_offset: 0,
            annotation_start_offset: 0,
            annotation_end_offset: 0,
            mixin_target_internal_names: vec![],
            inject_method_name: None,
            at_value: None,
            shadow_prefix: None,
            shadow_remap: true,
            shadow_member_is_method: None,
            existing_attributes: BTreeSet::new(),
            parent_injector_annotation: None,
            at_inside_slice: false,
            expression_completion_position: None,
            decoded_partial_value: None,
            decoded_expression_prefix: None,
            decoded_expression_prefix_utf16: None,
            attribute_name: None,
            resolved_annotation_fqn: None,
            is_kotlin: false,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaSourceImports {
    pub package_name: Option<String>,
    pub explicit: BTreeMap<String, String>,
    pub wildcard_packages: Vec<String>,
    pub ambiguous_explicit: BTreeMap<String, Vec<String>>,
    pub is_kotlin: bool,
}
pub struct JavaTypeResolutionContext<'a> {
    pub imports: &'a JavaSourceImports,
    pub lookup: Option<&'a dyn JavaTypeLookup>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLookupResult {
    Found { internal_name: String },
    Ambiguous { internal_names: Vec<String> },
    NotFound,
}
pub trait JavaTypeLookup {
    fn find_internal_name_by_qualified_name(&self, fqcn: &str) -> Option<String>;
    fn find_in_package(&self, package: &str, name: &str) -> TypeLookupResult;
    fn find_in_wildcard_imports(&self, imports: &[String], name: &str) -> TypeLookupResult;
}
pub struct ClassIndexJavaTypeLookup<'a> {
    pub class_index: &'a dyn ClassIndex,
}
impl JavaTypeLookup for ClassIndexJavaTypeLookup<'_> {
    fn find_internal_name_by_qualified_name(&self, fqcn: &str) -> Option<String> {
        self.class_index
            .find_class_by_fqn(fqcn)
            .map(|c| c.internal_name)
    }
    fn find_in_package(&self, package: &str, name: &str) -> TypeLookupResult {
        self.find_internal_name_by_qualified_name(&format!("{package}.{name}"))
            .map_or(TypeLookupResult::NotFound, |internal_name| {
                TypeLookupResult::Found { internal_name }
            })
    }
    fn find_in_wildcard_imports(&self, imports: &[String], name: &str) -> TypeLookupResult {
        let names: Vec<_> = imports
            .iter()
            .filter_map(|p| self.find_internal_name_by_qualified_name(&format!("{p}.{name}")))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        match names.len() {
            0 => TypeLookupResult::NotFound,
            1 => TypeLookupResult::Found {
                internal_name: names[0].clone(),
            },
            _ => TypeLookupResult::Ambiguous {
                internal_names: names,
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeDescriptorResult {
    Resolved {
        descriptor: String,
    },
    Unresolved {
        raw_type: String,
        normalized_type: String,
    },
    Ambiguous {
        raw_type: String,
        normalized_type: String,
        candidates: Vec<String>,
    },
}
impl TypeDescriptorResult {
    pub fn descriptor(self) -> Option<String> {
        if let Self::Resolved { descriptor } = self {
            Some(descriptor)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceToken {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub kind: SourceTokenKind,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTokenKind {
    Identifier,
    Symbol,
    String {
        content_start: usize,
        content_end: usize,
        raw: bool,
        closed: bool,
    },
    Character,
}
fn ident(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '$'
}
fn kotlin_hint(source: &str) -> bool {
    let tokens = source_tokens(source, false);
    let mut masked = source.as_bytes().to_vec();
    masked.fill(b' ');
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' || b == b'\r' {
            masked[i] = b;
        }
    }
    for t in &tokens {
        if matches!(
            t.kind,
            SourceTokenKind::Identifier | SourceTokenKind::Symbol
        ) {
            masked[t.start..t.end].copy_from_slice(&source.as_bytes()[t.start..t.end]);
        }
    }
    let code = String::from_utf8(masked).unwrap();
    if code.lines().any(|l| {
        let l = l.trim();
        l.starts_with("@file:")
            || ((l.starts_with("import ") || l.starts_with("package ")) && !l.ends_with(';'))
            || (l.starts_with("import ") && l.contains(" as "))
    }) || code.contains("::class")
    {
        return true;
    }
    let mut depth = 0;
    let mut parens = 0;
    for t in tokens {
        if t.kind != SourceTokenKind::Identifier && t.kind != SourceTokenKind::Symbol {
            continue;
        }
        match t.text.as_str() {
            "{" => depth += 1,
            "}" => depth -= 1,
            "(" => parens += 1,
            ")" => parens -= 1,
            "fun" | "val" | "var" if depth == 0 && parens == 0 => return true,
            _ => {}
        }
    }
    false
}
/// A small lexer for annotation facts, preserving byte offsets rather than reparsing language semantics.
pub fn source_tokens(source: &str, is_kotlin: bool) -> Vec<SourceToken> {
    let b = source.as_bytes();
    let mut i = 0;
    let mut result = vec![];
    while i < b.len() {
        let ch = source[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            i += ch.len_utf8();
            continue;
        }
        if source[i..].starts_with("//") {
            i = source[i..].find('\n').map_or(b.len(), |n| i + n);
            continue;
        }
        if source[i..].starts_with("/*") {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if source[i..].starts_with("/*") && is_kotlin {
                    depth += 1;
                    i += 2;
                } else if source[i..].starts_with("*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += source[i..].chars().next().unwrap().len_utf8();
                }
            }
            continue;
        }
        let start = i;
        if ch == '"' || ch == '\'' {
            let triple = ch == '"' && source[i..].starts_with("\"\"\"");
            let delimiter = if triple { 3 } else { 1 };
            i += delimiter;
            let cs = i;
            while i < b.len() {
                if b[i] == b'\\' && (!triple || !is_kotlin) {
                    i += 1;
                    if i < b.len() {
                        i += source[i..].chars().next().unwrap().len_utf8();
                    }
                    continue;
                }
                if (triple && source[i..].starts_with("\"\"\""))
                    || (!triple && source[i..].starts_with(ch))
                {
                    break;
                }
                i += source[i..].chars().next().unwrap().len_utf8();
            }
            let ce = i;
            let closed = i < b.len();
            if closed {
                i += delimiter;
            }
            let kind = if ch == '\'' {
                SourceTokenKind::Character
            } else {
                SourceTokenKind::String {
                    content_start: cs,
                    content_end: ce,
                    raw: triple && is_kotlin,
                    closed,
                }
            };
            result.push(SourceToken {
                text: source[start..i].into(),
                start,
                end: i,
                kind,
            });
            continue;
        }
        if ch == '`' && is_kotlin {
            i += 1;
            while i < b.len() && b[i] != b'`' {
                i += source[i..].chars().next().unwrap().len_utf8();
            }
            if i < b.len() {
                i += 1;
            }
            result.push(SourceToken {
                text: source[start + 1..i.saturating_sub(1)].into(),
                start,
                end: i,
                kind: SourceTokenKind::Identifier,
            });
            continue;
        }
        if ident(ch) {
            i += ch.len_utf8();
            while i < b.len() {
                let c = source[i..].chars().next().unwrap();
                if !ident(c) {
                    break;
                }
                i += c.len_utf8();
            }
            result.push(SourceToken {
                text: source[start..i].into(),
                start,
                end: i,
                kind: SourceTokenKind::Identifier,
            });
        } else {
            i += ch.len_utf8();
            result.push(SourceToken {
                text: source[start..i].into(),
                start,
                end: i,
                kind: SourceTokenKind::Symbol,
            });
        }
    }
    result
}
pub fn source_range(source: &str, start: usize, end: usize) -> McTextRange {
    McTextRange {
        start: offset_to_position(source, start).expect("lexer start boundary"),
        end: offset_to_position(source, end).unwrap_or_else(|| {
            let mut p = offset_to_position(source, source.len()).expect("source end boundary");
            p.character += end.saturating_sub(source.len());
            p
        }),
    }
}
pub fn matching(tokens: &[SourceToken], open: usize) -> Option<usize> {
    let (close, op) = match tokens.get(open)?.text.as_str() {
        "(" => (")", "("),
        "[" => ("]", "["),
        "{" => ("}", "{"),
        "<" => (">", "<"),
        _ => return None,
    };
    let mut depth = 0;
    for (i, t) in tokens.iter().enumerate().skip(open) {
        if t.kind != SourceTokenKind::Symbol {
            continue;
        }
        if t.text == op {
            depth += 1;
        } else if t.text == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}
fn qualified(tokens: &[SourceToken], start: usize) -> (String, usize) {
    let mut i = start;
    let mut name = String::new();
    if tokens
        .get(i)
        .is_some_and(|t| t.kind == SourceTokenKind::Identifier)
    {
        name.push_str(&tokens[i].text);
        i += 1;
        while tokens.get(i).is_some_and(|t| t.text == ".")
            && tokens
                .get(i + 1)
                .is_some_and(|t| t.kind == SourceTokenKind::Identifier)
        {
            name.push('.');
            name.push_str(&tokens[i + 1].text);
            i += 2;
        }
    }
    (name, i)
}
pub fn split_top_level(value: &str, delimiter: char) -> Vec<String> {
    let ts = source_tokens(value, true);
    let mut depth = 0;
    let mut start = 0;
    let mut parts = vec![];
    for t in ts {
        if t.kind != SourceTokenKind::Symbol {
            continue;
        }
        match t.text.as_str() {
            "(" | "[" | "{" | "<" => depth += 1,
            ")" | "]" | "}" | ">" => depth -= 1,
            _ => {}
        }
        if depth == 0 && t.text.starts_with(delimiter) {
            if !value[start..t.start].trim().is_empty() {
                parts.push(value[start..t.start].trim().into());
            }
            start = t.end;
        }
    }
    if !value[start..].trim().is_empty() {
        parts.push(value[start..].trim().into());
    }
    parts
}

pub struct JavaTypeDescriptorResolver;
impl JavaTypeDescriptorResolver {
    /// Syntactic rendering retained for callers that already own resolved source types.
    /// Validation and target resolution use descriptor_or_null instead.
    pub fn descriptor(raw: &str, imports: &JavaSourceImports) -> String {
        let ctx = JavaTypeResolutionContext {
            imports,
            lookup: None,
        };
        if let Some(d) = Self::descriptor_or_null(raw, &ctx) {
            return d;
        }
        let mut value = strip_annotations(raw)
            .split_whitespace()
            .filter(|s| !matches!(*s, "final" | "volatile" | "transient"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut arrays = 0;
        if value.starts_with("vararg ") {
            arrays += 1;
            value = value.trim_start_matches("vararg ").into();
        }
        if value.ends_with("...") {
            arrays += 1;
            value.truncate(value.len() - 3);
        }
        while value.trim_end().ends_with("[]") {
            arrays += 1;
            value = value.trim_end().trim_end_matches("[]").trim().into();
        }
        if imports.is_kotlin {
            value = value.trim_end_matches('?').into();
            if let Some(arg) = value
                .strip_prefix("Array<")
                .and_then(|a| a.strip_suffix('>'))
            {
                return format!("[{}", Self::descriptor(arg, imports));
            }
        }
        let base = value.split('<').next().unwrap_or("").trim();
        let fqn = imports.explicit.get(base).cloned().unwrap_or_else(|| {
            if base.contains('.') || imports.package_name.is_none() {
                base.into()
            } else {
                format!("{}.{base}", imports.package_name.as_deref().unwrap())
            }
        });
        format!("{}L{};", "[".repeat(arrays), fqn_to_internal(&fqn))
    }
    pub fn method_descriptor(
        return_type: &str,
        parameters: &[String],
        imports: &JavaSourceImports,
    ) -> String {
        let ctx = JavaTypeResolutionContext {
            imports,
            lookup: None,
        };
        let ret = if imports.is_kotlin {
            KotlinTypeDescriptorResolver::resolve(return_type, &ctx, true, false)
                .descriptor()
                .unwrap_or_else(|| Self::descriptor(return_type, imports))
        } else {
            Self::descriptor(return_type, imports)
        };
        format!(
            "({}){ret}",
            parameters
                .iter()
                .map(|p| Self::descriptor(p, imports))
                .collect::<Vec<_>>()
                .join("")
        )
    }
    pub fn is_kotlin_source(source: &str, uri: &str) -> bool {
        Self::imports_for_uri(source, uri).is_kotlin
    }
    pub fn imports_for(source: &str) -> JavaSourceImports {
        Self::imports_for_uri(source, "")
    }
    pub fn imports_for_uri(source: &str, uri: &str) -> JavaSourceImports {
        let path = uri
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let is_kotlin = if path.ends_with(".java") {
            false
        } else {
            path.ends_with(".kt") || path.ends_with(".kts") || kotlin_hint(source)
        };
        let ts = source_tokens(source, is_kotlin);
        let mut result = JavaSourceImports {
            is_kotlin,
            ..Default::default()
        };
        let mut imports: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut i = 0;
        while i < ts.len() {
            if ts[i].text == "package" || ts[i].text == "import" {
                let package = ts[i].text == "package";
                i += 1;
                if ts.get(i).is_some_and(|t| t.text == "static") {
                    while i < ts.len() && !matches!(ts[i].text.as_str(), ";") {
                        i += 1;
                    }
                    continue;
                }
                let (name, next) = qualified(&ts, i);
                i = next;
                if name.is_empty() {
                    continue;
                }
                if !is_kotlin {
                    let end = if ts.get(i).is_some_and(|t| t.text == ".")
                        && ts.get(i + 1).is_some_and(|t| t.text == "*")
                    {
                        i + 2
                    } else {
                        i
                    };
                    if !ts.get(end).is_some_and(|t| t.text == ";") {
                        continue;
                    }
                }
                if package {
                    result.package_name = Some(name);
                } else if ts.get(i).is_some_and(|t| t.text == ".")
                    && ts.get(i + 1).is_some_and(|t| t.text == "*")
                {
                    result.wildcard_packages.push(name);
                    i += 2;
                } else {
                    let mut alias = name.rsplit('.').next().unwrap().to_string();
                    if is_kotlin && ts.get(i).is_some_and(|t| t.text == "as") {
                        if let Some(a) = ts.get(i + 1) {
                            alias = a.text.clone();
                        }
                        i += 2;
                    }
                    imports.entry(alias).or_default().insert(name);
                }
            } else {
                i += 1;
            }
        }
        for (name, values) in imports {
            if values.len() == 1 {
                result
                    .explicit
                    .insert(name, values.into_iter().next().unwrap());
            } else {
                result
                    .ambiguous_explicit
                    .insert(name, values.into_iter().collect());
            }
        }
        result
    }
    pub fn descriptor_or_null(raw: &str, ctx: &JavaTypeResolutionContext<'_>) -> Option<String> {
        Self::descriptor_or_diagnostic(raw, ctx).descriptor()
    }
    pub fn descriptor_or_diagnostic(
        raw: &str,
        ctx: &JavaTypeResolutionContext<'_>,
    ) -> TypeDescriptorResult {
        if ctx.imports.is_kotlin {
            return KotlinTypeDescriptorResolver::resolve(raw, ctx, false, false);
        }
        let mut value = strip_annotations(raw)
            .split_whitespace()
            .filter(|s| !matches!(*s, "final" | "volatile" | "transient"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut arrays = 0;
        if value.ends_with("...") {
            arrays += 1;
            value.truncate(value.len() - 3);
        }
        loop {
            let v = value.trim();
            if v.ends_with("[]") {
                arrays += 1;
                value = v[..v.len() - 2].trim().into();
            } else {
                value = v.into();
                break;
            }
        }
        let base = value.split('<').next().unwrap_or("").trim();
        if let Some(p) = java_primitive(base) {
            return TypeDescriptorResult::Resolved {
                descriptor: format!("{}{}", "[".repeat(arrays), p),
            };
        }
        match resolve_internal(base, ctx) {
            TypeLookupResult::Found { internal_name } => TypeDescriptorResult::Resolved {
                descriptor: format!("{}L{internal_name};", "[".repeat(arrays)),
            },
            TypeLookupResult::Ambiguous { internal_names } => TypeDescriptorResult::Ambiguous {
                raw_type: raw.into(),
                normalized_type: base.into(),
                candidates: internal_names,
            },
            TypeLookupResult::NotFound => TypeDescriptorResult::Unresolved {
                raw_type: raw.into(),
                normalized_type: base.into(),
            },
        }
    }
    pub fn method_descriptor_or_null(
        return_type: &str,
        parameters: &[String],
        ctx: &JavaTypeResolutionContext<'_>,
    ) -> Option<String> {
        let params: Option<Vec<_>> = parameters
            .iter()
            .map(|p| Self::descriptor_or_null(p, ctx))
            .collect();
        let ret = if ctx.imports.is_kotlin {
            KotlinTypeDescriptorResolver::resolve(return_type, ctx, true, false).descriptor()
        } else {
            Self::descriptor_or_null(return_type, ctx)
        }?;
        Some(format!("({}){ret}", params?.join("")))
    }
    pub fn raw_parameter_types(params: &str) -> Vec<String> {
        split_top_level(params, ',')
            .iter()
            .filter_map(|p| {
                let clean = strip_annotations(p);
                let clean = clean.trim().trim_start_matches("final ").trim();
                let at = clean.rfind(char::is_whitespace)?;
                Some(clean[..at].trim().into())
            })
            .collect()
    }
    pub fn split_top_level(value: &str, delimiter: char) -> Vec<String> {
        split_top_level(value, delimiter)
    }
}
fn java_primitive(name: &str) -> Option<&'static str> {
    Some(match name {
        "void" => "V",
        "boolean" => "Z",
        "byte" => "B",
        "char" => "C",
        "short" => "S",
        "int" => "I",
        "long" => "J",
        "float" => "F",
        "double" => "D",
        _ => return None,
    })
}
pub fn fqn_to_internal(fqn: &str) -> String {
    let parts: Vec<_> = fqn.split('.').collect();
    let first = parts
        .iter()
        .position(|p| p.chars().next().is_some_and(char::is_uppercase));
    match first {
        Some(at) => {
            let p = parts[..at].join("/");
            let c = parts[at..].join("$");
            if p.is_empty() {
                c
            } else {
                format!("{p}/{c}")
            }
        }
        None => fqn.replace('.', "/"),
    }
}
fn resolve_internal(base: &str, ctx: &JavaTypeResolutionContext<'_>) -> TypeLookupResult {
    let imp = ctx.imports;
    let root = base.split('.').next().unwrap_or("");
    if let Some(c) = imp.ambiguous_explicit.get(root) {
        return TypeLookupResult::Ambiguous {
            internal_names: c.iter().map(|f| fqn_to_internal(f)).collect(),
        };
    }
    if let Some(f) = imp.explicit.get(root) {
        return TypeLookupResult::Found {
            internal_name: fqn_to_internal(&format!("{f}{}", &base[root.len()..])),
        };
    }
    if base.contains('.') {
        if let Some(n) = ctx
            .lookup
            .and_then(|l| l.find_internal_name_by_qualified_name(base))
        {
            return TypeLookupResult::Found { internal_name: n };
        }
        if [
            "java.",
            "javax.",
            "org.spongepowered.",
            "net.minecraft.",
            "com.mojang.",
        ]
        .iter()
        .any(|p| base.starts_with(p))
        {
            return TypeLookupResult::Found {
                internal_name: fqn_to_internal(base),
            };
        }
        return TypeLookupResult::NotFound;
    }
    if [
        "Boolean",
        "Byte",
        "Character",
        "CharSequence",
        "Class",
        "Double",
        "Enum",
        "Float",
        "Integer",
        "Iterable",
        "Long",
        "Object",
        "Short",
        "String",
        "Throwable",
        "Void",
    ]
    .contains(&base)
    {
        return TypeLookupResult::Found {
            internal_name: format!("java/lang/{base}"),
        };
    }
    if let Some(l) = ctx.lookup {
        if let Some(p) = &imp.package_name {
            let r = l.find_in_package(p, base);
            if r != TypeLookupResult::NotFound {
                return r;
            }
        }
        let r = l.find_in_wildcard_imports(&imp.wildcard_packages, base);
        if r != TypeLookupResult::NotFound {
            return r;
        }
    }
    let known = match base {
        "CallbackInfo" => Some("org/spongepowered/asm/mixin/injection/callback/CallbackInfo"),
        "CallbackInfoReturnable" => {
            Some("org/spongepowered/asm/mixin/injection/callback/CallbackInfoReturnable")
        }
        "ItemStack" => Some("net/minecraft/item/ItemStack"),
        "BlockPos" => Some("net/minecraft/util/math/BlockPos"),
        "Identifier" => Some("net/minecraft/util/Identifier"),
        "ResourceLocation" => Some("net/minecraft/resources/ResourceLocation"),
        "Component" => Some("net/minecraft/network/chat/Component"),
        "Text" => Some("net/minecraft/text/Text"),
        _ => None,
    };
    known.map_or(TypeLookupResult::NotFound, |n| TypeLookupResult::Found {
        internal_name: n.into(),
    })
}
pub struct KotlinTypeDescriptorResolver;
impl KotlinTypeDescriptorResolver {
    pub fn resolve(
        raw: &str,
        ctx: &JavaTypeResolutionContext<'_>,
        is_return_type: bool,
        boxed: bool,
    ) -> TypeDescriptorResult {
        let mut t = raw.trim();
        if t.starts_with("vararg ") || t.ends_with("...") || t.ends_with("[]") {
            t = t
                .trim_start_matches("vararg ")
                .trim_end_matches("...")
                .trim_end_matches("[]")
                .trim();
            return descriptor_raw(array_result(Self::resolve(t, ctx, false, false)), raw);
        }
        let nullable = t.ends_with('?');
        t = t.trim_end_matches('?').trim();
        let base = t.split('<').next().unwrap_or("").trim();
        let root = base.split('.').next().unwrap_or("");
        if let Some(c) = ctx.imports.ambiguous_explicit.get(root) {
            return TypeDescriptorResult::Ambiguous {
                raw_type: raw.into(),
                normalized_type: base.into(),
                candidates: c.iter().map(|f| fqn_to_internal(f)).collect(),
            };
        }
        let imported = ctx.imports.explicit.get(root);
        let name = imported.map_or_else(
            || base.to_string(),
            |n| format!("{n}{}", &base[root.len()..]),
        );
        if imported.is_none() && !base.contains('.') {
            if let Some(l) = ctx.lookup {
                let mut results = vec![];
                if let Some(p) = &ctx.imports.package_name {
                    results.push(l.find_in_package(p, base));
                }
                results.push(l.find_in_wildcard_imports(&ctx.imports.wildcard_packages, base));
                for r in results {
                    match r {
                        TypeLookupResult::Found { internal_name } => {
                            return TypeDescriptorResult::Resolved {
                                descriptor: format!("L{internal_name};"),
                            }
                        }
                        TypeLookupResult::Ambiguous { internal_names } => {
                            return TypeDescriptorResult::Ambiguous {
                                raw_type: raw.into(),
                                normalized_type: base.into(),
                                candidates: internal_names,
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let builtin = if imported.is_none() {
            name.trim_start_matches("kotlin.collections.")
                .trim_start_matches("kotlin.")
        } else if name.starts_with("kotlin.") {
            name.trim_start_matches("kotlin.collections.")
                .trim_start_matches("kotlin.")
        } else {
            &name
        };
        let primitive = match builtin {
            "Boolean" => Some(("Z", "Boolean")),
            "Byte" => Some(("B", "Byte")),
            "Char" => Some(("C", "Character")),
            "Short" => Some(("S", "Short")),
            "Int" => Some(("I", "Integer")),
            "Long" => Some(("J", "Long")),
            "Float" => Some(("F", "Float")),
            "Double" => Some(("D", "Double")),
            _ => None,
        };
        if let Some((p, box_name)) = primitive {
            return TypeDescriptorResult::Resolved {
                descriptor: if nullable || boxed {
                    format!("Ljava/lang/{box_name};")
                } else {
                    p.into()
                },
            };
        }
        if let Some(component) = builtin.strip_suffix("Array") {
            if !component.is_empty() {
                let component = Self::resolve(&format!("kotlin.{component}"), ctx, false, false);
                if matches!(&component,TypeDescriptorResult::Resolved{descriptor}if descriptor.len()==1)
                {
                    return array_result(component);
                }
            }
        }
        if builtin == "Array" {
            if let Some(arg) = t.split_once('<').and_then(|(_, a)| a.strip_suffix('>')) {
                let args = split_top_level(arg, ',');
                if args.len() == 1 {
                    if args[0] == "*" || args[0].starts_with("in ") {
                        return TypeDescriptorResult::Resolved {
                            descriptor: "[Ljava/lang/Object;".into(),
                        };
                    }
                    return descriptor_raw(
                        array_result(Self::resolve(
                            args[0].trim_start_matches("out "),
                            ctx,
                            false,
                            true,
                        )),
                        raw,
                    );
                }
            }
            return TypeDescriptorResult::Unresolved {
                raw_type: raw.into(),
                normalized_type: base.into(),
            };
        }
        let reference = match builtin {
            "Any" => Some("java/lang/Object"),
            "String" => Some("java/lang/String"),
            "CharSequence" => Some("java/lang/CharSequence"),
            "Throwable" => Some("java/lang/Throwable"),
            "Number" => Some("java/lang/Number"),
            "Comparable" => Some("java/lang/Comparable"),
            "Enum" => Some("java/lang/Enum"),
            "Annotation" => Some("java/lang/annotation/Annotation"),
            "Cloneable" => Some("java/lang/Cloneable"),
            "Unit" => Some("kotlin/Unit"),
            "Nothing" => Some("java/lang/Void"),
            "Iterable" | "MutableIterable" => Some("java/lang/Iterable"),
            "Iterator" | "MutableIterator" => Some("java/util/Iterator"),
            "Collection" | "MutableCollection" => Some("java/util/Collection"),
            "List" | "MutableList" => Some("java/util/List"),
            "ListIterator" | "MutableListIterator" => Some("java/util/ListIterator"),
            "Set" | "MutableSet" => Some("java/util/Set"),
            "Map" | "MutableMap" => Some("java/util/Map"),
            "Map.Entry" | "MutableMap.MutableEntry" => Some("java/util/Map$Entry"),
            _ => None,
        };
        if let Some(r) = reference {
            return TypeDescriptorResult::Resolved {
                descriptor: if builtin == "Unit" && is_return_type && !nullable {
                    "V".into()
                } else {
                    format!("L{r};")
                },
            };
        }
        let mut imp = ctx.imports.clone();
        imp.is_kotlin = false;
        descriptor_raw(
            JavaTypeDescriptorResolver::descriptor_or_diagnostic(
                base,
                &JavaTypeResolutionContext {
                    imports: &imp,
                    lookup: ctx.lookup,
                },
            ),
            raw,
        )
    }
}
fn array_result(r: TypeDescriptorResult) -> TypeDescriptorResult {
    match r {
        TypeDescriptorResult::Resolved { descriptor } => TypeDescriptorResult::Resolved {
            descriptor: format!("[{descriptor}"),
        },
        other => other,
    }
}
fn descriptor_raw(result: TypeDescriptorResult, raw: &str) -> TypeDescriptorResult {
    match result {
        TypeDescriptorResult::Unresolved {
            normalized_type, ..
        } => TypeDescriptorResult::Unresolved {
            raw_type: raw.into(),
            normalized_type,
        },
        TypeDescriptorResult::Ambiguous {
            normalized_type,
            candidates,
            ..
        } => TypeDescriptorResult::Ambiguous {
            raw_type: raw.into(),
            normalized_type,
            candidates,
        },
        other => other,
    }
}
fn strip_annotations(raw: &str) -> String {
    let ts = source_tokens(raw, false);
    let mut result = raw.as_bytes().to_vec();
    let mut i = 0;
    while i < ts.len() {
        if ts[i].text == "@" {
            let start = ts[i].start;
            let (_, next) = qualified(&ts, i + 1);
            i = next;
            if ts.get(i).is_some_and(|t| t.text == "(") {
                i = matching(&ts, i).map_or(i, |n| n + 1);
            }
            let end = ts.get(i).map_or(raw.len(), |t| t.start);
            for b in &mut result[start..end] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        } else {
            i += 1;
        }
    }
    String::from_utf8(result).unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedString {
    pub value: String,
    /// One entry per UTF-8 decoded byte, plus the exclusive end.
    pub source_offsets: Vec<usize>,
}
pub struct JavaStringContentDecoder;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedEscape {
    pub r#char: char,
    pub next_index: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedEscapeUtf16 {
    pub units: Vec<u16>,
    pub next_index: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedStringUtf16 {
    pub value: Vec<u16>,
    /// One raw UTF-8 byte offset per decoded UTF-16 unit, plus the exclusive end.
    pub source_offsets: Vec<usize>,
}
impl JavaStringContentDecoder {
    pub fn decode_unicode_escape_utf16(
        source: &str,
        index: usize,
        limit: usize,
    ) -> Option<DecodedEscapeUtf16> {
        if limit > source.len()
            || index + 2 > limit
            || source.as_bytes().get(index) != Some(&b'\\')
            || source.as_bytes().get(index + 1) != Some(&b'u')
        {
            return None;
        }
        let mut hex = index + 1;
        while hex < limit && source.as_bytes()[hex] == b'u' {
            hex += 1;
        }
        if hex + 4 > limit {
            return None;
        }
        Some(DecodedEscapeUtf16 {
            units: vec![u16::from_str_radix(source.get(hex..hex + 4)?, 16).ok()?],
            next_index: hex + 4,
        })
    }
    pub fn decode_unicode_escape(
        source: &str,
        index: usize,
        limit: usize,
    ) -> Option<DecodedEscape> {
        let decoded = Self::decode_unicode_escape_utf16(source, index, limit)?;
        Some(DecodedEscape {
            r#char: char::from_u32(decoded.units[0] as u32)?,
            next_index: decoded.next_index,
        })
    }
    pub fn decode_escape(source: &str, index: usize, limit: usize) -> Option<DecodedEscape> {
        if limit > source.len() || index + 2 > limit || source.as_bytes().get(index) != Some(&b'\\')
        {
            return None;
        }
        let escaped = source.as_bytes()[index + 1];
        let mut next = index + 2;
        let c = match escaped {
            b'b' => '\u{8}',
            b't' => '\t',
            b'n' => '\n',
            b'f' => '\u{c}',
            b'r' => '\r',
            b'"' => '"',
            b'\'' => '\'',
            b'\\' => '\\',
            b's' => ' ',
            b'0'..=b'7' => {
                let mut value = (escaped - b'0') as u32;
                let max = if escaped <= b'3' { 3 } else { 2 };
                let mut count = 1;
                while next < limit
                    && count < max
                    && (b'0'..=b'7').contains(&source.as_bytes()[next])
                {
                    value = value * 8 + (source.as_bytes()[next] - b'0') as u32;
                    count += 1;
                    next += 1;
                }
                char::from_u32(value)?
            }
            _ => return None,
        };
        Some(DecodedEscape {
            r#char: c,
            next_index: next,
        })
    }
    pub fn decode_content(source: &str, start: usize, end: usize) -> Option<String> {
        String::from_utf16(&Self::decode_content_utf16(source, start, end)?).ok()
    }
    pub fn decode_content_utf16(source: &str, start: usize, end: usize) -> Option<Vec<u16>> {
        if start > end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            return None;
        }
        let mut value = String::new();
        let mut index = start;
        while index < end {
            index = Self::append_decoded_char(source, index, end, &mut value)?;
        }
        Some(value.encode_utf16().collect())
    }
    pub fn decode_expression_content_utf16(
        source: &str,
        start: usize,
        end: usize,
        raw: bool,
        is_kotlin: bool,
    ) -> Option<DecodedStringUtf16> {
        map_expression_utf16_content(source, start, end, raw, is_kotlin)
    }
    pub fn append_decoded_char(
        source: &str,
        index: usize,
        limit: usize,
        value: &mut String,
    ) -> Option<usize> {
        if index >= limit || limit > source.len() {
            return None;
        }
        let current = source.get(index..limit)?.chars().next()?;
        if current == '\\' {
            let escape = Self::decode_escape(source, index, limit)?;
            value.push(escape.r#char);
            Some(escape.next_index)
        } else {
            value.push(current);
            Some(index + current.len_utf8())
        }
    }
    pub fn decode_with_offsets(
        source: &str,
        start: usize,
        end: usize,
        raw: bool,
        is_kotlin: bool,
    ) -> Option<DecodedString> {
        if start > end || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return None;
        }
        let mut i = start;
        let mut value = String::new();
        let mut offsets = vec![];
        while i < end {
            let at = i;
            let ch = source[i..].chars().next()?;
            let decoded = if is_kotlin && ch == '$' {
                return None;
            } else if ch != '\\' || raw {
                i += ch.len_utf8();
                ch
            } else {
                i += 1;
                let escaped = *source.as_bytes().get(i)?;
                i += 1;
                match escaped {
                    b'b' => '\u{8}',
                    b't' => '\t',
                    b'n' => '\n',
                    b'f' if !is_kotlin => '\u{c}',
                    b'r' => '\r',
                    b'"' => '"',
                    b'\'' => '\'',
                    b'\\' => '\\',
                    b'$' if is_kotlin => '$',
                    b's' if !is_kotlin => ' ',
                    b'u' => {
                        while i < end && source.as_bytes()[i] == b'u' {
                            i += 1;
                        }
                        if i + 4 > end {
                            return None;
                        }
                        let v = u32::from_str_radix(&source[i..i + 4], 16).ok()?;
                        i += 4;
                        char::from_u32(v)?
                    }
                    b'0'..=b'7' if !is_kotlin => {
                        let mut v = (escaped - b'0') as u32;
                        let mut n = 1;
                        while i < end
                            && n < if escaped <= b'3' { 3 } else { 2 }
                            && (b'0'..=b'7').contains(&source.as_bytes()[i])
                        {
                            v = v * 8 + (source.as_bytes()[i] - b'0') as u32;
                            i += 1;
                            n += 1;
                        }
                        char::from_u32(v)?
                    }
                    _ => return None,
                }
            };
            if i > end {
                return None;
            }
            value.push(decoded);
            offsets.extend(std::iter::repeat_n(at, decoded.len_utf8()));
        }
        offsets.push(end);
        Some(DecodedString {
            value,
            source_offsets: offsets,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationSite {
    pub annotation: MixinAnnotation,
    pub resolved_fqn: Option<String>,
    pub start: usize,
    pub end: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub name_end: usize,
}
fn annotation_identity(
    name: &str,
    imp: &JavaSourceImports,
) -> Option<(MixinAnnotation, Option<String>)> {
    if name.contains('.') {
        return MixinAnnotation::from_official_fqn(name).map(|a| (a, Some(name.into())));
    }
    if imp.ambiguous_explicit.contains_key(name) {
        return None;
    }
    if let Some(f) = imp.explicit.get(name) {
        return MixinAnnotation::from_official_fqn(f).map(|a| (a, Some(f.clone())));
    }
    let matches: Vec<_> = imp
        .wildcard_packages
        .iter()
        .filter_map(|p| {
            let f = format!("{p}.{name}");
            MixinAnnotation::from_official_fqn(&f).map(|a| (a, Some(f)))
        })
        .collect();
    if matches.len() == 1 {
        return matches.into_iter().next();
    }
    if imp.is_kotlin {
        None
    } else {
        MixinAnnotation::from_simple_name(name).map(|a| (a, None))
    }
}
pub fn annotations(source: &str) -> Vec<AnnotationSite> {
    let imp = JavaTypeDescriptorResolver::imports_for(source);
    annotations_with_imports(source, &imp)
}
pub fn annotations_with_imports(source: &str, imp: &JavaSourceImports) -> Vec<AnnotationSite> {
    let ts = source_tokens(source, imp.is_kotlin);
    let mut result = vec![];
    for i in 0..ts.len() {
        let start = if ts[i].text == "@" {
            i + 1
        } else if imp.is_kotlin
            && ts[i].kind == SourceTokenKind::Identifier
            && i > 0
            && matches!(ts[i - 1].text.as_str(), "[" | "," | "=" | "(")
        {
            i
        } else {
            continue;
        };
        let (name, next) = qualified(&ts, start);
        let Some((a, fqn)) = annotation_identity(&name, &imp) else {
            continue;
        };
        let has_paren = ts.get(next).is_some_and(|t| t.text == "(");
        if start == i && !has_paren {
            continue;
        }
        let name_end = ts[next - 1].end;
        let close = if has_paren { matching(&ts, next) } else { None };
        let end = if has_paren {
            close.map_or(source.len(), |n| ts[n].end)
        } else {
            name_end
        };
        let body_start = if has_paren { ts[next].end } else { name_end };
        let body_end = if has_paren {
            close.map_or(source.len(), |n| ts[n].start)
        } else {
            name_end
        };
        result.push(AnnotationSite {
            annotation: a,
            resolved_fqn: fqn,
            start: ts[i].start,
            end,
            body_start,
            body_end,
            name_end,
        });
    }
    result.sort_by_key(|a| a.start);
    result.dedup_by_key(|a| a.start);
    result
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceValue {
    pub value: String,
    pub start: usize,
    pub end: usize,
    pub expression_start: usize,
    pub expression_end: usize,
    pub raw: bool,
    pub decoded: Option<DecodedString>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationAttribute {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub value_start: usize,
    pub value_end: usize,
    pub explicit: bool,
}
pub fn annotation_attributes(source: &str, site: &AnnotationSite) -> Vec<AnnotationAttribute> {
    let imp = JavaTypeDescriptorResolver::imports_for(source);
    annotation_attributes_in_language(source, site, imp.is_kotlin)
}
fn annotation_attributes_in_language(
    source: &str,
    site: &AnnotationSite,
    is_kotlin: bool,
) -> Vec<AnnotationAttribute> {
    let ts = source_tokens(&source[site.body_start..site.body_end], is_kotlin);
    let mut starts = vec![0];
    let mut depth = 0;
    for (i, t) in ts.iter().enumerate() {
        if t.kind != SourceTokenKind::Symbol {
            continue;
        }
        match t.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => starts.push(i + 1),
            _ => {}
        }
    }
    let mut out = vec![];
    for n in 0..starts.len() {
        let start = starts[n];
        let end = starts.get(n + 1).map_or(ts.len(), |n| n - 1);
        if start >= end {
            continue;
        }
        let explicit = start + 1 < end && ts[start + 1].text == "=";
        let vs = if explicit { start + 2 } else { start };
        let value_start = ts
            .get(vs)
            .map_or(site.body_end, |t| site.body_start + t.start);
        let value_end = if end > vs {
            site.body_start + ts[end - 1].end
        } else {
            value_start
        };
        out.push(AnnotationAttribute {
            name: if explicit {
                ts[start].text.clone()
            } else {
                "value".into()
            },
            start: site.body_start + ts[start].start,
            end: value_end,
            value_start,
            value_end,
            explicit,
        });
    }
    out
}
pub fn annotation_values(source: &str, site: &AnnotationSite, name: &str) -> Vec<SourceValue> {
    let imp = JavaTypeDescriptorResolver::imports_for(source);
    annotation_values_with_imports(source, site, name, &imp)
}
pub fn annotation_values_with_imports(
    source: &str,
    site: &AnnotationSite,
    name: &str,
    imp: &JavaSourceImports,
) -> Vec<SourceValue> {
    let mut result = vec![];
    for a in annotation_attributes_in_language(source, site, imp.is_kotlin)
        .into_iter()
        .filter(|a| a.name == name)
    {
        let ts = source_tokens(&source[a.value_start..a.value_end], imp.is_kotlin);
        let mut i = 0;
        while i < ts.len() {
            match ts[i].kind {
                SourceTokenKind::String {
                    content_start,
                    content_end,
                    raw,
                    closed,
                } => {
                    let start = a.value_start + content_start;
                    let mut end = a.value_start + content_end;
                    let first = i;
                    let mut complete = closed;
                    let mut value = source[start..end].to_string();
                    let mut decoded = JavaStringContentDecoder::decode_with_offsets(
                        source,
                        start,
                        end,
                        raw,
                        imp.is_kotlin,
                    );
                    while name == "target" && ts.get(i + 1).is_some_and(|t| t.text == "+") {
                        let Some(next) = ts.get(i + 2) else {
                            complete = false;
                            break;
                        };
                        let SourceTokenKind::String {
                            content_start,
                            content_end,
                            raw,
                            closed,
                        } = next.kind
                        else {
                            complete = false;
                            break;
                        };
                        i += 2;
                        let next_start = a.value_start + content_start;
                        end = a.value_start + content_end;
                        complete &= closed;
                        value.push_str(&source[next_start..end]);
                        let next_decoded = JavaStringContentDecoder::decode_with_offsets(
                            source,
                            next_start,
                            end,
                            raw,
                            imp.is_kotlin,
                        );
                        decoded = match (decoded, next_decoded) {
                            (Some(mut a), Some(b)) => {
                                a.source_offsets.pop();
                                a.source_offsets.extend(b.source_offsets);
                                a.value.push_str(&b.value);
                                Some(a)
                            }
                            _ => None,
                        };
                    }
                    if complete {
                        result.push(SourceValue {
                            value,
                            start,
                            end,
                            expression_start: a.value_start + ts[first].start,
                            expression_end: a.value_start + ts[i].end,
                            raw,
                            decoded,
                        });
                    }
                    i += 1;
                }
                SourceTokenKind::Identifier => {
                    let (n, next) = qualified(&ts, i);
                    let mut end = next;
                    if n.ends_with(".class") {
                    } else if ts.get(next).is_some_and(|t| t.text == ":")
                        && ts.get(next + 1).is_some_and(|t| t.text == ":")
                        && ts.get(next + 2).is_some_and(|t| t.text == "class")
                    {
                        end += 3;
                    } else {
                        i += 1;
                        continue;
                    }
                    let start = a.value_start + ts[i].start;
                    let finish = a.value_start + ts[end - 1].end;
                    result.push(SourceValue {
                        value: source[start..finish].into(),
                        start,
                        end: finish,
                        expression_start: start,
                        expression_end: finish,
                        raw: false,
                        decoded: None,
                    });
                    i = end;
                }
                _ => i += 1,
            }
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinClassScope {
    pub class_name: String,
    pub raw_targets: Vec<String>,
}
#[derive(Debug, Clone)]
struct TypeScope {
    start: usize,
    end: usize,
    name: String,
    companion: bool,
    annotation: Option<AnnotationSite>,
}
fn type_scopes(source: &str) -> Vec<TypeScope> {
    let imp = JavaTypeDescriptorResolver::imports_for(source);
    let ts = source_tokens(source, imp.is_kotlin);
    let anns = annotations(source);
    let mut result = vec![];
    for (i, t) in ts.iter().enumerate() {
        if !matches!(
            t.text.as_str(),
            "class" | "interface" | "enum" | "record" | "object"
        ) {
            continue;
        }
        if i > 0 && matches!(ts[i - 1].text.as_str(), "." | ":") {
            continue;
        }
        let companion = t.text == "object" && i > 0 && ts[i - 1].text == "companion";
        let name = if companion {
            "Companion".into()
        } else {
            match ts.get(i + 1) {
                Some(n) if n.kind == SourceTokenKind::Identifier => n.text.clone(),
                _ => continue,
            }
        };
        let Some(open) = (i + 1..ts.len()).find(|n| matches!(ts[*n].text.as_str(), "{" | ";"))
        else {
            continue;
        };
        if ts[open].text != "{" {
            continue;
        }
        let end = matching(&ts, open).map_or(source.len(), |n| ts[n].end);
        let annotation = anns
            .iter()
            .find(|a| {
                a.annotation == MixinAnnotation::Mixin
                    && a.end <= t.start
                    && !source[a.end..t.start].contains(['{', '}', ';'])
            })
            .cloned();
        result.push(TypeScope {
            start: t.start,
            end,
            name,
            companion,
            annotation,
        });
    }
    result
}
pub struct AnnotationContextExtractor;
impl AnnotationContextExtractor {
    pub fn mask_non_code(source: &str) -> String {
        let mut b = source.as_bytes().to_vec();
        let ts = source_tokens(source, kotlin_hint(source));
        let mut keep = vec![false; b.len()];
        for t in ts {
            if matches!(
                t.kind,
                SourceTokenKind::Identifier | SourceTokenKind::Symbol
            ) {
                keep[t.start..t.end].fill(true);
            }
        }
        for (i, c) in b.iter_mut().enumerate() {
            if !keep[i] && *c != b'\n' && *c != b'\r' {
                *c = b' ';
            }
        }
        String::from_utf8(b).unwrap()
    }
    pub fn fqn_to_internal(fqn: &str) -> String {
        fqn.replace('.', "/")
    }
    pub fn find_annotation_offsets(source: &str, a: MixinAnnotation) -> Vec<usize> {
        annotations(source)
            .iter()
            .filter(|s| s.annotation == a)
            .map(|s| s.start)
            .collect()
    }
    pub fn find_injector_annotation_offsets(source: &str) -> Vec<usize> {
        annotations(source)
            .iter()
            .filter(|s| s.annotation.is_injector())
            .map(|s| s.start)
            .collect()
    }
    pub fn extract_at_annotation_bodies(source: &str) -> Vec<String> {
        annotations(source)
            .iter()
            .filter(|s| s.annotation == MixinAnnotation::At)
            .map(|s| source[s.body_start..s.body_end].into())
            .collect()
    }
    pub fn annotation_end_offset(source: &str, start: usize) -> Option<usize> {
        annotations(source)
            .iter()
            .find(|s| s.start == start)
            .map(|s| s.end)
    }
    pub fn parse_mixin_target_values(source: &str, start: usize) -> Vec<String> {
        let Some(a) = annotations(source)
            .into_iter()
            .find(|a| a.start == start && a.annotation == MixinAnnotation::Mixin)
        else {
            return vec![];
        };
        let mut out = vec![];
        for key in ["value", "targets", "target"] {
            for v in annotation_values(source, &a, key) {
                let is_string = v.decoded.is_some();
                let raw = v.decoded.map_or(v.value, |d| d.value);
                let n = raw.trim_end_matches(".class").trim_end_matches("::class");
                out.push(if !is_string && n.contains('.') {
                    n.replace('.', "/")
                } else {
                    n.into()
                });
            }
        }
        out
    }
    pub fn resolve_mixin_class_scope(source: &str, offset: usize) -> Option<MixinClassScope> {
        let scopes = type_scopes(source);
        let mut contained: Vec<_> = scopes
            .iter()
            .filter(|s| s.start <= offset && offset <= s.end)
            .collect();
        contained.sort_by_key(|s| s.start);
        let current = contained.iter().rev().find(|s| !s.companion)?;
        let a = current.annotation.as_ref()?;
        let imp = JavaTypeDescriptorResolver::imports_for(source);
        let names = contained
            .iter()
            .filter(|s| !s.companion)
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join("$");
        let name = imp
            .package_name
            .map_or(names.clone(), |p| format!("{p}.{names}"));
        Some(MixinClassScope {
            class_name: name,
            raw_targets: Self::parse_mixin_target_values(source, a.start),
        })
    }
    pub fn resolve_raw_mixin_targets(source: &str, offset: usize) -> Vec<String> {
        Self::resolve_mixin_class_scope(source, offset).map_or(vec![], |s| s.raw_targets)
    }
    pub fn resolve_mixin_targets(source: &str, offset: usize) -> Vec<String> {
        Self::resolve_raw_mixin_targets(source, offset)
    }
    pub fn extract(source: &str, line: usize, character: usize) -> Option<AnnotationContext> {
        Self::extract_at_offset(
            source,
            position_to_offset(source, McTextPosition { line, character })?,
        )
    }
    pub fn extract_at_offset(source: &str, cursor: usize) -> Option<AnnotationContext> {
        if !source.is_char_boundary(cursor) {
            return None;
        }
        let imp = JavaTypeDescriptorResolver::imports_for(source);
        let sites = annotations(source);
        if let Some(a) = sites.iter().rev().find(|a| {
            a.end <= cursor
                && (a.body_start == a.name_end || a.body_end + 1 == a.end)
                && source[a.end..cursor].chars().all(char::is_whitespace)
                && (a.annotation.is_injector()
                    || matches!(
                        a.annotation,
                        MixinAnnotation::Accessor
                            | MixinAnnotation::Invoker
                            | MixinAnnotation::Overwrite
                    ))
        }) {
            let mut c = AnnotationContext::new(a.annotation, AnnotationSlot::Handler, "");
            c.value_start_offset = cursor;
            c.value_end_offset = cursor;
            c.annotation_start_offset = a.start;
            c.annotation_end_offset = a.end;
            c.mixin_target_internal_names = Self::resolve_mixin_targets(source, cursor);
            c.resolved_annotation_fqn = a.resolved_fqn.clone();
            c.is_kotlin = imp.is_kotlin;
            return Some(c);
        }
        let mut site = sites
            .iter()
            .filter(|a| a.body_start <= cursor && cursor <= a.body_end)
            .max_by_key(|a| a.start)
            .cloned();
        if site.is_none() {
            site = sites
                .iter()
                .rev()
                .find(|a| {
                    matches!(
                        a.annotation,
                        MixinAnnotation::Shadow | MixinAnnotation::Overwrite
                    ) && a.end <= cursor
                        && !source[a.end..cursor].contains([';', '{', '}'])
                })
                .cloned();
        }
        let a = site?;
        // An unrelated nested annotation must not borrow its parent's known identity.
        let all_tokens = source_tokens(source, imp.is_kotlin);
        for (i, t) in all_tokens.iter().enumerate() {
            let begin = if t.text == "@" {
                i + 1
            } else if imp.is_kotlin
                && t.kind == SourceTokenKind::Identifier
                && i > 0
                && matches!(all_tokens[i - 1].text.as_str(), "[" | "," | "=" | "(")
            {
                i
            } else {
                continue;
            };
            let (name, next) = qualified(&all_tokens, begin);
            if t.start <= a.start
                || t.start > cursor
                || !all_tokens.get(next).is_some_and(|t| t.text == "(")
            {
                continue;
            }
            if matching(&all_tokens, next).map_or(true, |n| cursor <= all_tokens[n].end)
                && annotation_identity(&name, &imp).is_none()
            {
                return None;
            }
        }
        let attrs = annotation_attributes(source, &a);
        let mut existing = BTreeSet::new();
        for attr in &attrs {
            if attr.explicit {
                existing.insert(attr.name.clone());
            }
        }
        let mut ctx = AnnotationContext {
            annotation: a.annotation,
            slot: AnnotationSlot::Attribute,
            partial_value: String::new(),
            value_start_offset: cursor,
            value_end_offset: cursor,
            annotation_start_offset: a.start,
            annotation_end_offset: a.end,
            mixin_target_internal_names: Self::resolve_mixin_targets(source, cursor),
            inject_method_name: None,
            at_value: None,
            shadow_prefix: None,
            shadow_remap: true,
            shadow_member_is_method: None,
            existing_attributes: existing,
            parent_injector_annotation: None,
            at_inside_slice: false,
            expression_completion_position: None,
            decoded_partial_value: None,
            decoded_expression_prefix: None,
            decoded_expression_prefix_utf16: None,
            attribute_name: None,
            resolved_annotation_fqn: a.resolved_fqn.clone(),
            is_kotlin: imp.is_kotlin,
        };
        let parent = sites
            .iter()
            .filter(|p| p.start < a.start && a.end <= p.end && p.annotation.is_injector())
            .max_by_key(|p| p.start);
        let injector = if a.annotation.is_injector() {
            Some(&a)
        } else {
            parent
        };
        if let Some(p) = injector {
            ctx.inject_method_name = annotation_values(source, p, "method").first().map(|v| {
                v.decoded
                    .as_ref()
                    .map_or(v.value.clone(), |d| d.value.clone())
            });
            let ats = sites.iter().filter(|s| {
                s.annotation == MixinAnnotation::At && p.start < s.start && s.end <= p.end
            });
            ctx.at_value = ats
                .flat_map(|s| annotation_values(source, s, "value"))
                .next()
                .map(|v| v.value);
        }
        if a.annotation == MixinAnnotation::At {
            ctx.parent_injector_annotation = parent.map(|p| p.annotation);
            ctx.at_inside_slice = sites.iter().any(|s| {
                s.annotation == MixinAnnotation::Slice && s.start < a.start && a.end <= s.end
            });
            ctx.at_value = annotation_values(source, &a, "value")
                .first()
                .map(|v| v.value.clone());
        }
        if cursor >= a.end
            && matches!(
                a.annotation,
                MixinAnnotation::Shadow | MixinAnnotation::Overwrite
            )
        {
            let ts = source_tokens(&source[a.end..], imp.is_kotlin);
            let relative = cursor - a.end;
            let t = ts.iter().find(|t| {
                t.start <= relative && relative <= t.end && t.kind == SourceTokenKind::Identifier
            });
            if t.is_none() && cursor == a.end && a.end == source.len() {
                return None;
            }
            ctx.slot = if a.annotation == MixinAnnotation::Shadow {
                AnnotationSlot::ShadowMember
            } else {
                AnnotationSlot::OverwriteMethod
            };
            ctx.value_start_offset = t.map_or(cursor, |t| a.end + t.start);
            ctx.value_end_offset = t.map_or(cursor, |t| a.end + t.end);
            ctx.partial_value =
                source[ctx.value_start_offset..cursor.min(ctx.value_end_offset)].into();
            ctx.existing_attributes.clear();
            return Some(ctx);
        }
        let attr = attrs
            .iter()
            .find(|v| v.start <= cursor && cursor <= v.end)
            .or_else(|| {
                attrs
                    .last()
                    .filter(|v| v.end <= cursor && source[v.end..cursor].trim().is_empty())
            });
        let (name, vs, ve, explicit) = attr
            .map_or(("value", a.body_start, a.body_end, false), |v| {
                (v.name.as_str(), v.value_start, v.value_end, v.explicit)
            });
        ctx.attribute_name = explicit.then(|| name.into());
        if explicit && cursor < vs {
            ctx.attribute_name = None;
            ctx.partial_value = source[attr?.start..cursor].into();
            ctx.value_start_offset = attr?.start;
            ctx.value_end_offset = cursor;
            return Some(ctx);
        }
        let ts = source_tokens(&source[vs..ve], imp.is_kotlin);
        let token = ts
            .iter()
            .find(|t| t.start + vs <= cursor && cursor <= t.end + vs);
        if let Some(t) = token {
            if let SourceTokenKind::String {
                content_start,
                content_end,
                raw,
                closed,
                ..
            } = t.kind
            {
                let start = vs + content_start;
                let end = vs + content_end;
                if cursor < start || cursor > end {
                    return None;
                }
                ctx.value_start_offset = start;
                ctx.value_end_offset = if closed { end } else { cursor };
                ctx.partial_value = source[start..cursor].into();
                let d = JavaStringContentDecoder::decode_with_offsets(
                    source,
                    start,
                    cursor,
                    raw,
                    imp.is_kotlin,
                );
                ctx.decoded_partial_value = d.as_ref().map(|d| d.value.clone());
                if imp.is_kotlin
                    && ctx.decoded_partial_value.is_none()
                    && !matches!(
                        a.annotation,
                        MixinAnnotation::Expression | MixinAnnotation::Expressions
                    )
                {
                    return None;
                }
            } else {
                ctx.value_start_offset = vs + t.start;
                ctx.value_end_offset = ve;
                ctx.partial_value = source[vs + t.start..cursor].into();
            }
        } else {
            let start = source[vs.min(cursor)..cursor]
                .rfind(|c: char| c == ',' || c == '[' || c == '{' || c.is_whitespace())
                .map_or(vs, |n| vs + n + 1);
            ctx.value_start_offset = start;
            ctx.value_end_offset = ve.max(cursor);
            ctx.partial_value = source[start..cursor].into();
        }
        let shorthand = !explicit;
        let lapis = a.resolved_fqn.as_deref() == Some("io.github.diskria.lapis.annotations.KMixin");
        let names = attribute_names(a.annotation, lapis);
        let quoted = token.is_some_and(|t| matches!(t.kind, SourceTokenKind::String { .. }));
        if explicit
            && !quoted
            && name == "type"
            && matches!(
                a.annotation,
                MixinAnnotation::Local | MixinAnnotation::Definition
            )
        {
            ctx.value_start_offset = vs;
            ctx.value_end_offset = source_tokens(&source[ve..a.body_end], imp.is_kotlin)
                .iter()
                .find(|t| t.text == ",")
                .map_or(a.body_end, |t| ve + t.start);
            ctx.partial_value = source[vs..cursor].to_string();
        }
        let attribute_prefix = shorthand
            && !quoted
            && (a.annotation != MixinAnnotation::Mixin
                || !ctx
                    .partial_value
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase))
            && !ctx.partial_value.contains(['.', ':', '/'])
            && names.iter().any(|n| {
                n.to_lowercase()
                    .starts_with(&ctx.partial_value.to_lowercase())
            });
        if shorthand
            && !quoted
            && matches!(
                a.annotation,
                MixinAnnotation::Local
                    | MixinAnnotation::Share
                    | MixinAnnotation::Definition
                    | MixinAnnotation::Definitions
                    | MixinAnnotation::Expression
                    | MixinAnnotation::Expressions
            )
            && !attribute_prefix
        {
            return None;
        }
        ctx.slot = if attribute_prefix
            && (a.annotation != MixinAnnotation::Mixin || !ctx.partial_value.is_empty())
        {
            AnnotationSlot::Attribute
        } else {
            match (a.annotation, name) {
                (MixinAnnotation::Mixin, "value" | "target") => AnnotationSlot::Class,
                (MixinAnnotation::Mixin, "targets") => AnnotationSlot::Targets,
                (x, "method") if x.is_injector() => AnnotationSlot::Method,
                (MixinAnnotation::At, "target") => AnnotationSlot::Target,
                (MixinAnnotation::Accessor, _) => AnnotationSlot::AccessorValue,
                (MixinAnnotation::Invoker, _) => AnnotationSlot::InvokerValue,
                (MixinAnnotation::Shadow, "prefix") => AnnotationSlot::Prefix,
                (MixinAnnotation::Shadow, "remap") => AnnotationSlot::Remap,
                _ => AnnotationSlot::Value,
            }
        };
        if ctx.slot == AnnotationSlot::Attribute {
            ctx.value_end_offset = token.map_or(cursor, |t| vs + t.end);
        }
        if ctx.slot == AnnotationSlot::Class {
            ctx.partial_value = ctx
                .partial_value
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_string();
            let mut first = ts
                .iter()
                .position(|t| vs + t.start <= cursor && cursor <= vs + t.end)
                .unwrap_or(0);
            while first > 0 && !matches!(ts[first - 1].text.as_str(), "," | "[" | "{") {
                first -= 1;
            }
            let mut last = first;
            while last < ts.len() && !matches!(ts[last].text.as_str(), "," | "]" | "}") {
                last += 1;
            }
            ctx.value_start_offset = ts.get(first).map_or(vs, |t| vs + t.start);
            ctx.value_end_offset = if last > first {
                vs + ts[last - 1].end
            } else {
                ve
            };
            let class_literal = ts
                .windows(2)
                .any(|w| w[1].text == "class" && matches!(w[0].text.as_str(), "." | ":"));
            if !explicit
                && ts
                    .first()
                    .is_some_and(|t| matches!(t.text.as_str(), "[" | "{"))
                || !class_literal
            {
                ctx.value_start_offset = if explicit { vs } else { a.body_start };
                ctx.value_end_offset = cursor;
            }
        }
        if ctx.slot == AnnotationSlot::Method {
            ctx.inject_method_name = Some(ctx.partial_value.clone());
        }
        if a.annotation == MixinAnnotation::At && name == "value" {
            ctx.at_value = Some(ctx.partial_value.clone());
        }
        if a.annotation == MixinAnnotation::At && name == "target" {
            apply_concatenated_context(source, cursor, vs, ve, &mut ctx);
        }
        if matches!(
            a.annotation,
            MixinAnnotation::Expression | MixinAnnotation::Expressions
        ) && name == "value"
            && quoted
        {
            apply_expression_context(source, cursor, &mut ctx)?;
        }
        if ctx.expression_completion_position.is_none() {
            ctx.decoded_partial_value = None;
        }
        if ctx.slot != AnnotationSlot::Attribute {
            ctx.existing_attributes.clear();
        } else {
            ctx.inject_method_name = None;
            ctx.at_value = None;
        }
        if a.annotation != MixinAnnotation::At && !a.annotation.is_injector() {
            ctx.inject_method_name = None;
        }
        if a.annotation == MixinAnnotation::Definition
            && name == "method"
            && ctx.slot == AnnotationSlot::Value
        {
            ctx.inject_method_name = annotation_values(source, &a, "method")
                .first()
                .map(|v| v.value.clone());
        }
        if a.annotation == MixinAnnotation::Shadow {
            ctx.shadow_prefix = annotation_values(source, &a, "prefix")
                .first()
                .map(|v| v.value.clone());
            ctx.shadow_remap = !attrs
                .iter()
                .any(|v| v.name == "remap" && source[v.value_start..v.value_end].trim() == "false");
        }
        if a.body_start > a.name_end && a.body_end == a.end {
            ctx.annotation_end_offset = a.end + 1;
        }
        Some(ctx)
    }
}

pub struct MixinTargetResolver;
fn attribute_names(annotation: MixinAnnotation, lapis: bool) -> &'static [&'static str] {
    use MixinAnnotation::*;
    match annotation {
        Mixin if lapis => &["target", "env", "initStrategy"],
        Mixin => &["value", "targets", "priority", "remap"],
        Local => &["print", "ordinal", "index", "name", "argsOnly", "type"],
        Share => &["value", "namespace"],
        Definition => &["id", "method", "field", "type", "local", "remap"],
        Expression => &["value", "id"],
        Definitions | Expressions => &["value"],
        At => &[
            "value", "target", "ordinal", "opcode", "shift", "by", "args", "id", "remap",
        ],
        Constant => &[
            "nullValue",
            "intValue",
            "floatValue",
            "longValue",
            "doubleValue",
            "stringValue",
            "classValue",
            "ordinal",
            "slice",
            "expandZeroConditions",
            "log",
        ],
        Slice => &["id", "from", "to"],
        Shadow => &["prefix", "remap", "aliases"],
        Accessor | Invoker => &["value", "remap"],
        _ => &[
            "method",
            "at",
            "slice",
            "cancellable",
            "locals",
            "remap",
            "require",
            "expect",
            "allow",
            "constraints",
            "order",
            "index",
            "ordinal",
            "argsOnly",
            "print",
            "name",
        ],
    }
}
fn apply_concatenated_context(
    source: &str,
    cursor: usize,
    start: usize,
    end: usize,
    ctx: &mut AnnotationContext,
) {
    let tokens = source_tokens(&source[start..end], ctx.is_kotlin);
    let parts: Vec<_> = tokens
        .iter()
        .filter(|t| matches!(t.kind, SourceTokenKind::String { .. }))
        .collect();
    if parts.len() < 2 {
        return;
    }
    if !tokens
        .iter()
        .all(|t| matches!(t.kind, SourceTokenKind::String { .. }) || t.text == "+")
    {
        return;
    }
    let mut partial = String::new();
    for t in &parts {
        if let SourceTokenKind::String {
            content_start,
            content_end,
            ..
        } = t.kind
        {
            let a = start + content_start;
            let b = start + content_end;
            if cursor >= b {
                partial.push_str(&source[a..b]);
            } else if cursor >= a {
                partial.push_str(&source[a..cursor]);
                break;
            } else {
                break;
            }
        }
    }
    if let SourceTokenKind::String { content_start, .. } = parts[0].kind {
        ctx.value_start_offset = start + content_start;
    }
    if let SourceTokenKind::String {
        content_end,
        closed,
        ..
    } = parts.last().unwrap().kind
    {
        ctx.value_end_offset = if closed { start + content_end } else { cursor };
    }
    let _ = partial;
}
fn apply_expression_context(
    source: &str,
    cursor: usize,
    ctx: &mut AnnotationContext,
) -> Option<()> {
    let start = ctx.value_start_offset;
    let end = ctx.value_end_offset;
    let raw = ctx.is_kotlin && source[..start].ends_with("\"\"\"");
    let Some(decoded) = map_expression_content(source, start, end, raw, ctx.is_kotlin) else {
        let decoded = JavaStringContentDecoder::decode_expression_content_utf16(
            source,
            start,
            end,
            raw,
            ctx.is_kotlin,
        )?;
        let decoded_cursor = decoded
            .source_offsets
            .iter()
            .position(|at| *at >= cursor)
            .unwrap_or(decoded.value.len());
        let view = Utf16SourceView::new(&decoded.value);
        let lex = ExpressionCompletionLexer::lex(&view.text, view.byte(decoded_cursor));
        let token_start = view.unit(lex.token_start);
        let token_end = view.unit(lex.token_end);
        ctx.expression_completion_position = Some(lex.position);
        ctx.value_start_offset = decoded.source_offsets[token_start];
        ctx.value_end_offset = decoded.source_offsets[token_end];
        ctx.partial_value = source[ctx.value_start_offset..cursor.min(ctx.value_end_offset)].into();
        ctx.decoded_partial_value = String::from_utf16(&decoded.value[token_start..token_end]).ok();
        ctx.decoded_expression_prefix = None;
        ctx.decoded_expression_prefix_utf16 = Some(DecodedExpressionPrefixUtf16 {
            prefix: decoded.value[..decoded_cursor].to_vec(),
            cursor: decoded_cursor,
        });
        return Some(());
    };
    let mut decoded_cursor = decoded.value.len();
    for i in 0..decoded.source_offsets.len().saturating_sub(1) {
        let at = decoded.source_offsets[i];
        let next = decoded.source_offsets[i + 1];
        if at <= cursor && cursor < next && source.as_bytes().get(at) == Some(&b'\\') {
            decoded_cursor = i + 1;
            while !decoded.value.is_char_boundary(decoded_cursor) {
                decoded_cursor += 1;
            }
            break;
        }
        if at >= cursor {
            decoded_cursor = i;
            break;
        }
    }
    while !decoded.value.is_char_boundary(decoded_cursor) {
        decoded_cursor = decoded_cursor.saturating_sub(1);
    }
    let lex = ExpressionCompletionLexer::lex(&decoded.value, decoded_cursor);
    ctx.expression_completion_position = Some(lex.position);
    ctx.value_start_offset = decoded.source_offsets[lex.token_start];
    ctx.value_end_offset = decoded.source_offsets[lex.token_end];
    ctx.partial_value = source[ctx.value_start_offset..cursor.min(ctx.value_end_offset)].into();
    ctx.decoded_partial_value = Some(decoded.value[lex.token_start..lex.token_end].into());
    ctx.decoded_expression_prefix = Some(DecodedExpressionPrefix {
        prefix: decoded.value[..decoded_cursor].into(),
        cursor: decoded_cursor,
    });
    Some(())
}

// Java translates eligible Unicode escapes before interpreting string escapes.
fn map_expression_content(
    source: &str,
    start: usize,
    end: usize,
    raw: bool,
    is_kotlin: bool,
) -> Option<DecodedString> {
    let decoded = map_expression_utf16_content(source, start, end, raw, is_kotlin)?;
    let value = String::from_utf16(&decoded.value).ok()?;
    let mut source_offsets = Vec::new();
    let mut unit = 0;
    for c in value.chars() {
        source_offsets.extend(std::iter::repeat_n(
            decoded.source_offsets[unit],
            c.len_utf8(),
        ));
        unit += c.len_utf16();
    }
    source_offsets.push(*decoded.source_offsets.last()?);
    Some(DecodedString {
        value,
        source_offsets,
    })
}
fn map_expression_utf16_content(
    source: &str,
    start: usize,
    end: usize,
    raw: bool,
    is_kotlin: bool,
) -> Option<DecodedStringUtf16> {
    if start > end
        || end > source.len()
        || !source.is_char_boundary(start)
        || !source.is_char_boundary(end)
    {
        return None;
    }
    let mut phase1 = Vec::new();
    let mut starts = Vec::new();
    let mut index = start;
    let mut backslashes = 0;
    while index < end {
        let at = index;
        let current = source[index..end].chars().next()?;
        if !raw
            && current == '\\'
            && backslashes % 2 == 0
            && source.as_bytes().get(index + 1) == Some(&b'u')
        {
            let escape = JavaStringContentDecoder::decode_unicode_escape_utf16(source, index, end)?;
            if matches!(escape.units[0], 34 | 10 | 13) {
                return None;
            }
            index = escape.next_index;
            backslashes = 0;
            phase1.extend(escape.units);
            starts.push(at);
        } else {
            index += current.len_utf8();
            backslashes = if current == '\\' { backslashes + 1 } else { 0 };
            phase1.extend(current.encode_utf16(&mut [0; 2]).iter().copied());
            starts.extend(std::iter::repeat_n(at, current.len_utf16()));
        }
    }
    let mut value = Vec::new();
    let mut offsets = Vec::new();
    let mut index = 0;
    let mut end_offset = end;
    while index < phase1.len() {
        let at = index;
        let current = phase1[index];
        let decoded = if !raw && current == b'\\' as u16 {
            if is_kotlin && phase1.get(index + 1) == Some(&(b'$' as u16)) {
                index += 2;
                b'$' as u16
            } else {
                let ascii: String = phase1[index..]
                    .iter()
                    .copied()
                    .take(4)
                    .take_while(|u| *u < 128)
                    .map(|u| u as u8 as char)
                    .collect();
                if let Some(escape) =
                    JavaStringContentDecoder::decode_escape(&ascii, 0, ascii.len())
                {
                    index += escape.next_index;
                    escape.r#char as u16
                } else {
                    end_offset = starts[index];
                    break;
                }
            }
        } else {
            if is_kotlin && current == b'$' as u16 {
                return None;
            }
            index += 1;
            current
        };
        value.push(decoded);
        offsets.push(starts[at]);
    }
    offsets.push(end_offset);
    Some(DecodedStringUtf16 {
        value,
        source_offsets: offsets,
    })
}
impl MixinTargetResolver {
    pub fn resolve_target(
        raw: &str,
        index: &dyn ClassIndex,
        imports: Option<&JavaSourceImports>,
    ) -> Option<String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some(imports) = imports {
            let resolver = crate::mixinextras_expression::ClassLiteralTypeNameResolver::for_imports(
                imports, index,
            );
            let mut candidates = vec![raw.to_string()];
            if raw.contains('/') {
                candidates.push(raw.replace('/', "."));
            }
            for candidate in candidates {
                if let Some(d) = resolver.resolve(&candidate) {
                    if d.starts_with('L') && d.ends_with(';') {
                        if let Some(class) = index.find_class(&d[1..d.len() - 1]) {
                            return Some(class.internal_name);
                        }
                    }
                }
            }
        }
        let source_context = imports.is_some_and(|i| {
            !i.explicit.is_empty()
                || !i.wildcard_packages.is_empty()
                || !i.ambiguous_explicit.is_empty()
        });
        if !source_context || raw.contains(['.', '/']) {
            let internal = if raw.contains('/') {
                raw.to_string()
            } else {
                AnnotationContextExtractor::fqn_to_internal(raw)
            };
            if let Some(class) = index
                .find_class(&internal)
                .or_else(|| index.find_class_by_fqn(raw))
            {
                return Some(class.internal_name);
            }
        }
        if source_context {
            return None;
        }
        let matches: Vec<_> = index
            .find_classes(raw, 5)
            .into_iter()
            .filter(|c| c.simple_name == raw)
            .collect();
        if matches.len() == 1 {
            Some(matches[0].internal_name.clone())
        } else {
            None
        }
    }
    pub fn resolve_targets(
        raw: &[String],
        index: &dyn ClassIndex,
        imports: Option<&JavaSourceImports>,
    ) -> Vec<String> {
        let mut result = vec![];
        for r in raw {
            if let Some(n) = Self::resolve_target(r, index, imports) {
                if !result.contains(&n) {
                    result.push(n);
                }
            }
        }
        result
    }
    pub fn resolve_targets_from_source(source: &str, index: &dyn ClassIndex) -> Vec<String> {
        let imp = JavaTypeDescriptorResolver::imports_for(source);
        annotations(source)
            .iter()
            .find(|a| a.annotation == MixinAnnotation::Mixin)
            .map_or(vec![], |a| {
                Self::resolve_targets(
                    &AnnotationContextExtractor::parse_mixin_target_values(source, a.start),
                    index,
                    Some(&imp),
                )
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ParseSource {
    #[default]
    HandWrittenFallback,
    HandWritten,
    JdtAst,
    FallbackRegex,
    HostNormalized,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ParseConfidence {
    High,
    #[default]
    Medium,
    Low,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum JavaTypeKind {
    #[default]
    Class,
    Interface,
    Enum,
    Annotation,
    Record,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JavaModifier {
    Public,
    Protected,
    Private,
    Abstract,
    Static,
    Final,
    Synchronized,
    Native,
    Strictfp,
    Transient,
    Volatile,
}
impl JavaModifier {
    pub fn from_name(n: &str) -> Option<Self> {
        Some(match n.to_ascii_lowercase().as_str() {
            "public" => Self::Public,
            "protected" => Self::Protected,
            "private" => Self::Private,
            "abstract" => Self::Abstract,
            "static" => Self::Static,
            "final" => Self::Final,
            "synchronized" => Self::Synchronized,
            "native" => Self::Native,
            "strictfp" => Self::Strictfp,
            "transient" => Self::Transient,
            "volatile" => Self::Volatile,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixinMemberAnnotationKind {
    Accessor,
    Invoker,
    Shadow,
    Overwrite,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinTargetRef {
    pub internal_name: String,
    pub range: McTextRange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinMemberModel {
    pub annotation_kind: MixinMemberAnnotationKind,
    pub java_name: String,
    pub explicit_target_name: Option<String>,
    pub return_descriptor: Option<String>,
    pub parameter_descriptors: Vec<String>,
    pub method_descriptor: Option<String>,
    pub modifiers: BTreeSet<JavaModifier>,
    pub range: McTextRange,
    pub annotation_range: McTextRange,
    pub name_range: McTextRange,
    pub parse_source: ParseSource,
    pub confidence: ParseConfidence,
    pub warnings: Vec<String>,
    pub is_method: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodSelector {
    pub value: String,
    pub name: String,
    pub descriptor: Option<String>,
    pub range: McTextRange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtSelectorModel {
    pub value: String,
    pub target: Option<String>,
    pub ordinal: Option<i32>,
    pub range: McTextRange,
    pub target_range: Option<McTextRange>,
    pub ordinal_range: Option<McTextRange>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InjectorModel {
    pub annotation: MixinAnnotation,
    pub annotation_kind: InjectorKind,
    pub method_selectors: Vec<MethodSelector>,
    pub at_selectors: Vec<AtSelectorModel>,
    pub handler_method_descriptor: Option<String>,
    pub range: McTextRange,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InjectorKind {
    Inject,
    Redirect,
    ModifyArg,
    ModifyArgs,
    ModifyVariable,
    ModifyConstant,
    ModifyExpressionValue,
    ModifyReturnValue,
    ModifyReceiver,
    WrapOperation,
    WrapWithCondition,
    WrapMethod,
}
impl InjectorKind {
    pub fn from_annotation(a: MixinAnnotation) -> Option<Self> {
        Some(match a {
            MixinAnnotation::Inject => Self::Inject,
            MixinAnnotation::Redirect => Self::Redirect,
            MixinAnnotation::ModifyArg => Self::ModifyArg,
            MixinAnnotation::ModifyArgs => Self::ModifyArgs,
            MixinAnnotation::ModifyVariable => Self::ModifyVariable,
            MixinAnnotation::ModifyConstant => Self::ModifyConstant,
            MixinAnnotation::ModifyExpressionValue => Self::ModifyExpressionValue,
            MixinAnnotation::ModifyReturnValue => Self::ModifyReturnValue,
            MixinAnnotation::ModifyReceiver => Self::ModifyReceiver,
            MixinAnnotation::WrapOperation => Self::WrapOperation,
            MixinAnnotation::WrapWithCondition => Self::WrapWithCondition,
            MixinAnnotation::WrapMethod => Self::WrapMethod,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticParseDebugInfo {
    pub parse_source: ParseSource,
    pub used_compilation_unit: bool,
    pub used_java_project: bool,
    pub binding_resolved_count: usize,
    pub binding_failed_count: usize,
    pub fallback_reason: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinClassModel {
    pub source_uri: String,
    pub package_name: String,
    pub qualified_name: String,
    pub type_kind: JavaTypeKind,
    pub targets: Vec<MixinTargetRef>,
    pub members: Vec<MixinMemberModel>,
    pub injectors: Vec<InjectorModel>,
    pub resolved_mixin_extras_contexts: Vec<crate::extras_model::ResolvedMixinExtrasContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_handler_source: Option<crate::extras_model::HostHandlerSource>,
    pub parse_source: ParseSource,
    pub confidence: ParseConfidence,
    pub warnings: Vec<String>,
    pub debug_info: SemanticParseDebugInfo,
}

/// Header facts only: no inferred types, synthetic JVM arguments or independent type index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMemberDeclaration {
    pub name: String,
    pub return_type: String,
    pub parameter_types: Vec<String>,
    pub modifiers: BTreeSet<JavaModifier>,
    pub is_method: bool,
    pub start: usize,
    pub end: usize,
    pub name_start: usize,
    pub name_end: usize,
    pub parameter_start: Option<usize>,
    pub parameter_end: Option<usize>,
    pub warnings: Vec<String>,
}
pub fn parse_member_declaration(
    source: &str,
    offset: usize,
    imports: &JavaSourceImports,
) -> Option<SourceMemberDeclaration> {
    if !source.is_char_boundary(offset) {
        return None;
    }
    let ts = source_tokens(source, imports.is_kotlin);
    let mut i = ts.iter().position(|t| t.start >= offset)?;
    while ts.get(i).is_some_and(|t| t.text == "@") {
        let (_, n) = qualified(&ts, i + 1);
        i = n;
        if ts.get(i).is_some_and(|t| t.text == "(") {
            i = matching(&ts, i)? + 1;
        }
    }
    let first = i;
    let start = ts.get(i)?.start;
    let mut modifiers = BTreeSet::new();
    while let Some(t) = ts.get(i) {
        if let Some(m) = JavaModifier::from_name(&t.text) {
            modifiers.insert(m);
            i += 1;
        } else if matches!(
            t.text.as_str(),
            "internal"
                | "open"
                | "override"
                | "inline"
                | "suspend"
                | "tailrec"
                | "external"
                | "infix"
                | "operator"
                | "const"
                | "lateinit"
        ) {
            i += 1;
        } else {
            break;
        }
    }
    let mut warnings = vec![];
    if imports.is_kotlin {
        if ts.get(i)?.text == "fun" {
            i += 1;
            if ts.get(i).is_some_and(|t| t.text == "<") {
                i = matching(&ts, i)? + 1;
            }
            let paren =
                (i..ts.len()).find(|n| matches!(ts[*n].text.as_str(), "(" | "{" | "=" | ";"))?;
            if ts[paren].text != "(" {
                return None;
            }
            let name_token = ts.get(paren.checked_sub(1)?)?;
            if name_token.kind != SourceTokenKind::Identifier {
                return None;
            }
            let close = matching(&ts, paren)?;
            let mut ret = "Unit".to_string();
            let mut end = ts[close].end;
            if ts.get(close + 1).is_some_and(|t| t.text == ":") {
                let first_ret = close + 2;
                let mut ret_end = first_ret;
                let mut depth = 0;
                while ret_end < ts.len() {
                    let t = &ts[ret_end];
                    if depth == 0
                        && (matches!(t.text.as_str(), "=" | "{" | ";" | "}")
                            || source[end..t.start].contains(['\n', '\r']) && ret_end > first_ret)
                    {
                        break;
                    }
                    match t.text.as_str() {
                        "<" | "(" | "[" => depth += 1,
                        ">" | ")" | "]" => depth -= 1,
                        _ => {}
                    }
                    end = t.end;
                    ret_end += 1;
                }
                ret = source[ts.get(first_ret)?.start..end].trim().into();
            } else if ts.get(close + 1).is_some_and(|t| t.text == "=") {
                ret.clear();
                warnings.push("Kotlin expression-body return type is unresolved".into());
            }
            let params = split_top_level(&source[ts[paren].end..ts[close].start], ',')
                .into_iter()
                .map(|p| {
                    let p = strip_annotations(&p);
                    let prefix = if p.trim_start().starts_with("vararg ") {
                        "vararg "
                    } else {
                        ""
                    };
                    let Some((_, ty)) = p.split_once(':') else {
                        return String::new();
                    };
                    format!("{prefix}{}", ty.split('=').next().unwrap_or("").trim())
                })
                .collect();
            if ts[first..i].iter().any(|t| t.text == "suspend") {
                warnings.push("Kotlin suspend JVM lowering requires host semantic facts".into());
            }
            return Some(SourceMemberDeclaration {
                name: name_token.text.clone(),
                return_type: ret,
                parameter_types: params,
                modifiers,
                is_method: true,
                start,
                end,
                name_start: name_token.start,
                name_end: name_token.end,
                parameter_start: Some(ts[paren].end),
                parameter_end: Some(ts[close].start),
                warnings,
            });
        }
        if !matches!(ts.get(i)?.text.as_str(), "val" | "var") {
            return None;
        }
        let name = ts.get(i + 1)?;
        if name.kind != SourceTokenKind::Identifier {
            return None;
        }
        let mut ret = String::new();
        let mut end = name.end;
        if ts.get(i + 2).is_some_and(|t| t.text == ":") {
            let from = i + 3;
            let mut n = from;
            while n < ts.len() {
                let t = &ts[n];
                if matches!(t.text.as_str(), "=" | ";" | "{" | "}")
                    || (n > from && source[end..t.start].contains(['\n', '\r']))
                {
                    break;
                }
                end = t.end;
                n += 1;
            }
            if end > name.end {
                end = source[ts[from].start..]
                    .find(['=', ';', '{', '}', '\r', '\n'])
                    .map_or(source.len(), |n| ts[from].start + n);
                ret = source[ts[from].start..end].trim().into();
            }
        }
        return Some(SourceMemberDeclaration {
            name: name.text.clone(),
            return_type: ret,
            parameter_types: vec![],
            modifiers,
            is_method: false,
            start,
            end,
            name_start: name.start,
            name_end: name.end,
            parameter_start: None,
            parameter_end: None,
            warnings,
        });
    }
    if ts.get(i).is_some_and(|t| t.text == "<") {
        i = matching(&ts, i)? + 1;
    }
    let type_start = i;
    let mut n = i;
    let mut generic = 0;
    let mut method = None;
    let mut header_end = None;
    while n < ts.len() {
        match ts[n].text.as_str() {
            "<" => generic += 1,
            ">" => generic -= 1,
            "(" if generic == 0 => {
                method = Some(n);
                break;
            }
            "=" | ";" | "{" | "}" if generic == 0 => {
                header_end = Some(n);
                break;
            }
            _ => {}
        }
        n += 1;
    }
    if let Some(paren) = method {
        let name = ts.get(paren.checked_sub(1)?)?;
        if name.kind != SourceTokenKind::Identifier || name.start <= ts[type_start].start {
            return None;
        }
        let close = matching(&ts, paren)?;
        let ret = source[ts[type_start].start..name.start].trim().into();
        return Some(SourceMemberDeclaration {
            name: name.text.clone(),
            return_type: ret,
            parameter_types: JavaTypeDescriptorResolver::raw_parameter_types(
                &source[ts[paren].end..ts[close].start],
            ),
            modifiers,
            is_method: true,
            start,
            end: ts
                .iter()
                .skip(close + 1)
                .find(|t| matches!(t.text.as_str(), ";" | "{"))
                .map_or(source.len(), |t| t.end),
            name_start: name.start,
            name_end: name.end,
            parameter_start: Some(ts[paren].end),
            parameter_end: Some(ts[close].start),
            warnings,
        });
    }
    let stop = header_end?;
    if ts[stop].text == "}" {
        return None;
    }
    let name_index = (type_start..stop)
        .rev()
        .find(|n| ts[*n].kind == SourceTokenKind::Identifier)?;
    if name_index == type_start {
        return None;
    }
    let name = &ts[name_index];
    let mut ty = source[ts[type_start].start..name.start].trim().to_string();
    if ts.get(name_index + 1).is_some_and(|t| t.text == "[") {
        ty.push_str("[]");
    }
    Some(SourceMemberDeclaration {
        name: name.text.clone(),
        return_type: ty,
        parameter_types: vec![],
        modifiers,
        is_method: false,
        start,
        end: if ts[stop].text == "=" {
            ts.iter().skip(stop + 1).find(|t| t.text == ";")?.end
        } else {
            ts[stop].end
        },
        name_start: name.start,
        name_end: name.end,
        parameter_start: None,
        parameter_end: None,
        warnings,
    })
}
fn resolved_name(name: &str, imports: &JavaSourceImports) -> Option<String> {
    let root = name.split('.').next()?;
    if imports.ambiguous_explicit.contains_key(root) {
        return None;
    }
    if let Some(f) = imports.explicit.get(root) {
        return Some(format!("{f}{}", &name[root.len()..]));
    }
    if name.contains('.') {
        return Some(name.into());
    }
    for p in &imports.wildcard_packages {
        if p == "io.github.diskria.lapis.annotations" && name == "MappingName"
            || p == "javax.lang.model.element.Modifier"
        {
            return Some(format!("{p}.{name}"));
        }
    }
    None
}
fn raw_annotation_spans(
    source: &str,
    imports: &JavaSourceImports,
) -> Vec<(String, usize, usize, usize, usize)> {
    let ts = source_tokens(source, imports.is_kotlin);
    let mut out = vec![];
    for (i, t) in ts.iter().enumerate() {
        if t.text != "@" {
            continue;
        }
        let (n, next) = qualified(&ts, i + 1);
        if n.is_empty() {
            continue;
        }
        let mut end = ts[next - 1].end;
        let mut bs = end;
        let mut be = end;
        if ts.get(next).is_some_and(|t| t.text == "(") {
            bs = ts[next].end;
            be = matching(&ts, next).map_or(source.len(), |n| ts[n].start);
            end = matching(&ts, next).map_or(source.len(), |n| ts[n].end);
        }
        out.push((n, t.start, end, bs, be));
    }
    out
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowMemberDeclaration {
    pub name: String,
    pub is_method: bool,
    pub descriptor: String,
    pub is_static: bool,
    pub range: McTextRange,
    pub parse_source: ParseSource,
    pub confidence: ParseConfidence,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessorMethodDeclaration {
    pub method_name: String,
    pub return_type_descriptor: Option<String>,
    pub parameter_descriptors: Vec<String>,
    pub explicit_field_name: Option<String>,
    pub range: McTextRange,
    pub parse_source: ParseSource,
    pub confidence: ParseConfidence,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvokerMethodDeclaration {
    pub method_name: String,
    pub return_type_descriptor: Option<String>,
    pub parameter_descriptors: Vec<String>,
    pub explicit_target_name: Option<String>,
    pub range: McTextRange,
    pub parse_source: ParseSource,
    pub confidence: ParseConfidence,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverwriteMethodDeclaration {
    pub name: String,
    pub descriptor: String,
    pub is_static: bool,
    pub range: McTextRange,
    pub parse_source: ParseSource,
    pub confidence: ParseConfidence,
    pub warnings: Vec<String>,
}
pub struct MixinMemberDeclarationParser;
impl MixinMemberDeclarationParser {
    pub fn parse_shadow_declarations(
        source: &str,
        index: Option<&dyn ClassIndex>,
    ) -> Vec<ShadowMemberDeclaration> {
        Self::parse_members(source, index)
            .into_iter()
            .filter(|m| m.annotation_kind == MixinMemberAnnotationKind::Shadow)
            .filter_map(|m| {
                let descriptor = if m.is_method == Some(true) {
                    m.method_descriptor
                } else {
                    m.return_descriptor
                }?;
                Some(ShadowMemberDeclaration {
                    name: m.explicit_target_name.unwrap_or(m.java_name),
                    is_method: m.is_method == Some(true),
                    descriptor,
                    is_static: m.modifiers.contains(&JavaModifier::Static),
                    range: m.range,
                    parse_source: ParseSource::HandWritten,
                    confidence: ParseConfidence::High,
                    warnings: m.warnings,
                })
            })
            .collect()
    }
    pub fn parse_accessor_declarations(
        source: &str,
        index: Option<&dyn ClassIndex>,
    ) -> Vec<AccessorMethodDeclaration> {
        Self::parse_members(source, index)
            .into_iter()
            .filter(|m| {
                m.annotation_kind == MixinMemberAnnotationKind::Accessor
                    && m.method_descriptor.is_some()
            })
            .map(|m| AccessorMethodDeclaration {
                method_name: m.java_name,
                return_type_descriptor: m.return_descriptor,
                parameter_descriptors: m.parameter_descriptors,
                explicit_field_name: m.explicit_target_name,
                range: m.range,
                parse_source: ParseSource::HandWritten,
                confidence: ParseConfidence::High,
                warnings: m.warnings,
            })
            .collect()
    }
    pub fn parse_invoker_declarations(
        source: &str,
        index: Option<&dyn ClassIndex>,
    ) -> Vec<InvokerMethodDeclaration> {
        Self::parse_members(source, index)
            .into_iter()
            .filter(|m| {
                m.annotation_kind == MixinMemberAnnotationKind::Invoker
                    && m.method_descriptor.is_some()
            })
            .map(|m| InvokerMethodDeclaration {
                method_name: m.java_name,
                return_type_descriptor: m.return_descriptor,
                parameter_descriptors: m.parameter_descriptors,
                explicit_target_name: m.explicit_target_name,
                range: m.range,
                parse_source: ParseSource::HandWritten,
                confidence: ParseConfidence::High,
                warnings: m.warnings,
            })
            .collect()
    }
    pub fn parse_overwrite_declarations(
        source: &str,
        index: Option<&dyn ClassIndex>,
    ) -> Vec<OverwriteMethodDeclaration> {
        Self::parse_members(source, index)
            .into_iter()
            .filter(|m| m.annotation_kind == MixinMemberAnnotationKind::Overwrite)
            .filter_map(|m| {
                Some(OverwriteMethodDeclaration {
                    name: m.explicit_target_name.unwrap_or(m.java_name),
                    descriptor: m.method_descriptor?,
                    is_static: m.modifiers.contains(&JavaModifier::Static),
                    range: m.range,
                    parse_source: ParseSource::HandWritten,
                    confidence: ParseConfidence::High,
                    warnings: m.warnings,
                })
            })
            .collect()
    }
    pub fn parse_declaration_diagnostics(
        source: &str,
        index: Option<&dyn ClassIndex>,
    ) -> Vec<crate::McDiagnostic> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        let lookup = index.map(|class_index| ClassIndexJavaTypeLookup { class_index });
        let ctx = JavaTypeResolutionContext {
            imports: &imports,
            lookup: lookup.as_ref().map(|l| l as &dyn JavaTypeLookup),
        };
        let mut result = vec![];
        for a in annotations(source).iter().filter(|a| {
            matches!(
                a.annotation,
                MixinAnnotation::Shadow
                    | MixinAnnotation::Accessor
                    | MixinAnnotation::Invoker
                    | MixinAnnotation::Overwrite
            )
        }) {
            let Some(d) = parse_member_declaration(source, a.end, &imports) else {
                continue;
            };
            let types = std::iter::once(&d.return_type).chain(d.parameter_types.iter());
            for raw in types {
                if imports.is_kotlin && raw.trim().is_empty() {
                    continue;
                }
                let diagnostic =
                    match JavaTypeDescriptorResolver::descriptor_or_diagnostic(raw, &ctx) {
                        TypeDescriptorResult::Resolved { .. } => continue,
                        TypeDescriptorResult::Unresolved {
                            raw_type,
                            normalized_type,
                        } => crate::McDiagnostic {
                            code: "MIXIN_UNRESOLVED_JAVA_TYPE".into(),
                            severity: crate::McSeverity::Error,
                            message: format!(
                            "Unresolved Java type in Mixin member declaration: {normalized_type}"
                        ),
                            range: source_range(source, a.start, d.end),
                            metadata: BTreeMap::from([
                                ("type".into(), raw_type),
                                ("normalizedType".into(), normalized_type),
                            ]),
                        },
                        TypeDescriptorResult::Ambiguous {
                            raw_type,
                            normalized_type,
                            candidates,
                        } => crate::McDiagnostic {
                            code: "MIXIN_UNRESOLVED_HANDLER_DESCRIPTOR".into(),
                            severity: crate::McSeverity::Error,
                            message: format!(
                            "Ambiguous Java type in Mixin member declaration: {normalized_type}"
                        ),
                            range: source_range(source, a.start, d.end),
                            metadata: BTreeMap::from([
                                ("type".into(), raw_type),
                                ("normalizedType".into(), normalized_type),
                                ("candidates".into(), candidates.join(",")),
                            ]),
                        },
                    };
                if !result.contains(&diagnostic) {
                    result.push(diagnostic);
                }
            }
        }
        result
    }
    pub fn parse_members(source: &str, index: Option<&dyn ClassIndex>) -> Vec<MixinMemberModel> {
        let imports = JavaTypeDescriptorResolver::imports_for(source);
        Self::parse_members_with_imports(source, index, &imports)
    }
    pub fn parse_members_with_imports(
        source: &str,
        index: Option<&dyn ClassIndex>,
        imports: &JavaSourceImports,
    ) -> Vec<MixinMemberModel> {
        let lookup = index.map(|class_index| ClassIndexJavaTypeLookup { class_index });
        let ctx = JavaTypeResolutionContext {
            imports: &imports,
            lookup: lookup.as_ref().map(|l| l as &dyn JavaTypeLookup),
        };
        let anns = annotations_with_imports(source, imports);
        let raw_anns = raw_annotation_spans(source, &imports);
        let scopes = type_scopes(source);
        let mut result = vec![];
        for a in anns.iter().filter(|a| {
            matches!(
                a.annotation,
                MixinAnnotation::Shadow
                    | MixinAnnotation::Accessor
                    | MixinAnnotation::Invoker
                    | MixinAnnotation::Overwrite
            )
        }) {
            let Some(mut d) = parse_member_declaration(source, a.end, &imports) else {
                continue;
            };
            let kind = match a.annotation {
                MixinAnnotation::Shadow => MixinMemberAnnotationKind::Shadow,
                MixinAnnotation::Accessor => MixinMemberAnnotationKind::Accessor,
                MixinAnnotation::Invoker => MixinMemberAnnotationKind::Invoker,
                _ => MixinMemberAnnotationKind::Overwrite,
            };
            if !d.is_method && kind != MixinMemberAnnotationKind::Shadow {
                continue;
            }
            let mut block_start = a.start;
            for (_, start, end, _, _) in raw_anns
                .iter()
                .rev()
                .filter(|(_, _, end, _, _)| *end <= a.start)
            {
                if source_tokens(&source[*end..block_start], imports.is_kotlin).is_empty() {
                    block_start = *start;
                } else {
                    break;
                }
            }
            let mapping = raw_anns
                .iter()
                .filter(|(_, start, end, _, _)| block_start <= *start && *end <= d.start)
                .find(|(name, _, _, _, _)| {
                    resolved_name(name, &imports).as_deref()
                        == Some("io.github.diskria.lapis.annotations.MappingName")
                })
                .and_then(|(_, _, _, bs, be)| {
                    source_tokens(&source[*bs..*be], true)
                        .into_iter()
                        .find_map(|t| {
                            if let SourceTokenKind::String {
                                content_start,
                                content_end,
                                raw,
                                ..
                            } = t.kind
                            {
                                JavaStringContentDecoder::decode_with_offsets(
                                    source,
                                    bs + content_start,
                                    bs + content_end,
                                    raw,
                                    true,
                                )
                                .map(|d| d.value)
                            } else {
                                None
                            }
                        })
                });
            if scopes
                .iter()
                .any(|s| s.companion && s.start < d.start && d.start < s.end)
            {
                d.modifiers.insert(JavaModifier::Static);
            }
            if imports.is_kotlin && kind == MixinMemberAnnotationKind::Shadow {
                let ts = source_tokens(&source[a.body_start..a.body_end], true);
                let mut i = 0;
                while i < ts.len() {
                    let (n, next) = qualified(&ts, i);
                    if next == i {
                        i += 1;
                        continue;
                    }
                    if let Some(n) = resolved_name(&n, &imports) {
                        if let Some(m) = n
                            .strip_prefix("javax.lang.model.element.Modifier.")
                            .and_then(JavaModifier::from_name)
                        {
                            d.modifiers.insert(m);
                        }
                    }
                    i = next;
                }
            }
            let method = if d.is_method {
                JavaTypeDescriptorResolver::method_descriptor_or_null(
                    &d.return_type,
                    &d.parameter_types,
                    &ctx,
                )
            } else {
                None
            };
            let ret = if d.is_method {
                method
                    .as_ref()
                    .and_then(|s| s.split_once(')').map(|(_, r)| r.into()))
            } else {
                JavaTypeDescriptorResolver::descriptor_or_null(&d.return_type, &ctx)
            };
            if ret.is_none() {
                d.warnings.push(
                    if imports.is_kotlin {
                        "Kotlin member JVM descriptor is unresolved"
                    } else {
                        "Java member JVM descriptor is unresolved"
                    }
                    .into(),
                );
            }
            let explicit = annotation_values(source, a, "value")
                .first()
                .and_then(|v| v.decoded.as_ref().map(|d| d.value.clone()));
            let target = mapping.or(explicit).or_else(|| {
                matches!(
                    kind,
                    MixinMemberAnnotationKind::Shadow | MixinMemberAnnotationKind::Overwrite
                )
                .then(|| d.name.clone())
            });
            result.push(MixinMemberModel {
                annotation_kind: kind,
                java_name: d.name,
                explicit_target_name: target,
                return_descriptor: ret,
                parameter_descriptors: d
                    .parameter_types
                    .iter()
                    .filter_map(|t| JavaTypeDescriptorResolver::descriptor_or_null(t, &ctx))
                    .collect(),
                method_descriptor: method,
                modifiers: d.modifiers,
                range: source_range(source, a.start, d.end),
                annotation_range: source_range(source, a.start, a.end),
                name_range: source_range(source, d.name_start, d.name_end),
                parse_source: ParseSource::HandWrittenFallback,
                confidence: if d.warnings.is_empty() {
                    ParseConfidence::Medium
                } else {
                    ParseConfidence::Low
                },
                warnings: d.warnings,
                is_method: Some(d.is_method),
            });
        }
        result.sort_by_key(|m| m.range.start);
        result
    }
    pub fn find_member_name_text_range(
        source: &str,
        range: McTextRange,
        member_name: &str,
        annotation_name: &str,
        is_method: bool,
    ) -> Option<McTextRange> {
        let wanted = MixinAnnotation::from_simple_name(annotation_name)?;
        Self::parse_members(source, None)
            .into_iter()
            .find(|m| {
                let a = match m.annotation_kind {
                    MixinMemberAnnotationKind::Shadow => MixinAnnotation::Shadow,
                    MixinMemberAnnotationKind::Accessor => MixinAnnotation::Accessor,
                    MixinMemberAnnotationKind::Invoker => MixinAnnotation::Invoker,
                    MixinMemberAnnotationKind::Overwrite => MixinAnnotation::Overwrite,
                };
                a == wanted
                    && m.range.start >= range.start
                    && m.range.end <= range.end
                    && m.is_method == Some(is_method)
                    && (m.java_name == member_name
                        || m.explicit_target_name.as_deref() == Some(member_name))
            })
            .map(|m| m.name_range)
    }
    pub fn find_shadow_prefix(source: &str) -> Option<String> {
        annotations(source)
            .iter()
            .filter(|a| a.annotation == MixinAnnotation::Shadow)
            .find_map(|a| {
                annotation_values(source, a, "prefix")
                    .first()
                    .map(|v| v.value.clone())
            })
    }
    pub fn find_shadow_remap(source: &str) -> bool {
        !annotations(source)
            .iter()
            .filter(|a| a.annotation == MixinAnnotation::Shadow)
            .flat_map(|a| annotation_attributes(source, a))
            .any(|a| a.name == "remap" && source[a.value_start..a.value_end].trim() == "false")
    }
}
pub struct MixinSemanticModelParser;
impl MixinSemanticModelParser {
    pub fn parse_with_metadata(
        source: &str,
        uri: &str,
        parse_source: ParseSource,
        confidence: ParseConfidence,
        warnings: Vec<String>,
        debug_info: SemanticParseDebugInfo,
        resolved_mixin_extras_contexts: Vec<crate::extras_model::ResolvedMixinExtrasContext>,
    ) -> MixinClassModel {
        let mut model = Self::parse(source, uri);
        model.parse_source = parse_source;
        model.confidence = confidence;
        model.warnings = warnings;
        model.debug_info = debug_info;
        model.resolved_mixin_extras_contexts = resolved_mixin_extras_contexts;
        for member in &mut model.members {
            member.parse_source = parse_source;
            member.confidence = confidence;
        }
        model
    }
    pub fn parse(source: &str, uri: &str) -> MixinClassModel {
        Self::parse_with_lookup(source, uri, None)
    }
    pub fn parse_with_lookup(
        source: &str,
        uri: &str,
        index: Option<&dyn ClassIndex>,
    ) -> MixinClassModel {
        let imports = JavaTypeDescriptorResolver::imports_for_uri(source, uri);
        let mut sites = annotations_with_imports(source, &imports);
        for site in &mut sites {
            if site.body_start > site.name_end && site.body_end == site.end {
                site.end = site.name_end;
                site.body_start = site.name_end;
                site.body_end = site.name_end;
            }
        }
        let mut model = MixinClassModel {
            source_uri: uri.into(),
            package_name: imports.package_name.clone().unwrap_or_default(),
            members: MixinMemberDeclarationParser::parse_members_with_imports(
                source, index, &imports,
            ),
            ..Default::default()
        };
        if !imports.is_kotlin {
            model.members.retain(|m| m.return_descriptor.is_some());
            for m in &mut model.members {
                m.annotation_range = m.range;
                if matches!(
                    m.annotation_kind,
                    MixinMemberAnnotationKind::Shadow | MixinMemberAnnotationKind::Overwrite
                ) {
                    m.parameter_descriptors.clear();
                    m.modifiers.retain(|m| *m == JavaModifier::Static);
                } else {
                    m.modifiers.clear();
                }
            }
        }
        model
            .members
            .sort_by_key(|m| match (imports.is_kotlin, m.annotation_kind) {
                (false, MixinMemberAnnotationKind::Shadow)
                | (true, MixinMemberAnnotationKind::Accessor) => 0,
                (false, MixinMemberAnnotationKind::Invoker)
                | (true, MixinMemberAnnotationKind::Shadow) => 2,
                (false, MixinMemberAnnotationKind::Accessor)
                | (true, MixinMemberAnnotationKind::Invoker) => 1,
                (_, MixinMemberAnnotationKind::Overwrite) => 3,
            });
        if let Some(scope) = type_scopes(source).first() {
            model.qualified_name = imports
                .package_name
                .as_ref()
                .map_or_else(|| scope.name.clone(), |p| format!("{p}.{}", scope.name));
        }
        if model.qualified_name.is_empty() {
            let tokens = source_tokens(source, imports.is_kotlin);
            if let Some((i, _)) = tokens.iter().enumerate().find(|(i, t)| {
                matches!(t.text.as_str(), "class" | "interface" | "enum" | "record")
                    && (*i == 0 || !matches!(tokens[*i - 1].text.as_str(), "." | ":"))
            }) {
                if let Some(name) = tokens.get(i + 1) {
                    model.qualified_name = imports
                        .package_name
                        .as_ref()
                        .map_or_else(|| name.text.clone(), |p| format!("{p}.{}", name.text));
                }
            }
        }
        for a in sites
            .iter()
            .filter(|a| a.annotation == MixinAnnotation::Mixin)
        {
            for attr in ["value", "targets", "target"] {
                for v in annotation_values_with_imports(source, a, attr, &imports) {
                    let value = v.decoded.map_or(v.value, |d| d.value);
                    let raw = value.trim_end_matches(".class").trim_end_matches("::class");
                    let descriptor = imports
                        .is_kotlin
                        .then(|| {
                            JavaTypeDescriptorResolver::descriptor_or_null(
                                raw,
                                &JavaTypeResolutionContext {
                                    imports: &imports,
                                    lookup: None,
                                },
                            )
                        })
                        .flatten();
                    let internal = descriptor
                        .filter(|d| d.starts_with('L'))
                        .map_or_else(|| raw.replace('.', "/"), |d| d[1..d.len() - 1].into());
                    model.targets.push(MixinTargetRef {
                        internal_name: internal,
                        range: source_range(source, v.start, v.end),
                    });
                }
            }
        }
        for a in sites.iter().filter(|a| a.annotation.is_injector()) {
            let method_selectors = annotation_values_with_imports(source, a, "method", &imports)
                .into_iter()
                .filter_map(|v| {
                    let value = v.decoded?.value;
                    let (n, d) = value
                        .split_once('(')
                        .map_or((value.clone(), None), |(n, d)| {
                            (n.into(), Some(format!("({d}")))
                        });
                    let attribute = annotation_attributes_in_language(source, a, imports.is_kotlin)
                        .into_iter()
                        .find(|attr| {
                            attr.name == "method"
                                && attr.explicit
                                && attr.value_start <= v.start
                                && v.end <= attr.value_end
                                && !source[attr.value_start..attr.value_end]
                                    .trim_start()
                                    .starts_with(['[', '{'])
                        });
                    Some(MethodSelector {
                        value,
                        name: n,
                        descriptor: d,
                        range: attribute.map_or_else(
                            || source_range(source, v.start, v.end),
                            |a| source_range(source, a.start, a.end),
                        ),
                    })
                })
                .collect();
            let at_selectors = sites
                .iter()
                .filter(|s| {
                    s.annotation == MixinAnnotation::At && a.start < s.start && s.end <= a.end
                })
                .filter_map(|s| {
                    let value = annotation_values_with_imports(source, s, "value", &imports)
                        .first()?
                        .decoded
                        .as_ref()?
                        .value
                        .clone();
                    let target = annotation_values_with_imports(source, s, "target", &imports)
                        .into_iter()
                        .next();
                    let ordinal = annotation_attributes(source, s)
                        .into_iter()
                        .find(|a| a.name == "ordinal");
                    Some(AtSelectorModel {
                        value,
                        target: target
                            .as_ref()
                            .and_then(|t| t.decoded.as_ref().map(|d| d.value.clone())),
                        ordinal: ordinal
                            .as_ref()
                            .and_then(|o| source[o.value_start..o.value_end].parse().ok()),
                        range: source_range(source, s.start, s.end),
                        target_range: target.map(|t| source_range(source, t.start, t.end)),
                        ordinal_range: ordinal.map(|o| source_range(source, o.start, o.end)),
                    })
                })
                .collect();
            model.injectors.push(InjectorModel {
                annotation: a.annotation,
                annotation_kind: InjectorKind::from_annotation(a.annotation)
                    .expect("filtered injector"),
                method_selectors,
                at_selectors,
                handler_method_descriptor: None,
                range: source_range(source, a.start, a.end),
            });
        }
        model
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MixinCompletionContext {
    MixinTarget {
        annotation_range: McTextRange,
        partial_value: String,
    },
    InjectMethod {
        injector: InjectorModel,
        partial_value: String,
    },
    AtValue {
        injector: InjectorModel,
        at_selector: AtSelectorModel,
        partial_value: String,
    },
    AtTarget {
        injector: InjectorModel,
        at_selector: AtSelectorModel,
        owner: Option<String>,
        method_name: Option<String>,
        method_descriptor: Option<String>,
        partial_value: String,
    },
    AccessorValue {
        member: MixinMemberModel,
        partial_value: String,
    },
    InvokerValue {
        member: MixinMemberModel,
        partial_value: String,
    },
    MemberName {
        member: MixinMemberModel,
        partial_value: String,
        value_start_offset: usize,
        value_end_offset: usize,
    },
}
pub struct SemanticCompletionContextExtractor;
impl SemanticCompletionContextExtractor {
    pub fn extract(
        source: &str,
        line: usize,
        character: usize,
        model: Option<&MixinClassModel>,
    ) -> Option<MixinCompletionContext> {
        let model = model?;
        let offset = position_to_offset(source, McTextPosition { line, character })?;
        let c = AnnotationContextExtractor::extract_at_offset(source, offset)?;
        let contains = |r: McTextRange| {
            r.start <= McTextPosition { line, character }
                && McTextPosition { line, character } <= r.end
        };
        match c.slot {
            AnnotationSlot::Class | AnnotationSlot::Targets => {
                let target = model.targets.iter().find(|t| contains(t.range))?;
                Some(MixinCompletionContext::MixinTarget {
                    annotation_range: target.range,
                    partial_value: c.partial_value,
                })
            }
            AnnotationSlot::Method => Some(MixinCompletionContext::InjectMethod {
                injector: model
                    .injectors
                    .iter()
                    .find(|i| contains(i.range))
                    .cloned()
                    .or_else(|| {
                        if model.targets.is_empty() || !c.annotation.is_injector() {
                            return None;
                        }
                        let before = &source[c.annotation_start_offset..offset];
                        let (_, tail) = before.rsplit_once("method")?;
                        let tail = tail.trim_start().strip_prefix('=')?.trim_start();
                        let value = tail.strip_prefix('"').unwrap_or(tail);
                        if value.contains(['"', ',', ')']) {
                            return None;
                        }
                        Some(InjectorModel {
                            annotation: c.annotation,
                            annotation_kind: InjectorKind::from_annotation(c.annotation)?,
                            method_selectors: vec![],
                            at_selectors: vec![],
                            handler_method_descriptor: None,
                            range: source_range(source, c.annotation_start_offset, offset),
                        })
                    })?,
                partial_value: c.partial_value,
            }),
            AnnotationSlot::Target | AnnotationSlot::Value
                if c.annotation == MixinAnnotation::At =>
            {
                let injector = model.injectors.iter().find(|i| contains(i.range))?.clone();
                let at_selector = injector
                    .at_selectors
                    .iter()
                    .find(|a| contains(a.range))?
                    .clone();
                if c.slot == AnnotationSlot::Target {
                    let m = injector.method_selectors.first();
                    Some(MixinCompletionContext::AtTarget {
                        at_selector,
                        owner: model.targets.first().map(|t| t.internal_name.clone()),
                        method_name: m.map(|m| m.name.clone()),
                        method_descriptor: m.and_then(|m| m.descriptor.clone()),
                        injector,
                        partial_value: c.partial_value,
                    })
                } else {
                    Some(MixinCompletionContext::AtValue {
                        injector,
                        at_selector,
                        partial_value: c.partial_value,
                    })
                }
            }
            AnnotationSlot::AccessorValue => Some(MixinCompletionContext::AccessorValue {
                member: model.members.iter().find(|m| contains(m.range))?.clone(),
                partial_value: c.partial_value,
            }),
            AnnotationSlot::InvokerValue => Some(MixinCompletionContext::InvokerValue {
                member: model.members.iter().find(|m| contains(m.range))?.clone(),
                partial_value: c.partial_value,
            }),
            AnnotationSlot::ShadowMember | AnnotationSlot::OverwriteMethod => {
                Some(MixinCompletionContext::MemberName {
                    member: model.members.iter().find(|m| contains(m.range))?.clone(),
                    partial_value: c.partial_value,
                    value_start_offset: c.value_start_offset,
                    value_end_offset: c.value_end_offset,
                })
            }
            _ => None,
        }
    }
    pub fn to_annotation_context(
        source: &str,
        line: usize,
        character: usize,
        model: Option<&MixinClassModel>,
        context: &MixinCompletionContext,
    ) -> Option<AnnotationContext> {
        let offset = position_to_offset(source, McTextPosition { line, character })?;
        let lexical = AnnotationContextExtractor::extract(source, line, character);
        if JavaTypeDescriptorResolver::imports_for(source).is_kotlin {
            if lexical.is_some() {
                return lexical;
            }
        }
        let (mut c, range) = match context {
            MixinCompletionContext::MixinTarget {
                annotation_range,
                partial_value,
            } => (
                AnnotationContext::new(
                    MixinAnnotation::Mixin,
                    if lexical
                        .as_ref()
                        .is_some_and(|c| c.slot == AnnotationSlot::Targets)
                    {
                        AnnotationSlot::Targets
                    } else {
                        AnnotationSlot::Class
                    },
                    partial_value.clone(),
                ),
                *annotation_range,
            ),
            MixinCompletionContext::InjectMethod {
                injector,
                partial_value,
            } => {
                let mut c = AnnotationContext::new(
                    injector.annotation,
                    AnnotationSlot::Method,
                    partial_value.clone(),
                );
                c.inject_method_name = Some(partial_value.trim_matches('"').into());
                (c, injector.range)
            }
            MixinCompletionContext::AtValue {
                injector,
                at_selector,
                partial_value,
            } => {
                let mut c = AnnotationContext::new(
                    MixinAnnotation::At,
                    AnnotationSlot::Value,
                    partial_value.clone(),
                );
                c.inject_method_name = injector.method_selectors.first().map(|m| m.value.clone());
                c.at_value = Some(partial_value.trim_matches('"').into());
                c.parent_injector_annotation = Some(injector.annotation);
                (c, at_selector.range)
            }
            MixinCompletionContext::AtTarget {
                injector,
                at_selector,
                method_name,
                method_descriptor,
                partial_value,
                ..
            } => {
                let mut c = AnnotationContext::new(
                    MixinAnnotation::At,
                    AnnotationSlot::Target,
                    lexical
                        .as_ref()
                        .map_or_else(|| partial_value.clone(), |c| c.partial_value.clone()),
                );
                c.inject_method_name = method_name
                    .as_ref()
                    .map(|n| format!("{n}{}", method_descriptor.as_deref().unwrap_or("")))
                    .or_else(|| injector.method_selectors.first().map(|m| m.value.clone()));
                c.at_value = Some(at_selector.value.clone());
                c.parent_injector_annotation = Some(injector.annotation);
                (c, at_selector.range)
            }
            MixinCompletionContext::AccessorValue {
                member,
                partial_value,
            } => (
                AnnotationContext::new(
                    MixinAnnotation::Accessor,
                    AnnotationSlot::AccessorValue,
                    partial_value.clone(),
                ),
                member.annotation_range,
            ),
            MixinCompletionContext::InvokerValue {
                member,
                partial_value,
            } => (
                AnnotationContext::new(
                    MixinAnnotation::Invoker,
                    AnnotationSlot::InvokerValue,
                    partial_value.clone(),
                ),
                member.annotation_range,
            ),
            MixinCompletionContext::MemberName {
                member,
                partial_value,
                ..
            } => {
                let a = match member.annotation_kind {
                    MixinMemberAnnotationKind::Shadow => MixinAnnotation::Shadow,
                    MixinMemberAnnotationKind::Overwrite => MixinAnnotation::Overwrite,
                    _ => return None,
                };
                let mut c = AnnotationContext::new(
                    a,
                    if a == MixinAnnotation::Shadow {
                        AnnotationSlot::ShadowMember
                    } else {
                        AnnotationSlot::OverwriteMethod
                    },
                    partial_value.clone(),
                );
                c.shadow_member_is_method = member
                    .is_method
                    .or(Some(member.method_descriptor.is_some()));
                if let Some(lex) = &lexical {
                    c.shadow_prefix = lex.shadow_prefix.clone();
                    c.shadow_remap = lex.shadow_remap;
                    c.resolved_annotation_fqn = lex.resolved_annotation_fqn.clone();
                }
                (c, member.annotation_range)
            }
        };
        c.annotation_start_offset = position_to_offset(source, range.start)?;
        c.annotation_end_offset = position_to_offset(source, range.end)?;
        c.value_start_offset = lexical.as_ref().map_or(offset, |c| c.value_start_offset);
        c.value_end_offset = offset;
        c.mixin_target_internal_names = model.map_or(vec![], |m| {
            m.targets.iter().map(|t| t.internal_name.clone()).collect()
        });
        if matches!(context, MixinCompletionContext::AtTarget { .. }) {
            if let Some(lex) = &lexical {
                c.value_start_offset = lex.value_start_offset;
                c.value_end_offset = lex.value_end_offset;
                c.annotation_start_offset = lex.annotation_start_offset;
                c.annotation_end_offset = lex.annotation_end_offset;
            }
        }
        if let MixinCompletionContext::MemberName {
            value_start_offset,
            value_end_offset,
            ..
        } = context
        {
            c.value_start_offset = *value_start_offset;
            c.value_end_offset = *value_end_offset;
            if let Some(lex) = &lexical {
                c.annotation_start_offset = lex.annotation_start_offset;
                c.annotation_end_offset = lex.annotation_end_offset;
            }
        }
        Some(c)
    }
}
