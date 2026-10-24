use serde::{Deserialize, Serialize};
// Access transformer domain services. Owners stay named; member namespaces come from project mappings.
use crate::{
    awat::*,
    descriptor::parse_method_descriptor,
    mapping::{FieldRef, MappingLookupResult, MethodRef, ProjectMappingContext},
    model::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessTransformerModifier {
    Public,
    Protected,
    Default,
    Private,
    PublicRemoveFinal,
    ProtectedRemoveFinal,
    PrivateRemoveFinal,
    PublicAddFinal,
    ProtectedAddFinal,
    PrivateAddFinal,
}
impl AccessTransformerModifier {
    pub const ALL: [Self; 10] = [
        Self::Public,
        Self::Protected,
        Self::Default,
        Self::Private,
        Self::PublicRemoveFinal,
        Self::ProtectedRemoveFinal,
        Self::PrivateRemoveFinal,
        Self::PublicAddFinal,
        Self::ProtectedAddFinal,
        Self::PrivateAddFinal,
    ];
    pub fn token(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Protected => "protected",
            Self::Default => "default",
            Self::Private => "private",
            Self::PublicRemoveFinal => "public-f",
            Self::ProtectedRemoveFinal => "protected-f",
            Self::PrivateRemoveFinal => "private-f",
            Self::PublicAddFinal => "public+f",
            Self::ProtectedAddFinal => "protected+f",
            Self::PrivateAddFinal => "private+f",
        }
    }
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.token() == token)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTransformerEntry {
    pub modifier: AccessTransformerModifier,
    pub owner: String,
    pub name: Option<String>,
    pub descriptor: Option<String>,
    pub line: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTransformerFile {
    pub entries: Vec<AccessTransformerEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTransformerParseFailure {
    pub line: usize,
    pub message: String,
}
pub type AccessTransformerParseResult =
    Result<AccessTransformerFile, AccessTransformerParseFailure>;
pub struct AccessTransformerParser;
impl AccessTransformerParser {
    pub fn parse(text: &str) -> AccessTransformerParseResult {
        let fail = |line, message: &str| AccessTransformerParseFailure {
            line,
            message: message.into(),
        };
        let mut entries = vec![];
        for (i, raw) in text.split('\n').enumerate() {
            let line = i + 1;
            let t = tokens(raw, 0);
            if t.is_empty() {
                continue;
            }
            let modifier = AccessTransformerModifier::parse(t[0].text)
                .ok_or_else(|| fail(line, "invalid access transformer modifier"))?;
            if !(2..=3).contains(&t.len()) {
                return Err(fail(
                    line,
                    "entry must contain modifier, owner, and optional member",
                ));
            }
            let (name, descriptor) = match t.get(2) {
                None => (None, None),
                Some(token) => match token.text.find('(') {
                    None => (Some(token.text.into()), None),
                    Some(at) => {
                        let descriptor = &token.text[at..];
                        if parse_method_descriptor(descriptor).is_err() {
                            return Err(fail(line, "invalid method descriptor"));
                        }
                        (Some(token.text[..at].into()), Some(descriptor.into()))
                    }
                },
            };
            entries.push(AccessTransformerEntry {
                modifier,
                owner: t[1].text.into(),
                name,
                descriptor,
                line,
            });
        }
        Ok(AccessTransformerFile { entries })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtSlot {
    Modifier,
    Owner,
    MemberName,
    MemberDescriptor,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtContext {
    pub slot: AtSlot,
    pub partial_value: String,
    pub line_number: usize,
    pub line_start_offset: usize,
    pub line_end_offset: usize,
    pub value_start_offset: usize,
    pub value_end_offset: usize,
    pub modifier: Option<AccessTransformerModifier>,
    pub owner: Option<String>,
    pub member_name: Option<String>,
    pub member_descriptor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtLineToken {
    pub text: String,
    pub start_offset: usize,
    pub end_offset: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtLineContext {
    pub line_number: usize,
    pub line_start_offset: usize,
    pub line_end_offset: usize,
    pub content: String,
    pub modifier: Option<AtLineToken>,
    pub owner: Option<AtLineToken>,
    pub member: Option<AtLineToken>,
}
pub struct AtContextExtractor;
impl AtContextExtractor {
    pub fn extract(text: &str, line: usize, character: usize) -> Option<AtContext> {
        Self::extract_at_offset(text, AtTextPositions::to_offset(text, line, character)?)
    }
    pub fn parse_line(text: &str, line_number: usize) -> Option<AtLineContext> {
        let (s, e, _) = line_bounds(text, line_number)?;
        let raw = &text[s..e];
        let content = raw.split('#').next().unwrap_or("");
        let ts = tokens(content, s);
        let token = |i| {
            ts.get(i).map(|t: &Token<'_>| AtLineToken {
                text: t.text.into(),
                start_offset: t.start,
                end_offset: t.end,
            })
        };
        Some(AtLineContext {
            line_number,
            line_start_offset: s,
            line_end_offset: s + content.len(),
            content: content.trim_end().into(),
            modifier: token(0),
            owner: token(1),
            member: token(2),
        })
    }
    pub fn extract_at_offset(text: &str, offset: usize) -> Option<AtContext> {
        if !text.is_char_boundary(offset) {
            return None;
        }
        let line = text[..offset].bytes().filter(|b| *b == b'\n').count() + 1;
        let l = Self::parse_line(text, line)?;
        let member = l.member.as_ref();
        let paren = member.and_then(|m| m.text.find('('));
        let descriptor_start = member.zip(paren).map(|(m, p)| m.start_offset + p);
        let name = member.map(|m| m.text[..paren.unwrap_or(m.text.len())].to_string());
        let descriptor = member.zip(paren).map(|(m, p)| m.text[p..].to_string());
        let slot = if l.content.is_empty() {
            AtSlot::Modifier
        } else if offset > l.line_start_offset + l.content.len() {
            if l.modifier.is_none() {
                AtSlot::Modifier
            } else if l.owner.is_none() {
                AtSlot::Owner
            } else if member.is_none() {
                AtSlot::MemberName
            } else if descriptor_start.is_none() {
                AtSlot::MemberDescriptor
            } else {
                AtSlot::MemberName
            }
        } else if l.modifier.as_ref().is_none_or(|m| offset <= m.end_offset) {
            AtSlot::Modifier
        } else if l.owner.as_ref().is_none_or(|o| offset <= o.end_offset) {
            AtSlot::Owner
        } else if descriptor_start.is_some_and(|s| offset >= s) {
            AtSlot::MemberDescriptor
        } else {
            AtSlot::MemberName
        };
        let token = match slot {
            AtSlot::Modifier => l.modifier.as_ref(),
            AtSlot::Owner => l.owner.as_ref(),
            _ => member,
        };
        let start = if slot == AtSlot::MemberDescriptor {
            descriptor_start
        } else {
            token.map(|t| t.start_offset)
        }
        .unwrap_or(offset);
        let end = if slot == AtSlot::MemberDescriptor && descriptor_start.is_none() {
            offset
        } else {
            token.map_or(offset, |t| t.end_offset)
        };
        let partial = token.map_or(String::new(), |t| {
            let base = if slot == AtSlot::MemberDescriptor {
                descriptor_start.unwrap_or(t.end_offset)
            } else {
                t.start_offset
            };
            let limit = if slot == AtSlot::MemberName {
                descriptor_start.unwrap_or(t.end_offset)
            } else {
                t.end_offset
            };
            text[base..offset.clamp(base, limit)].into()
        });
        Some(AtContext {
            slot,
            partial_value: partial,
            line_number: line,
            line_start_offset: l.line_start_offset,
            line_end_offset: l.line_end_offset,
            value_start_offset: start,
            value_end_offset: end,
            modifier: l
                .modifier
                .and_then(|t| AccessTransformerModifier::parse(&t.text)),
            owner: l.owner.map(|t| t.text),
            member_name: name,
            member_descriptor: descriptor,
        })
    }
}
pub struct AtTextPositions;
impl AtTextPositions {
    pub fn to_offset(text: &str, line: usize, character: usize) -> Option<usize> {
        clamped_position_to_offset(text, line, character)
    }
    pub fn offset_to_position(text: &str, offset: usize) -> Option<McTextPosition> {
        offset_to_position(text, offset)
    }
    pub fn range_for_offsets(text: &str, start: usize, end: usize) -> Option<McTextRange> {
        Some(McTextRange {
            start: offset_to_position(text, start)?,
            end: offset_to_position(text, end)?,
        })
    }
    pub fn line_range(text: &str, line: usize) -> Option<McTextRange> {
        let (s, e, _) = line_bounds(text, line)?;
        Some(range(text, s, e))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtInsertResult {
    pub insert_text: String,
    pub mapping_found: bool,
}
pub struct AtMemberInsertFormatter;
impl AtMemberInsertFormatter {
    pub fn format_owner_insert(c: &ClassIndexEntry) -> String {
        c.fqn()
    }
    pub fn format_method_insert(
        method: &MethodIndexEntry,
        owner: &str,
        mappings: Option<&ProjectMappingContext>,
    ) -> AtInsertResult {
        let fallback = format!("{}{}", method.name, method.descriptor);
        let Some(m) = mappings else {
            return AtInsertResult {
                insert_text: fallback,
                mapping_found: true,
            };
        };
        let target = m.at_namespace.unwrap_or(m.source_namespace);
        if m.source_namespace == target {
            return AtInsertResult {
                insert_text: fallback,
                mapping_found: true,
            };
        }
        if let MappingLookupResult::Found(r) = m.resolver.remap_method(
            &MethodRef {
                owner: owner.into(),
                name: method.name.clone(),
                descriptor: method.descriptor.clone(),
                namespace: m.source_namespace,
            },
            target,
        ) {
            AtInsertResult {
                insert_text: format!("{}{}", r.name, r.descriptor),
                mapping_found: true,
            }
        } else {
            AtInsertResult {
                insert_text: fallback,
                mapping_found: false,
            }
        }
    }
    pub fn format_field_insert(
        field: &FieldIndexEntry,
        owner: &str,
        mappings: Option<&ProjectMappingContext>,
    ) -> AtInsertResult {
        let Some(m) = mappings else {
            return AtInsertResult {
                insert_text: field.name.clone(),
                mapping_found: true,
            };
        };
        let target = m.at_namespace.unwrap_or(m.source_namespace);
        if m.source_namespace == target {
            return AtInsertResult {
                insert_text: field.name.clone(),
                mapping_found: true,
            };
        }
        if let MappingLookupResult::Found(r) = m.resolver.remap_field(
            &FieldRef {
                owner: owner.into(),
                name: field.name.clone(),
                descriptor: field.descriptor.clone(),
                namespace: m.source_namespace,
            },
            target,
        ) {
            AtInsertResult {
                insert_text: r.name,
                mapping_found: true,
            }
        } else {
            AtInsertResult {
                insert_text: field.name.clone(),
                mapping_found: false,
            }
        }
    }
    pub fn remap_method_for_entry(
        owner: &str,
        method: &MethodIndexEntry,
        m: &ProjectMappingContext,
    ) -> AtInsertResult {
        match m.resolver.remap_method(
            &MethodRef {
                owner: owner.into(),
                name: method.name.clone(),
                descriptor: method.descriptor.clone(),
                namespace: m.source_namespace,
            },
            m.at_namespace.unwrap_or(m.source_namespace),
        ) {
            MappingLookupResult::Found(r) => AtInsertResult {
                insert_text: format!("{}{}", r.name, r.descriptor),
                mapping_found: true,
            },
            _ => AtInsertResult {
                insert_text: format!("{}{}", method.name, method.descriptor),
                mapping_found: false,
            },
        }
    }
    pub fn remap_field_for_entry(
        owner: &str,
        field: &FieldIndexEntry,
        m: &ProjectMappingContext,
    ) -> AtInsertResult {
        match m.resolver.remap_field(
            &FieldRef {
                owner: owner.into(),
                name: field.name.clone(),
                descriptor: field.descriptor.clone(),
                namespace: m.source_namespace,
            },
            m.at_namespace.unwrap_or(m.source_namespace),
        ) {
            MappingLookupResult::Found(r) => AtInsertResult {
                insert_text: r.name,
                mapping_found: true,
            },
            _ => AtInsertResult {
                insert_text: field.name.clone(),
                mapping_found: false,
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAtMember {
    pub kind: MemberKind,
    pub named_name: String,
    pub descriptor: Option<String>,
    pub method: Option<MethodIndexEntry>,
    pub field: Option<FieldIndexEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtMemberResolution {
    Found(ResolvedAtMember),
    NotFound,
    WrongNamespace {
        named_name: String,
        expected_namespace: MappingNamespace,
        actual_name: String,
    },
    MappingNotFound {
        named_name: String,
        kind: MemberKind,
    },
    DescriptorMismatch {
        named_name: String,
        descriptor: String,
        candidates: Vec<MethodIndexEntry>,
    },
    MissingDescriptor {
        named_name: String,
    },
}
pub struct AtMemberResolver;
impl AtMemberResolver {
    pub fn resolve(
        owner: &str,
        name: &str,
        descriptor: Option<&str>,
        index: &dyn ClassIndex,
        mappings: Option<&ProjectMappingContext>,
    ) -> AtMemberResolution {
        let methods = index
            .get_methods(owner)
            .into_iter()
            .filter(|m| m.name == name)
            .collect::<Vec<_>>();
        if !methods.is_empty() {
            let method = if let Some(d) = descriptor {
                methods.iter().find(|m| m.descriptor == d)
            } else if methods.len() == 1 {
                methods.first()
            } else {
                return AtMemberResolution::MissingDescriptor {
                    named_name: name.into(),
                };
            };
            let Some(method) = method else {
                return AtMemberResolution::DescriptorMismatch {
                    named_name: name.into(),
                    descriptor: descriptor.unwrap_or("").into(),
                    candidates: methods,
                };
            };
            let insert = AtMemberInsertFormatter::format_method_insert(method, owner, mappings);
            if mappings.is_some_and(|m| m.at_namespace.is_some_and(|n| n != m.source_namespace)) {
                if !insert.mapping_found {
                    return AtMemberResolution::MappingNotFound {
                        named_name: name.into(),
                        kind: MemberKind::Method,
                    };
                }
                if insert.insert_text.split('(').next() != Some(name) {
                    return AtMemberResolution::WrongNamespace {
                        named_name: name.into(),
                        expected_namespace: mappings.unwrap().at_namespace.unwrap(),
                        actual_name: name.into(),
                    };
                }
            }
            return AtMemberResolution::Found(ResolvedAtMember {
                kind: MemberKind::Method,
                named_name: name.into(),
                descriptor: descriptor.map(str::to_string),
                method: Some(method.clone()),
                field: None,
            });
        }
        let fields = index
            .get_fields(owner)
            .into_iter()
            .filter(|f| f.name == name)
            .collect::<Vec<_>>();
        if !fields.is_empty() {
            let field = if let Some(d) = descriptor {
                fields.iter().find(|f| f.descriptor == d)
            } else if fields.len() == 1 {
                fields.first()
            } else {
                return AtMemberResolution::MissingDescriptor {
                    named_name: name.into(),
                };
            };
            let Some(field) = field else {
                return AtMemberResolution::NotFound;
            };
            let insert = AtMemberInsertFormatter::format_field_insert(field, owner, mappings);
            if mappings.is_some_and(|m| m.at_namespace.is_some_and(|n| n != m.source_namespace)) {
                if !insert.mapping_found {
                    return AtMemberResolution::MappingNotFound {
                        named_name: name.into(),
                        kind: MemberKind::Field,
                    };
                }
                if insert.insert_text != name {
                    return AtMemberResolution::WrongNamespace {
                        named_name: name.into(),
                        expected_namespace: mappings.unwrap().at_namespace.unwrap(),
                        actual_name: name.into(),
                    };
                }
            }
            return AtMemberResolution::Found(ResolvedAtMember {
                kind: MemberKind::Field,
                named_name: name.into(),
                descriptor: Some(field.descriptor.clone()),
                method: None,
                field: Some(field.clone()),
            });
        }
        if let Some(m) = mappings.filter(|m| m.at_namespace.is_some()) {
            for method in index.get_methods(owner) {
                let insert = AtMemberInsertFormatter::remap_method_for_entry(owner, &method, m);
                let p = insert.insert_text.find('(');
                if insert.insert_text[..p.unwrap_or(insert.insert_text.len())] != *name {
                    continue;
                }
                if !insert.mapping_found {
                    return AtMemberResolution::MappingNotFound {
                        named_name: method.name,
                        kind: MemberKind::Method,
                    };
                }
                if descriptor
                    .zip(p)
                    .is_some_and(|(d, p)| d != &insert.insert_text[p..])
                {
                    continue;
                }
                return AtMemberResolution::Found(ResolvedAtMember {
                    kind: MemberKind::Method,
                    named_name: method.name.clone(),
                    descriptor: Some(descriptor.unwrap_or(&method.descriptor).into()),
                    method: Some(method),
                    field: None,
                });
            }
            for field in index.get_fields(owner) {
                let insert = AtMemberInsertFormatter::remap_field_for_entry(owner, &field, m);
                if insert.insert_text != name {
                    continue;
                }
                if !insert.mapping_found {
                    return AtMemberResolution::MappingNotFound {
                        named_name: field.name,
                        kind: MemberKind::Field,
                    };
                }
                return AtMemberResolution::Found(ResolvedAtMember {
                    kind: MemberKind::Field,
                    named_name: field.name.clone(),
                    descriptor: Some(field.descriptor.clone()),
                    method: None,
                    field: Some(field),
                });
            }
        }
        AtMemberResolution::NotFound
    }
}
pub struct AccessTransformerCompletionService<'a> {
    index: &'a dyn ClassIndex,
    mappings: Option<&'a ProjectMappingContext>,
}
impl<'a> AccessTransformerCompletionService<'a> {
    pub fn new(index: &'a dyn ClassIndex, mappings: Option<&'a ProjectMappingContext>) -> Self {
        Self { index, mappings }
    }
    pub fn complete(&self, c: &AtContext) -> Vec<McCompletionItem> {
        let p = &c.partial_value;
        match c.slot {
            AtSlot::Modifier => AccessTransformerModifier::ALL
                .into_iter()
                .filter(|m| m.token().starts_with(p))
                .map(|m| {
                    let t = m.token();
                    completion(
                        t.into(),
                        Some("access modifier".into()),
                        None,
                        t.into(),
                        t.into(),
                        McCompletionKind::Keyword,
                        format!("0100_{t}"),
                        McCompletionMetadata {
                            source: "at.modifier".into(),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            AtSlot::Owner => self
                .index
                .find_classes(p.rsplit('.').next().unwrap_or(p), CLASS_COMPLETION_LIMIT)
                .into_iter()
                .map(|c| {
                    completion(
                        c.simple_name.clone(),
                        Some(c.fqn()),
                        Some(c.internal_name.clone()),
                        format!("{} {}", c.simple_name, c.fqn()),
                        c.fqn(),
                        McCompletionKind::Class,
                        format!("0200_{}", c.simple_name),
                        McCompletionMetadata {
                            source: "at.class".into(),
                            owner: Some(c.internal_name),
                            name: Some(c.simple_name),
                            namespace: Some("NAMED".into()),
                            descriptor: None,
                        },
                    )
                })
                .collect(),
            AtSlot::MemberDescriptor => vec![],
            AtSlot::MemberName => {
                let Some(owner) = c.owner.as_deref().and_then(|o| find_owner(self.index, o)) else {
                    return vec![];
                };
                let mut items = vec![];
                let ns = self
                    .mappings
                    .and_then(|m| m.at_namespace)
                    .map(|n| format!("{n:?}").to_uppercase());
                for m in self
                    .index
                    .get_methods(&owner.internal_name)
                    .into_iter()
                    .filter(|m| m.name.starts_with(p))
                {
                    let insert = AtMemberInsertFormatter::format_method_insert(
                        &m,
                        &owner.internal_name,
                        self.mappings,
                    )
                    .insert_text;
                    items.push(completion(
                        m.readable_signature.clone(),
                        Some(format!("named: {}", m.name)),
                        Some(insert.clone()),
                        format!("{} {}", m.name, m.readable_signature),
                        insert,
                        McCompletionKind::Value,
                        format!("0300_{}", m.name),
                        McCompletionMetadata {
                            source: "at.member.method".into(),
                            owner: Some(owner.internal_name.clone()),
                            name: Some(m.name),
                            descriptor: Some(m.descriptor),
                            namespace: ns.clone(),
                        },
                    ));
                }
                for f in self
                    .index
                    .get_fields(&owner.internal_name)
                    .into_iter()
                    .filter(|f| f.name.starts_with(p))
                {
                    let insert = AtMemberInsertFormatter::format_field_insert(
                        &f,
                        &owner.internal_name,
                        self.mappings,
                    )
                    .insert_text;
                    items.push(completion(
                        format!("{}: {}", f.name, f.readable_type),
                        Some(format!("named: {}", f.name)),
                        Some(insert.clone()),
                        format!("{} {}", f.name, f.readable_type),
                        insert,
                        McCompletionKind::Field,
                        format!("0400_{}", f.name),
                        McCompletionMetadata {
                            source: "at.member.field".into(),
                            owner: Some(owner.internal_name.clone()),
                            name: Some(f.name),
                            descriptor: Some(f.descriptor),
                            namespace: ns.clone(),
                        },
                    ));
                }
                items
            }
        }
    }
}
pub struct AtDiagnosticCodes;
impl AtDiagnosticCodes {
    pub const INVALID_MODIFIER: &'static str = "at.invalidModifier";
    pub const UNRESOLVED_CLASS: &'static str = "at.unresolvedClass";
    pub const UNRESOLVED_MEMBER: &'static str = "at.unresolvedMember";
    pub const MISSING_METHOD_DESCRIPTOR: &'static str = "at.missingMethodDescriptor";
    pub const INVALID_DESCRIPTOR: &'static str = "at.invalidDescriptor";
    pub const DUPLICATE_ENTRY: &'static str = "at.duplicateEntry";
    pub const WRONG_NAMESPACE: &'static str = "at.wrongNamespace";
    pub const SRG_MAPPING_NOT_FOUND: &'static str = "at.srgMappingNotFound";
    pub const PARSE_ERROR: &'static str = "at.parseError";
}
pub struct AccessTransformerDiagnosticsService<'a> {
    index: &'a dyn ClassIndex,
    mappings: Option<&'a ProjectMappingContext>,
}
impl<'a> AccessTransformerDiagnosticsService<'a> {
    pub fn new(index: &'a dyn ClassIndex, mappings: Option<&'a ProjectMappingContext>) -> Self {
        Self { index, mappings }
    }
    pub fn analyze(&self, text: &str, _uri: &str) -> Vec<McDiagnostic> {
        let entries = match AccessTransformerParser::parse(text) {
            Ok(f) => f.entries,
            Err(f) => {
                let code = if f.message.contains("modifier") {
                    AtDiagnosticCodes::INVALID_MODIFIER
                } else if f.message.contains("descriptor") {
                    AtDiagnosticCodes::INVALID_DESCRIPTOR
                } else {
                    AtDiagnosticCodes::PARSE_ERROR
                };
                return vec![diagnostic(
                    text,
                    f.line,
                    None,
                    code,
                    McSeverity::Error,
                    f.message,
                    &[("line", f.line.to_string())],
                )];
            }
        };
        let mut out = vec![];
        let mut seen = std::collections::BTreeMap::new();
        for e in entries {
            let mut meta = vec![("owner", e.owner.clone()), ("line", e.line.to_string())];
            let issue = if let Some(owner) = find_owner(self.index, &e.owner) {
                if let Some(name) = e.name.as_deref() {
                    meta.push(("member", name.into()));
                    match AtMemberResolver::resolve(
                        &owner.internal_name,
                        name,
                        e.descriptor.as_deref(),
                        self.index,
                        self.mappings,
                    ) {
                        AtMemberResolution::NotFound => Some((
                            AtDiagnosticCodes::UNRESOLVED_MEMBER,
                            format!("Unresolved member '{}' in '{}'", name, e.owner),
                        )),
                        AtMemberResolution::WrongNamespace {
                            actual_name,
                            expected_namespace,
                            ..
                        } => {
                            meta.push((
                                "expectedNamespace",
                                format!("{expected_namespace:?}").to_uppercase(),
                            ));
                            Some((
                                AtDiagnosticCodes::WRONG_NAMESPACE,
                                format!(
                                    "Member '{}' should use {} names",
                                    actual_name,
                                    format!("{expected_namespace:?}").to_lowercase()
                                ),
                            ))
                        }
                        AtMemberResolution::MappingNotFound { named_name, kind } => Some((
                            AtDiagnosticCodes::SRG_MAPPING_NOT_FOUND,
                            format!(
                                "SRG mapping not found for {} '{}'",
                                format!("{kind:?}").to_lowercase(),
                                named_name
                            ),
                        )),
                        AtMemberResolution::DescriptorMismatch {
                            named_name,
                            descriptor,
                            candidates,
                        } => {
                            meta.retain(|(key, _)| *key != "owner");
                            meta.push(("descriptor", descriptor.clone()));
                            meta.push((
                                "expectedDescriptors",
                                candidates
                                    .iter()
                                    .map(|m| m.descriptor.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", "),
                            ));
                            Some((
                                AtDiagnosticCodes::INVALID_DESCRIPTOR,
                                format!(
                                    "Descriptor '{}' does not match any descriptor for '{}'",
                                    descriptor, named_name
                                ),
                            ))
                        }
                        AtMemberResolution::MissingDescriptor { named_name } => Some((
                            AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR,
                            format!("Missing descriptor for method '{}'", named_name),
                        )),
                        AtMemberResolution::Found(r) => {
                            if let Some(method) = r.method {
                                if e.descriptor.is_none() {
                                    meta.push(("descriptor", method.descriptor));
                                    Some((
                                        AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR,
                                        format!("Missing descriptor for method '{}'", r.named_name),
                                    ))
                                } else if e.descriptor.as_deref() != Some(&method.descriptor)
                                    && r.descriptor.as_deref() != Some(&method.descriptor)
                                {
                                    Some((
                                        AtDiagnosticCodes::INVALID_DESCRIPTOR,
                                        format!(
                                            "Descriptor '{}' does not match '{}'",
                                            e.descriptor.as_deref().unwrap_or(""),
                                            method.descriptor
                                        ),
                                    ))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                    }
                } else {
                    None
                }
            } else {
                Some((
                    AtDiagnosticCodes::UNRESOLVED_CLASS,
                    format!("Unresolved class '{}'", e.owner),
                ))
            };
            if let Some((code, message)) = issue {
                out.push(diagnostic(
                    text,
                    e.line,
                    None,
                    code,
                    McSeverity::Error,
                    message,
                    &meta,
                ));
            }
            if let Some(prev) = seen.insert(AccessTransformerEditor::entry_key(&e), e.line) {
                out.push(diagnostic(
                    text,
                    e.line,
                    None,
                    AtDiagnosticCodes::DUPLICATE_ENTRY,
                    McSeverity::Warning,
                    "Duplicate access transformer entry".into(),
                    &[
                        ("line", e.line.to_string()),
                        ("duplicateOfLine", prev.to_string()),
                    ],
                ));
            }
        }
        out
    }
}
fn find_owner(index: &dyn ClassIndex, owner: &str) -> Option<ClassIndexEntry> {
    index
        .find_class_by_fqn(owner)
        .or_else(|| index.find_class(&owner.replace('.', "/")))
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessTransformerEditResult {
    pub content: String,
    pub changed: bool,
}
pub struct AccessTransformerEditor;
impl AccessTransformerEditor {
    pub fn format_entry(e: &AccessTransformerEntry) -> String {
        let mut s = format!("{} {}", e.modifier.token(), e.owner);
        if let Some(n) = &e.name {
            s.push(' ');
            s.push_str(n);
            if let Some(d) = &e.descriptor {
                s.push_str(d);
            }
        }
        s
    }
    pub fn append_entry(text: &str, e: &AccessTransformerEntry) -> AccessTransformerEditResult {
        let nl = newline(text);
        let content = format!(
            "{}{}{}{}",
            text,
            if text.is_empty() || text.ends_with('\n') {
                ""
            } else {
                nl
            },
            Self::format_entry(e),
            nl
        );
        AccessTransformerEditResult {
            content,
            changed: true,
        }
    }
    pub fn replace_line(text: &str, line: usize, new_line: &str) -> AccessTransformerEditResult {
        let content = replace_line(text, line, new_line);
        AccessTransformerEditResult {
            changed: content != text,
            content,
        }
    }
    pub fn remove_line(text: &str, line: usize) -> AccessTransformerEditResult {
        let content = remove_line(text, line);
        AccessTransformerEditResult {
            changed: content != text,
            content,
        }
    }
    pub fn entry_key(e: &AccessTransformerEntry) -> String {
        format!(
            "{}|{}|{}|{}",
            e.modifier.token(),
            e.owner,
            e.name.as_deref().unwrap_or(""),
            e.descriptor.as_deref().unwrap_or("")
        )
    }
}
pub struct AccessTransformerDefinitionService<'a> {
    index: &'a dyn ClassIndex,
    mappings: Option<&'a ProjectMappingContext>,
}
impl<'a> AccessTransformerDefinitionService<'a> {
    pub fn new(index: &'a dyn ClassIndex, mappings: Option<&'a ProjectMappingContext>) -> Self {
        Self { index, mappings }
    }
    pub fn definitions_at(
        &self,
        text: &str,
        line: usize,
        character: usize,
    ) -> Vec<McDefinitionTarget> {
        AtTextPositions::to_offset(text, line, character)
            .map_or_else(Vec::new, |o| self.definitions_at_offset(text, o))
    }
    pub fn definitions_at_offset(&self, text: &str, offset: usize) -> Vec<McDefinitionTarget> {
        let Some(c) = AtContextExtractor::extract_at_offset(text, offset) else {
            return vec![];
        };
        let Some(owner) = c.owner.as_deref().and_then(|o| find_owner(self.index, o)) else {
            return vec![];
        };
        let r = range(text, c.value_start_offset, c.value_end_offset);
        if c.slot == AtSlot::Owner {
            return vec![definition(
                self.index,
                &owner.internal_name,
                MemberKind::Class,
                None,
                None,
                r,
            )];
        }
        if !matches!(c.slot, AtSlot::MemberName | AtSlot::MemberDescriptor) {
            return vec![];
        }
        let Some(name) = c.member_name.as_deref() else {
            return vec![];
        };
        if let AtMemberResolution::Found(found) = AtMemberResolver::resolve(
            &owner.internal_name,
            name,
            c.member_descriptor.as_deref(),
            self.index,
            self.mappings,
        ) {
            return vec![definition(
                self.index,
                &owner.internal_name,
                found.kind,
                Some(found.named_name),
                found.descriptor,
                r,
            )];
        }
        let methods = self
            .index
            .get_methods(&owner.internal_name)
            .into_iter()
            .filter(|m| m.name == name)
            .collect::<Vec<_>>();
        let selected = if let Some(d) = c.member_descriptor.as_deref() {
            methods.iter().find(|m| m.descriptor == d)
        } else if methods.len() == 1 {
            methods.first()
        } else {
            None
        };
        if let Some(m) = selected {
            return vec![definition(
                self.index,
                &owner.internal_name,
                MemberKind::Method,
                Some(m.name.clone()),
                Some(m.descriptor.clone()),
                r,
            )];
        }
        let fields = self
            .index
            .get_fields(&owner.internal_name)
            .into_iter()
            .filter(|f| f.name == name)
            .collect::<Vec<_>>();
        let selected = if let Some(d) = c.member_descriptor.as_deref() {
            fields.iter().find(|f| f.descriptor == d)
        } else if fields.len() == 1 {
            fields.first()
        } else {
            None
        };
        selected.map_or_else(Vec::new, |f| {
            vec![definition(
                self.index,
                &owner.internal_name,
                MemberKind::Field,
                Some(f.name.clone()),
                Some(f.descriptor.clone()),
                r,
            )]
        })
    }
}
pub struct AccessTransformerReferenceService<'a> {
    index: &'a dyn ClassIndex,
}
impl<'a> AccessTransformerReferenceService<'a> {
    pub fn new(index: &'a dyn ClassIndex) -> Self {
        Self { index }
    }
    pub fn find_references(
        &self,
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
    ) -> Vec<McReferenceLocation> {
        let mut out = vec![];
        for source in sources {
            for (i, _) in source.text.split('\n').enumerate() {
                let Some((s, e, _)) = line_bounds(&source.text, i + 1) else {
                    continue;
                };
                let t = tokens(&source.text[s..e], s);
                if t.len() < 2 {
                    continue;
                }
                let owner = t[1].text;
                if owner != target.owner_fqn.as_deref().unwrap_or("")
                    && owner != target.owner_internal_name
                    && owner != target.owner_internal_name.replace('/', ".")
                    && find_owner(self.index, owner)
                        .is_none_or(|c| c.internal_name != target.owner_internal_name)
                {
                    continue;
                }
                let (token, src) = match target.kind {
                    MemberKind::Class if t.len() == 2 => (&t[1], "at.class"),
                    MemberKind::Field
                        if t.len() == 3
                            && !t[2].text.contains('(')
                            && Some(t[2].text) == target.name.as_deref() =>
                    {
                        (&t[2], "at.field")
                    }
                    MemberKind::Method if t.len() == 3 => {
                        let Some(name) = target.name.as_deref() else {
                            continue;
                        };
                        if !t[2].text.starts_with(name)
                            || target
                                .descriptor
                                .as_deref()
                                .is_some_and(|d| t[2].text != format!("{name}{d}"))
                        {
                            continue;
                        }
                        (&t[2], "at.method")
                    }
                    _ => continue,
                };
                out.push(McReferenceLocation {
                    document_uri: source.document_uri.clone(),
                    range: range(&source.text, token.start, token.end),
                    metadata: metadata(&[("source", src.into())]),
                });
            }
        }
        out
    }
}
pub struct AccessTransformerCodeActionService<'a> {
    index: &'a dyn ClassIndex,
    mappings: Option<&'a ProjectMappingContext>,
}
impl<'a> AccessTransformerCodeActionService<'a> {
    pub fn new(index: &'a dyn ClassIndex, mappings: Option<&'a ProjectMappingContext>) -> Self {
        Self { index, mappings }
    }
    pub fn fixes_for_diagnostics(
        &self,
        diagnostics: &[McDiagnostic],
        uri: &str,
        text: &str,
    ) -> Vec<McFix> {
        let mut fixes = vec![];
        for d in diagnostics {
            let Some(line) = d.metadata.get("line").and_then(|s| s.parse::<usize>().ok()) else {
                continue;
            };
            match d.code.as_str() {
                AtDiagnosticCodes::MISSING_METHOD_DESCRIPTOR => {
                    if let (Some(descriptor), Some(name)) =
                        (d.metadata.get("descriptor"), d.metadata.get("member"))
                    {
                        fixes.push(McFix::AddAtMethodDescriptorFix {
                            title: "Add method descriptor".into(),
                            document_uri: uri.into(),
                            line,
                            member_name: name.clone(),
                            descriptor: descriptor.clone(),
                        });
                    }
                }
                AtDiagnosticCodes::WRONG_NAMESPACE | AtDiagnosticCodes::SRG_MAPPING_NOT_FOUND => {
                    if d.code == AtDiagnosticCodes::SRG_MAPPING_NOT_FOUND
                        || self.remap_entry_on_line(text, line).is_some()
                    {
                        fixes.push(McFix::RemapAccessTransformerEntryFix {
                            title: "Remap AT entry namespace".into(),
                            document_uri: uri.into(),
                            line,
                        });
                    }
                }
                AtDiagnosticCodes::DUPLICATE_ENTRY => {
                    fixes.push(McFix::RemoveDuplicateAtEntryFix {
                        title: "Remove duplicate entry".into(),
                        document_uri: uri.into(),
                        line,
                    })
                }
                _ => (),
            }
        }
        fixes
    }
    pub fn generate_entry_fix(context: &AtContext, uri: &str) -> Option<McFix> {
        Some(McFix::AddAccessTransformerEntryFix {
            title: "Generate Access Transformer entry".into(),
            document_uri: uri.into(),
            modifier: context.modifier?.token().into(),
            owner: context.owner.clone()?,
            member_name: context.member_name.clone(),
            member_descriptor: context.member_descriptor.clone(),
            insert_line: context.line_number,
        })
    }
    fn remap_entry_on_line(&self, text: &str, line: usize) -> Option<AccessTransformerEntry> {
        let m = self.mappings.filter(|m| m.at_namespace.is_some())?;
        let file = AccessTransformerParser::parse(text).ok()?;
        let mut e = file.entries.into_iter().find(|e| e.line == line)?;
        let owner = find_owner(self.index, &e.owner)?;
        let Some(name) = e.name.as_deref() else {
            return Some(e);
        };
        let resolution = AtMemberResolver::resolve(
            &owner.internal_name,
            name,
            e.descriptor.as_deref(),
            self.index,
            self.mappings,
        );
        let (method, field) = match resolution {
            AtMemberResolution::Found(r) => (r.method, r.field),
            AtMemberResolution::WrongNamespace { named_name, .. } => {
                let methods = self
                    .index
                    .get_methods(&owner.internal_name)
                    .into_iter()
                    .filter(|t| t.name == named_name)
                    .collect::<Vec<_>>();
                let method = if let Some(d) = &e.descriptor {
                    methods.into_iter().find(|t| &t.descriptor == d)
                } else if methods.len() == 1 {
                    methods.into_iter().next()
                } else {
                    None
                };
                let fields = if method.is_some() {
                    vec![]
                } else {
                    self.index
                        .get_fields(&owner.internal_name)
                        .into_iter()
                        .filter(|t| t.name == named_name)
                        .collect::<Vec<_>>()
                };
                let field = if let Some(d) = &e.descriptor {
                    fields.into_iter().find(|t| &t.descriptor == d)
                } else if fields.len() == 1 {
                    fields.into_iter().next()
                } else {
                    None
                };
                (method, field)
            }
            _ => return None,
        };
        if let Some(method) = method {
            let insert =
                AtMemberInsertFormatter::remap_method_for_entry(&owner.internal_name, &method, m);
            let p = insert.insert_text.find('(');
            e.name = Some(insert.insert_text[..p.unwrap_or(insert.insert_text.len())].into());
            e.descriptor = p.map(|p| insert.insert_text[p..].into());
        } else if let Some(field) = field {
            e.name = Some(
                AtMemberInsertFormatter::remap_field_for_entry(&owner.internal_name, &field, m)
                    .insert_text,
            );
            e.descriptor = None;
        } else {
            return None;
        }
        Some(e)
    }
    pub fn apply_fix(&self, fix: &McFix, text: &str) -> Option<McFix> {
        let (uri, edit) = match fix {
            McFix::AddAccessTransformerEntryFix {
                document_uri,
                modifier,
                owner,
                member_name,
                member_descriptor,
                insert_line,
                ..
            } => {
                let e = AccessTransformerEntry {
                    modifier: AccessTransformerModifier::parse(modifier)?,
                    owner: owner.clone(),
                    name: member_name.clone(),
                    descriptor: member_descriptor.clone(),
                    line: *insert_line,
                };
                let nl = newline(text);
                (
                    document_uri,
                    McTextEdit {
                        start_offset: text.len(),
                        end_offset: text.len(),
                        new_text: format!(
                            "{}{}{}",
                            if text.is_empty() || text.ends_with('\n') {
                                ""
                            } else {
                                nl
                            },
                            AccessTransformerEditor::format_entry(&e),
                            nl
                        ),
                    },
                )
            }
            McFix::AddAtMethodDescriptorFix {
                document_uri,
                line,
                member_name,
                descriptor,
                ..
            } => {
                let l = AtContextExtractor::parse_line(text, *line)?;
                let t = l.member?;
                if t.text.split('(').next() != Some(member_name.as_str()) {
                    return None;
                }
                let new_text = format!("{member_name}{descriptor}");
                if new_text == t.text {
                    return None;
                }
                (
                    document_uri,
                    McTextEdit {
                        start_offset: t.start_offset,
                        end_offset: t.end_offset,
                        new_text,
                    },
                )
            }
            McFix::RemapAccessTransformerEntryFix {
                document_uri, line, ..
            } => {
                let e = self.remap_entry_on_line(text, *line)?;
                let l = AtContextExtractor::parse_line(text, *line)?;
                let t = l.member?;
                let name = e.name?;
                let new_text = format!("{}{}", name, e.descriptor.as_deref().unwrap_or(""));
                if new_text == t.text {
                    return None;
                }
                (
                    document_uri,
                    McTextEdit {
                        start_offset: t.start_offset,
                        end_offset: t.end_offset,
                        new_text,
                    },
                )
            }
            McFix::RemoveDuplicateAtEntryFix {
                document_uri, line, ..
            } => {
                let (s, _, end) = line_bounds(text, *line)?;
                (
                    document_uri,
                    McTextEdit {
                        start_offset: s,
                        end_offset: end,
                        new_text: String::new(),
                    },
                )
            }
            _ => return None,
        };
        Some(workspace_edit(fix, uri, vec![edit]))
    }
}
