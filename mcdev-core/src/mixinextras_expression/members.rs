use super::*;
use crate::{
    bytecode::BytecodeIndex, expression_engine as engine, extras_model::*, mixin_source::*,
    mixinextras_handler::HandlerSignatureService, *,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedExpressionReceiver {
    pub receiver: String,
    pub member_prefix: String,
}
pub struct ParsedExpressionReceiverUtf16 {
    pub receiver: Vec<u16>,
    pub member_prefix: String,
}
pub struct ExpressionReceiverParser;
impl ExpressionReceiverParser {
    pub fn parse_utf16(prefix: &[u16], cursor: usize) -> Option<ParsedExpressionReceiverUtf16> {
        let view = Utf16SourceView::new(prefix);
        let cursor = view.byte(cursor.min(prefix.len()));
        let parsed = Self::parse(&view.text, cursor)?;
        let before = view.text[..cursor].trim_end();
        let dot = before.rfind('.')?;
        let end = view.text[..dot].trim_end().len();
        let start = end.checked_sub(parsed.receiver.len())?;
        let receiver = prefix[view.unit(start)..view.unit(end)].to_vec();
        if !engine::is_valid_receiver_expression_utf16(&receiver) {
            return None;
        }
        Some(ParsedExpressionReceiverUtf16 {
            receiver,
            member_prefix: parsed.member_prefix,
        })
    }
    pub fn parse(prefix: &str, cursor: usize) -> Option<ParsedExpressionReceiver> {
        let mut cursor = cursor.min(prefix.len());
        while !prefix.is_char_boundary(cursor) {
            cursor -= 1;
        }
        let lex = ExpressionCompletionLexer::lex(prefix, cursor);
        if lex.position != ExpressionCompletionPosition::AfterDot {
            return None;
        }
        let anchor = if lex.token_end > lex.token_start {
            lex.token_start
        } else {
            cursor
        };
        let before = prefix[..anchor].trim_end();
        if !before.ends_with('.')
            || before[..before.len() - 1].contains("::")
            || prefix[before.len()..].starts_with("class")
        {
            return None;
        }
        let dot = before.len() - 1;
        let member = prefix[dot + 1..cursor].trim_start();
        if !member.is_empty()
            && (!identifier_start(member.chars().next()?) || !member.chars().all(identifier_part))
        {
            return None;
        }
        let end = prefix[..dot].trim_end().len();
        let start = receiver_start(&prefix[..end])?;
        let receiver = prefix[start..end].trim();
        if !Self::is_valid_receiver_expression(receiver) {
            return None;
        }
        Some(ParsedExpressionReceiver {
            receiver: receiver.into(),
            member_prefix: member.into(),
        })
    }
    pub fn is_valid_receiver_expression(receiver: &str) -> bool {
        engine::is_valid_receiver_expression(receiver)
    }
}
fn identifier_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}
fn identifier_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}
fn receiver_start(source: &str) -> Option<usize> {
    let mut stack = vec![];
    let mut index = 0;
    let mut start = 0;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if ch == '"' || ch == '\'' {
            let quote = ch;
            index += 1;
            let mut closed = false;
            while index < source.len() {
                let c = source[index..].chars().next()?;
                index += c.len_utf8();
                if c == '\\' {
                    let next = source[index..].chars().next()?;
                    index += next.len_utf8();
                } else if c == quote {
                    closed = true;
                    break;
                }
            }
            if !closed {
                return None;
            }
            continue;
        }
        if source[index..].starts_with("//") {
            index = source[index..]
                .find('\n')
                .map_or(source.len(), |n| index + n);
            continue;
        }
        if source[index..].starts_with("/*") {
            index += 2 + source[index + 2..].find("*/")? + 2;
            continue;
        }
        match ch {
            '(' | '[' | '{' => stack.push((ch, index)),
            ')' | ']' | '}' => {
                let (open, _) = stack.pop()?;
                if !matches!((open, ch), ('(', ')') | ('[', ']') | ('{', '}')) {
                    return None;
                }
            }
            _ if stack.is_empty() => {
                if "+-*/%&|^=!<>?:,;~".contains(ch) {
                    start = index + ch.len_utf8();
                } else {
                    for keyword in ["return", "throw"] {
                        if source[index..].starts_with(keyword)
                            && (index == 0
                                || source[..index]
                                    .chars()
                                    .next_back()
                                    .is_some_and(|c| !identifier_part(c) && c != '.'))
                            && source[index + keyword.len()..]
                                .chars()
                                .next()
                                .is_none_or(|c| !identifier_part(c))
                        {
                            start = index + keyword.len();
                            index += keyword.len() - 1;
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    if let Some((_, at)) = stack.last() {
        Some(at + 1 + receiver_start(&source[at + 1..])?)
    } else {
        Some(start)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionMemberCacheKey {
    pub receiver_expression: String,
    pub receiver_expression_utf16: Option<Vec<u16>>,
    pub owner_internal_name: String,
    pub method_name: String,
    pub method_descriptor: String,
    pub context_type: engine::MatchContextType,
    pub class_bytes_fingerprint: String,
    pub definition_index_fingerprint: String,
}
pub struct ExpressionMemberCompletionCache {
    capacity: usize,
    entries: Mutex<VecDeque<(ExpressionMemberCacheKey, Vec<engine::MemberCandidate>)>>,
}
impl ExpressionMemberCompletionCache {
    pub const DEFAULT_CAPACITY: usize = 32;
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(VecDeque::new()),
        }
    }
    pub fn get(&self, key: &ExpressionMemberCacheKey) -> Option<Vec<engine::MemberCandidate>> {
        let mut entries = self.entries.lock().unwrap();
        let index = entries.iter().position(|(k, _)| k == key)?;
        let entry = entries.remove(index)?;
        let result = entry.1.clone();
        entries.push_back(entry);
        Some(result)
    }
    pub fn put(&self, key: ExpressionMemberCacheKey, candidates: Vec<engine::MemberCandidate>) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(index) = entries.iter().position(|(k, _)| k == &key) {
            entries.remove(index);
        }
        entries.push_back((key, candidates));
        while entries.len() > self.capacity {
            entries.pop_front();
        }
    }
}
impl Default for ExpressionMemberCompletionCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpressionMemberCompletionServiceResult {
    Available {
        candidates: Vec<engine::MemberCandidate>,
    },
    Empty,
    Unavailable,
}
pub trait ExpressionMemberMatcherRunner: Send + Sync {
    fn complete_reachable_members_utf16(
        &self,
        bytes: &[u8],
        name: &str,
        descriptor: &str,
        receiver: &[u16],
        context: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation> {
        match String::from_utf16(receiver) {
            Ok(s) => self.complete_reachable_members(bytes, name, descriptor, &s, context),
            Err(_) => Ok(engine::MemberCompletionResult::Unavailable {
                reason: "The supplied expression matcher has no lossless UTF-16 capability".into(),
                cause: None,
            }),
        }
    }
    fn complete_reachable_members(
        &self,
        bytes: &[u8],
        name: &str,
        descriptor: &str,
        receiver: &str,
        context: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation>;
}
pub struct DefaultExpressionMemberMatcherRunner;
impl ExpressionMemberMatcherRunner for DefaultExpressionMemberMatcherRunner {
    fn complete_reachable_members_utf16(
        &self,
        bytes: &[u8],
        name: &str,
        descriptor: &str,
        receiver: &[u16],
        context: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation> {
        engine::complete_receiver_members_utf16(bytes, name, descriptor, receiver, "", context)
    }
    fn complete_reachable_members(
        &self,
        bytes: &[u8],
        name: &str,
        descriptor: &str,
        receiver: &str,
        context: &engine::MatchContext<'_>,
    ) -> Result<engine::MemberCompletionResult, engine::Cancellation> {
        engine::complete_receiver_members(bytes, name, descriptor, receiver, "", context)
    }
}
pub struct ExpressionMemberCompletionService<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub bytecode_index: &'a dyn BytecodeIndex,
    pub reachable_members_cache: Arc<ExpressionMemberCompletionCache>,
    pub matcher_runner: Arc<dyn ExpressionMemberMatcherRunner>,
}
impl<'a> ExpressionMemberCompletionService<'a> {
    pub fn new(class_index: &'a dyn ClassIndex, bytecode_index: &'a dyn BytecodeIndex) -> Self {
        Self {
            class_index,
            bytecode_index,
            reachable_members_cache: Arc::new(ExpressionMemberCompletionCache::default()),
            matcher_runner: Arc::new(DefaultExpressionMemberMatcherRunner),
        }
    }
    pub fn complete_members(
        &self,
        source: &str,
        context: &AnnotationContext,
        owners: &[String],
        resolved_contexts: &[ResolvedMixinExtrasContext],
        cancellation: &dyn engine::CancellationChecker,
    ) -> Result<ExpressionMemberCompletionServiceResult, engine::Cancellation> {
        use ExpressionMemberCompletionServiceResult as ResultKind;
        if !matches!(
            context.annotation,
            MixinAnnotation::Expression | MixinAnnotation::Expressions
        ) || context.slot != AnnotationSlot::Value
            || context.expression_completion_position
                != Some(ExpressionCompletionPosition::AfterDot)
        {
            return Ok(ResultKind::Empty);
        }
        let parsed = if let Some(decoded) = &context.decoded_expression_prefix_utf16 {
            ExpressionReceiverParser::parse_utf16(&decoded.prefix, decoded.cursor)
        } else {
            context
                .decoded_expression_prefix
                .as_ref()
                .and_then(|d| ExpressionReceiverParser::parse(&d.prefix, d.cursor))
                .map(|p| ParsedExpressionReceiverUtf16 {
                    receiver: p.receiver.encode_utf16().collect(),
                    member_prefix: p.member_prefix,
                })
        };
        let Some(parsed) = parsed else {
            return Ok(ResultKind::Empty);
        };
        let Some(site) =
            ExpressionContextResolver::find_enclosing_site(source, context.value_start_offset)
        else {
            return Ok(ResultKind::Unavailable);
        };
        if !site
            .at_value
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("MIXINEXTRAS:EXPRESSION"))
        {
            return Ok(ResultKind::Unavailable);
        }
        let Some(kind) = official_context_type(site.annotation) else {
            return Ok(ResultKind::Unavailable);
        };
        let signatures = HandlerSignatureService::new(self.class_index, Some(self.bytecode_index));
        let Some(target) = signatures.resolve_target_method(owners, &site.method_attribute) else {
            return Ok(ResultKind::Unavailable);
        };
        let Some(expression_context) =
            ExpressionContextResolver::resolve_expression_context_for_site(
                source,
                &site,
                resolved_contexts,
            )
        else {
            return Ok(ResultKind::Unavailable);
        };
        let resolver = ClassLiteralTypeNameResolver::for_source(source, self.class_index);
        let pool = engine::build_identifier_pool(
            &definition_inputs(&expression_context.definition_index),
            &|name| resolver.resolve(name),
        )
        .pool;
        let fingerprint =
            definition_index_semantic_fingerprint(&expression_context.definition_index, &resolver);
        let owners: Vec<_> = MixinTargetResolver::resolve_targets(owners, self.class_index, None)
            .into_iter()
            .filter(|owner| {
                self.class_index
                    .get_methods(owner)
                    .iter()
                    .any(|m| m.name == target.name && m.descriptor == target.descriptor)
            })
            .collect();
        if owners.is_empty() {
            return Ok(ResultKind::Unavailable);
        }
        let common =
            |left: &str, right: &str| self.bytecode_index.resolve_common_super_class(left, right);
        let match_context = engine::MatchContext {
            context_type: kind,
            identifier_pool: &pool,
            allow_incomplete_list_inputs: false,
            common_super_class: &common,
            cancellation,
        };
        let mut merged = vec![];
        let mut seen = BTreeSet::new();
        for owner in owners {
            cancellation.check_cancelled()?;
            let Some(reachable) = self.reachable_members_for_owner(
                &owner,
                &parsed.receiver,
                &target,
                &match_context,
                &fingerprint,
            )?
            else {
                return Ok(ResultKind::Unavailable);
            };
            for candidate in reachable {
                let key = (
                    candidate.kind,
                    candidate.owner_internal_name.clone(),
                    candidate.name.clone(),
                    candidate.descriptor.clone(),
                );
                if seen.insert(key) {
                    merged.push(candidate);
                }
            }
        }
        merged.retain(|c| c.name.starts_with(&parsed.member_prefix));
        Ok(if merged.is_empty() {
            ResultKind::Empty
        } else {
            ResultKind::Available { candidates: merged }
        })
    }
    fn reachable_members_for_owner(
        &self,
        owner: &str,
        receiver: &[u16],
        target: &MethodIndexEntry,
        context: &engine::MatchContext<'_>,
        fingerprint: &str,
    ) -> Result<Option<Vec<engine::MemberCandidate>>, engine::Cancellation> {
        context.cancellation.check_cancelled()?;
        let Some(bytes) = self.bytecode_index.get_class_bytes(owner) else {
            return Ok(None);
        };
        let key = ExpressionMemberCacheKey {
            receiver_expression: String::from_utf16(receiver).unwrap_or_default(),
            receiver_expression_utf16: String::from_utf16(receiver)
                .is_err()
                .then(|| receiver.to_vec()),
            owner_internal_name: owner.into(),
            method_name: target.name.clone(),
            method_descriptor: target.descriptor.clone(),
            context_type: context.context_type,
            class_bytes_fingerprint: format!("{:x}", Sha256::digest(&bytes)),
            definition_index_fingerprint: fingerprint.into(),
        };
        if let Some(cached) = self.reachable_members_cache.get(&key) {
            context.cancellation.check_cancelled()?;
            return Ok(Some(cached));
        }
        match self.matcher_runner.complete_reachable_members_utf16(
            &bytes,
            &target.name,
            &target.descriptor,
            receiver,
            context,
        )? {
            engine::MemberCompletionResult::Unavailable { .. } => Ok(None),
            engine::MemberCompletionResult::Available(candidates) => {
                self.reachable_members_cache.put(key, candidates.clone());
                Ok(Some(candidates))
            }
        }
    }
}
pub fn official_context_type(
    annotation: MixinExtrasAnnotation,
) -> Option<engine::MatchContextType> {
    use engine::MatchContextType as C;
    use MixinExtrasAnnotation as A;
    Some(match annotation {
        A::ModifyExpressionValue => C::ModifyExpressionValue,
        A::ModifyReturnValue => C::ModifyReturnValue,
        A::ModifyReceiver => C::ModifyReceiver,
        A::WrapOperation | A::WrapMethod => C::WrapOperation,
        A::WrapWithCondition => C::WrapWithCondition,
        A::Inject => C::Inject,
        A::Redirect => C::Redirect,
        A::ModifyArg => C::ModifyArg,
        A::ModifyArgs => C::ModifyArgs,
        A::ModifyVariable => C::ModifyVariable,
        A::ModifyConstant => C::ModifyConstant,
        _ => return None,
    })
}
pub fn definition_index_semantic_fingerprint(
    index: &MixinExtrasDefinitionIndex,
    resolver: &ClassLiteralTypeNameResolver,
) -> String {
    let resolved = |name: &str| {
        resolver
            .resolve(name)
            .unwrap_or_else(|| format!("unresolved:{name}"))
    };
    index
        .definitions
        .iter()
        .map(|d| {
            let locals = d
                .local_specs
                .iter()
                .map(|l| {
                    format!(
                        "{}|{}|{}|{}|{}|{}",
                        l.args_only,
                        l.index.map_or("null".into(), |v| v.to_string()),
                        l.ordinal.map_or("null".into(), |v| v.to_string()),
                        l.names.iter().cloned().collect::<Vec<_>>().join(","),
                        l.print,
                        l.type_class_name.as_deref().map_or("null".into(), resolved)
                    )
                })
                .collect::<Vec<_>>()
                .join("\u{2}");
            [
                d.id.clone().unwrap_or_default(),
                d.raw_method_references.join("\u{2}"),
                d.raw_field_references.join("\u{2}"),
                d.class_literal_type_names
                    .iter()
                    .map(|n| resolved(n))
                    .collect::<Vec<_>>()
                    .join("\u{2}"),
                locals,
                d.remap.map_or(String::new(), |v| v.to_string()),
            ]
            .join("\u{1}")
        })
        .collect::<Vec<_>>()
        .join("\0")
}
