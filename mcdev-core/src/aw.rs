use serde::{Deserialize, Serialize};
// Access widener v2 domain services, independent of the LSP transport.
use crate::{
    awat::*,
    descriptor::{parse_field_descriptor, parse_method_descriptor},
    mapping::{
        parse_namespace, ClassRef, FieldRef, MappingLookupResult, MappingResolver, MethodRef,
        ProjectMappingContext,
    },
    model::*,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessWidenerDirective {
    Accessible,
    Extendable,
    Mutable,
    Natural,
}
impl AccessWidenerDirective {
    pub fn token(self) -> &'static str {
        match self {
            Self::Accessible => "accessible",
            Self::Extendable => "extendable",
            Self::Mutable => "mutable",
            Self::Natural => "natural",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "accessible" => Some(Self::Accessible),
            "extendable" => Some(Self::Extendable),
            "mutable" => Some(Self::Mutable),
            "natural" => Some(Self::Natural),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AccessWidenerKind {
    Class,
    Method,
    Field,
}
impl AccessWidenerKind {
    pub fn token(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Method => "method",
            Self::Field => "field",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "class" => Some(Self::Class),
            "method" => Some(Self::Method),
            "field" => Some(Self::Field),
            _ => None,
        }
    }
    fn member_kind(self) -> MemberKind {
        match self {
            Self::Class => MemberKind::Class,
            Self::Method => MemberKind::Method,
            Self::Field => MemberKind::Field,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessWidenerFile {
    pub namespace: MappingNamespace,
    pub entries: Vec<AccessWidenerEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessWidenerEntry {
    pub directive: AccessWidenerDirective,
    pub kind: AccessWidenerKind,
    pub owner: String,
    pub name: Option<String>,
    pub descriptor: Option<String>,
    pub line: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessWidenerParseFailure {
    pub line: usize,
    pub message: String,
}
pub type AccessWidenerParseResult = Result<AccessWidenerFile, AccessWidenerParseFailure>;
pub struct AccessWidenerParser;
impl AccessWidenerParser {
    pub fn parse(text: &str) -> AccessWidenerParseResult {
        let fail = |line, message: &str| AccessWidenerParseFailure {
            line,
            message: message.into(),
        };
        let mut lines = text.split('\n');
        let header = lines
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>();
        if header.len() != 3 || header[0] != "accessWidener" || header[1] != "v2" {
            return Err(fail(1, "expected accessWidener v2 header"));
        }
        let namespace = parse_namespace(header[2]).ok_or_else(|| {
            fail(
                1,
                &format!("unknown access widener namespace '{}'", header[2]),
            )
        })?;
        let mut entries = vec![];
        for (i, raw) in lines.enumerate() {
            let line = i + 2;
            let p = raw
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect::<Vec<_>>();
            if p.is_empty() {
                continue;
            }
            let directive = AccessWidenerDirective::parse(p[0])
                .ok_or_else(|| fail(line, "invalid access widener directive"))?;
            let kind = p
                .get(1)
                .and_then(|s| AccessWidenerKind::parse(s))
                .ok_or_else(|| fail(line, "invalid access widener kind"))?;
            let expected = if kind == AccessWidenerKind::Class {
                3
            } else {
                5
            };
            if p.len() != expected {
                return Err(fail(
                    line,
                    &format!(
                        "{} entry must contain {}",
                        kind.token(),
                        if expected == 3 {
                            "owner"
                        } else {
                            "owner, name, and descriptor"
                        }
                    ),
                ));
            }
            if kind == AccessWidenerKind::Method && parse_method_descriptor(p[4]).is_err() {
                return Err(fail(line, "invalid method descriptor"));
            }
            if kind == AccessWidenerKind::Field && parse_field_descriptor(p[4]).is_err() {
                return Err(fail(line, "invalid field descriptor"));
            }
            entries.push(AccessWidenerEntry {
                directive,
                kind,
                owner: p[2].into(),
                name: p.get(3).map(|s| s.to_string()),
                descriptor: p.get(4).map(|s| s.to_string()),
                line,
            });
        }
        Ok(AccessWidenerFile { namespace, entries })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwSyntaxSlot {
    HeaderNamespace,
    Directive,
    Kind,
    Owner,
    Name,
    Descriptor,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwAnnotationContext {
    pub slot: AwSyntaxSlot,
    pub partial_value: String,
    pub value_start_offset: usize,
    pub value_end_offset: usize,
    pub line_start_offset: usize,
    pub line_end_offset: usize,
    pub line_number: usize,
    pub file_namespace: Option<MappingNamespace>,
    pub directive: Option<AccessWidenerDirective>,
    pub kind: Option<AccessWidenerKind>,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub descriptor: Option<String>,
    pub is_header_line: bool,
}
pub struct AwContextExtractor;
impl AwContextExtractor {
    pub fn to_offset(text: &str, line: usize, character: usize) -> Option<usize> {
        clamped_position_to_offset(text, line, character)
    }
    pub fn offset_to_position(text: &str, offset: usize) -> Option<McTextPosition> {
        offset_to_position(text, offset)
    }
    pub fn parse_file_namespace(text: &str) -> Option<MappingNamespace> {
        let h = text
            .split('\n')
            .next()?
            .split('#')
            .next()?
            .split_whitespace()
            .collect::<Vec<_>>();
        if h.len() < 3 || h[0] != "accessWidener" || h[1] != "v2" {
            None
        } else {
            parse_namespace(h[2])
        }
    }
    pub fn extract(text: &str, line: usize, character: usize) -> Option<AwAnnotationContext> {
        Self::extract_at_offset(text, Self::to_offset(text, line, character)?)
    }
    pub fn extract_at_offset(text: &str, offset: usize) -> Option<AwAnnotationContext> {
        if !text.is_char_boundary(offset) {
            return None;
        }
        let start = text[..offset].rfind('\n').map_or(0, |n| n + 1);
        let end = text[offset..].find('\n').map_or(text.len(), |n| offset + n);
        let line = text[..start].bytes().filter(|b| *b == b'\n').count() + 1;
        let t = tokens(&text[start..end], start);
        let namespace = Self::parse_file_namespace(text);
        let mut c = AwAnnotationContext {
            slot: AwSyntaxSlot::Directive,
            partial_value: String::new(),
            value_start_offset: offset,
            value_end_offset: offset,
            line_start_offset: start,
            line_end_offset: end,
            line_number: line,
            file_namespace: namespace,
            directive: None,
            kind: None,
            owner: None,
            name: None,
            descriptor: None,
            is_header_line: line == 1,
        };
        if line == 1 {
            let n = t.get(2)?;
            if offset < n.start || offset > n.end {
                return None;
            }
            c.slot = AwSyntaxSlot::HeaderNamespace;
            c.partial_value = text[n.start..offset].into();
            c.value_start_offset = n.start;
            c.value_end_offset = n.end;
            return Some(c);
        }
        if t.is_empty() {
            return Some(c);
        }
        let after = t.last().is_some_and(|last| {
            offset > last.end && text[last.end..offset].chars().any(char::is_whitespace)
        });
        let active = if after {
            t.len()
        } else {
            t.iter()
                .position(|t| offset >= t.start && offset <= t.end)
                .or_else(|| t.iter().rposition(|t| t.start <= offset))
                .unwrap_or(t.len() - 1)
        };
        c.directive = t
            .first()
            .and_then(|t| AccessWidenerDirective::parse(t.text));
        c.kind = t.get(1).and_then(|t| AccessWidenerKind::parse(t.text));
        c.owner = t.get(2).map(|t| t.text.into());
        c.name = if matches!(
            c.kind,
            Some(AccessWidenerKind::Field | AccessWidenerKind::Method)
        ) {
            t.get(3).map(|t| t.text.into())
        } else {
            None
        };
        c.descriptor = t.get(4).map(|t| t.text.into());
        c.slot = match active {
            0 => AwSyntaxSlot::Directive,
            1 => AwSyntaxSlot::Kind,
            2 => AwSyntaxSlot::Owner,
            3 => {
                if c.kind == Some(AccessWidenerKind::Class) || c.kind.is_none() {
                    AwSyntaxSlot::Owner
                } else {
                    AwSyntaxSlot::Name
                }
            }
            _ => AwSyntaxSlot::Descriptor,
        };
        if let Some(token) = t.get(active) {
            c.value_start_offset = token.start;
            c.value_end_offset = token.end;
            c.partial_value = text[token.start..offset.clamp(token.start, token.end)].into();
        }
        Some(c)
    }
}
pub struct AwNamespaceHelper;
impl AwNamespaceHelper {
    pub fn resolve_owner_in_index(
        owner: &str,
        namespace: Option<MappingNamespace>,
        index: &dyn ClassIndex,
        mappings: Option<&ProjectMappingContext>,
    ) -> Option<String> {
        if index.find_class(owner).is_some() {
            return Some(owner.into());
        }
        let resolver = &mappings?.resolver;
        let mut namespaces = vec![];
        if let Some(n) = namespace {
            namespaces.push(n);
        }
        namespaces.push(MappingNamespace::Named);
        namespaces.extend(resolver.namespaces());
        for n in namespaces {
            if let MappingLookupResult::Found(c) = resolver.remap_class(
                &ClassRef {
                    internal_name: owner.into(),
                    namespace: n,
                },
                MappingNamespace::Named,
            ) {
                if index.find_class(&c.internal_name).is_some() {
                    return Some(c.internal_name);
                }
            }
        }
        None
    }
    pub fn has_namespace_mismatch(
        owner: &str,
        namespace: Option<MappingNamespace>,
        index: &dyn ClassIndex,
        mappings: Option<&ProjectMappingContext>,
    ) -> bool {
        let Some(namespace) = namespace else {
            return false;
        };
        if index.find_class(owner).is_some() {
            return false;
        }
        let Some(m) = mappings else { return false };
        m.resolver.namespaces().into_iter().filter(|n|*n!=namespace).any(|n|matches!(m.resolver.remap_class(&ClassRef{internal_name:owner.into(),namespace:n},MappingNamespace::Named),MappingLookupResult::Found(c) if index.find_class(&c.internal_name).is_some()))
    }
    pub fn insert_owner(
        owner: &str,
        target: Option<MappingNamespace>,
        resolver: Option<&dyn MappingResolver>,
    ) -> String {
        if let (Some(n), Some(r)) = (target, resolver) {
            if n != MappingNamespace::Named {
                if let MappingLookupResult::Found(c) = r.remap_class(
                    &ClassRef {
                        internal_name: owner.into(),
                        namespace: MappingNamespace::Named,
                    },
                    n,
                ) {
                    return c.internal_name;
                }
            }
        }
        owner.into()
    }
    pub fn insert_member_name(
        owner: &str,
        name: &str,
        descriptor: &str,
        kind: AccessWidenerKind,
        target: Option<MappingNamespace>,
        resolver: Option<&dyn MappingResolver>,
    ) -> String {
        if let (Some(n), Some(r)) = (target, resolver) {
            if n != MappingNamespace::Named {
                match kind {
                    AccessWidenerKind::Method => {
                        if let MappingLookupResult::Found(m) = r.remap_method(
                            &MethodRef {
                                owner: owner.into(),
                                name: name.into(),
                                descriptor: descriptor.into(),
                                namespace: MappingNamespace::Named,
                            },
                            n,
                        ) {
                            return m.name;
                        }
                    }
                    AccessWidenerKind::Field => {
                        if let MappingLookupResult::Found(f) = r.remap_field(
                            &FieldRef {
                                owner: owner.into(),
                                name: name.into(),
                                descriptor: descriptor.into(),
                                namespace: MappingNamespace::Named,
                            },
                            n,
                        ) {
                            return f.name;
                        }
                    }
                    _ => (),
                }
            }
        }
        name.into()
    }
    pub fn insert_descriptor(
        descriptor: &str,
        target: Option<MappingNamespace>,
        resolver: Option<&dyn MappingResolver>,
    ) -> String {
        if let (Some(n), Some(r)) = (target, resolver) {
            if n != MappingNamespace::Named {
                if let MappingLookupResult::Found(d) =
                    r.remap_descriptor(descriptor, MappingNamespace::Named, n)
                {
                    return d;
                }
            }
        }
        descriptor.into()
    }
}
pub struct AwDiagnosticCodes;
impl AwDiagnosticCodes {
    pub const INVALID_DIRECTIVE: &'static str = "AW_INVALID_DIRECTIVE";
    pub const INVALID_KIND: &'static str = "AW_INVALID_KIND";
    pub const UNRESOLVED_CLASS: &'static str = "AW_UNRESOLVED_CLASS";
    pub const UNRESOLVED_MEMBER: &'static str = "AW_UNRESOLVED_MEMBER";
    pub const INVALID_DESCRIPTOR: &'static str = "AW_INVALID_DESCRIPTOR";
    pub const DUPLICATE_ENTRY: &'static str = "AW_DUPLICATE_ENTRY";
    pub const MUTABLE_ON_NON_FIELD: &'static str = "AW_MUTABLE_ON_NON_FIELD";
    pub const EXTENDABLE_INVALID_TARGET: &'static str = "AW_EXTENDABLE_INVALID_TARGET";
    pub const NAMESPACE_MISMATCH: &'static str = "AW_NAMESPACE_MISMATCH";
}
pub struct AccessWidenerDiagnosticsService<'a> {
    index: &'a dyn ClassIndex,
}
impl<'a> AccessWidenerDiagnosticsService<'a> {
    pub fn new(index: &'a dyn ClassIndex) -> Self {
        Self { index }
    }
    pub fn analyze(
        &self,
        text: &str,
        _uri: &str,
        mappings: Option<&ProjectMappingContext>,
    ) -> Vec<McDiagnostic> {
        let mut out = vec![];
        match AccessWidenerParser::parse(text) {
            Err(f) => {
                let code = if f.message.contains("kind") {
                    AwDiagnosticCodes::INVALID_KIND
                } else if f.message.contains("descriptor") {
                    AwDiagnosticCodes::INVALID_DESCRIPTOR
                } else {
                    AwDiagnosticCodes::INVALID_DIRECTIVE
                };
                out.push(diagnostic(
                    text,
                    f.line,
                    None,
                    code,
                    McSeverity::Error,
                    f.message,
                    &[("line", f.line.to_string())],
                ));
            }
            Ok(file) => {
                for entry in &file.entries {
                    let line = entry.line;
                    if entry.directive == AccessWidenerDirective::Mutable
                        && entry.kind != AccessWidenerKind::Field
                    {
                        out.push(diagnostic(
                            text,
                            line,
                            None,
                            AwDiagnosticCodes::MUTABLE_ON_NON_FIELD,
                            McSeverity::Error,
                            "mutable directive can only be applied to fields".into(),
                            &[],
                        ));
                    }
                    if entry.directive == AccessWidenerDirective::Extendable
                        && entry.kind != AccessWidenerKind::Class
                    {
                        out.push(diagnostic(
                            text,
                            line,
                            None,
                            AwDiagnosticCodes::EXTENDABLE_INVALID_TARGET,
                            McSeverity::Error,
                            "extendable directive can only be applied to classes".into(),
                            &[],
                        ));
                    }
                    let meta = [("owner", entry.owner.clone()), ("line", line.to_string())];
                    if AwNamespaceHelper::has_namespace_mismatch(
                        &entry.owner,
                        Some(file.namespace),
                        self.index,
                        mappings,
                    ) {
                        out.push(diagnostic(
                            text,
                            line,
                            Some(2),
                            AwDiagnosticCodes::NAMESPACE_MISMATCH,
                            McSeverity::Error,
                            format!(
                                "Owner '{}' does not match access widener namespace '{}'",
                                entry.owner,
                                format!("{:?}", file.namespace).to_lowercase()
                            ),
                            &meta,
                        ));
                    }
                    let Some(owner) = AwNamespaceHelper::resolve_owner_in_index(
                        &entry.owner,
                        Some(file.namespace),
                        self.index,
                        mappings,
                    ) else {
                        out.push(diagnostic(
                            text,
                            line,
                            Some(2),
                            AwDiagnosticCodes::UNRESOLVED_CLASS,
                            McSeverity::Error,
                            format!("Unresolved class: {}", entry.owner),
                            &meta,
                        ));
                        continue;
                    };
                    if entry.kind == AccessWidenerKind::Class {
                        continue;
                    }
                    let name = entry.name.as_deref().unwrap_or("");
                    let desc = entry.descriptor.as_deref().unwrap_or("");
                    let candidates = match entry.kind {
                        AccessWidenerKind::Method => self
                            .index
                            .get_methods(&owner)
                            .into_iter()
                            .filter(|m| m.name == name)
                            .map(|m| m.descriptor)
                            .collect::<Vec<_>>(),
                        _ => self
                            .index
                            .get_fields(&owner)
                            .into_iter()
                            .filter(|f| f.name == name)
                            .map(|f| f.descriptor)
                            .collect(),
                    };
                    if candidates.is_empty() {
                        out.push(diagnostic(
                            text,
                            line,
                            Some(3),
                            AwDiagnosticCodes::UNRESOLVED_MEMBER,
                            McSeverity::Error,
                            format!("Unresolved {}: {}", entry.kind.token(), name),
                            &[("name", name.into()), ("line", line.to_string())],
                        ));
                    } else if !desc.is_empty() && !candidates.iter().any(|d| d == desc) {
                        out.push(diagnostic(
                            text,
                            line,
                            Some(4),
                            AwDiagnosticCodes::INVALID_DESCRIPTOR,
                            McSeverity::Error,
                            format!(
                                "{} descriptor mismatch for '{}'",
                                if entry.kind == AccessWidenerKind::Method {
                                    "Method"
                                } else {
                                    "Field"
                                },
                                name
                            ),
                            &[
                                ("name", name.into()),
                                ("descriptor", desc.into()),
                                ("line", line.to_string()),
                            ],
                        ));
                    }
                }
                for entry in &file.entries {
                    let key = AccessWidenerEditor::entry_key(entry);
                    if file
                        .entries
                        .iter()
                        .filter(|e| AccessWidenerEditor::entry_key(e) == key)
                        .count()
                        > 1
                    {
                        out.push(diagnostic(
                            text,
                            entry.line,
                            None,
                            AwDiagnosticCodes::DUPLICATE_ENTRY,
                            McSeverity::Warning,
                            "Duplicate access widener entry".into(),
                            &[("line", entry.line.to_string()), ("key", key)],
                        ));
                    }
                }
            }
        }
        for (i, raw) in text.split('\n').enumerate().skip(1) {
            let t = tokens(raw, 0);
            if let Some(d) = t.first() {
                if AccessWidenerDirective::parse(d.text).is_none() {
                    out.push(diagnostic(
                        text,
                        i + 1,
                        Some(0),
                        AwDiagnosticCodes::INVALID_DIRECTIVE,
                        McSeverity::Error,
                        format!("Invalid access widener directive: {}", d.text),
                        &[("directive", d.text.into()), ("line", (i + 1).to_string())],
                    ));
                }
            }
            if let Some(k) = t.get(1) {
                if AccessWidenerKind::parse(k.text).is_none() {
                    out.push(diagnostic(
                        text,
                        i + 1,
                        Some(1),
                        AwDiagnosticCodes::INVALID_KIND,
                        McSeverity::Error,
                        format!("Invalid access widener kind: {}", k.text),
                        &[("kind", k.text.into()), ("line", (i + 1).to_string())],
                    ));
                }
            }
        }
        out
    }
}
#[derive(Debug, Clone, Copy)]
pub struct AccessWidenerCompletionOptions {
    pub include_descriptors_for_members: bool,
}
impl Default for AccessWidenerCompletionOptions {
    fn default() -> Self {
        Self {
            include_descriptors_for_members: true,
        }
    }
}
pub struct AccessWidenerCompletionService<'a> {
    index: &'a dyn ClassIndex,
}
impl<'a> AccessWidenerCompletionService<'a> {
    pub fn new(index: &'a dyn ClassIndex) -> Self {
        Self { index }
    }
    pub fn complete(
        &self,
        c: &AwAnnotationContext,
        mappings: Option<&ProjectMappingContext>,
        _options: AccessWidenerCompletionOptions,
    ) -> Vec<McCompletionItem> {
        let p = &c.partial_value;
        let target = c.file_namespace.or(mappings.and_then(|m| m.aw_namespace));
        let r = mappings.map(|m| m.resolver.as_ref());
        let keyword = |s: &str, detail: &str, source: &str, sort: &str, kind| {
            completion(
                s.into(),
                Some(detail.into()),
                None,
                s.into(),
                s.into(),
                kind,
                format!("{sort}_{s}"),
                McCompletionMetadata {
                    source: source.into(),
                    name: if source == "aw.namespace" {
                        None
                    } else {
                        Some(s.into())
                    },
                    namespace: if source == "aw.namespace" {
                        Some(s.to_uppercase())
                    } else {
                        None
                    },
                    ..Default::default()
                },
            )
        };
        match c.slot {
            AwSyntaxSlot::HeaderNamespace => ["named", "intermediary", "official"]
                .into_iter()
                .filter(|s| s.starts_with(&p.to_lowercase()))
                .map(|s| {
                    keyword(
                        s,
                        "Access widener namespace",
                        "aw.namespace",
                        "0000",
                        McCompletionKind::Value,
                    )
                })
                .collect(),
            AwSyntaxSlot::Directive => ["accessible", "extendable", "mutable", "natural"]
                .into_iter()
                .filter(|s| s.starts_with(p))
                .map(|s| {
                    keyword(
                        s,
                        "Access widener directive",
                        "aw.directive",
                        "0100",
                        McCompletionKind::Keyword,
                    )
                })
                .collect(),
            AwSyntaxSlot::Kind => ["class", "method", "field"]
                .into_iter()
                .filter(|s| {
                    s.starts_with(p)
                        && match c.directive {
                            Some(AccessWidenerDirective::Mutable) => *s == "field",
                            Some(AccessWidenerDirective::Extendable) => *s == "class",
                            _ => true,
                        }
                })
                .map(|s| {
                    keyword(
                        s,
                        "Access widener kind",
                        "aw.kind",
                        "0200",
                        McCompletionKind::Keyword,
                    )
                })
                .collect(),
            AwSyntaxSlot::Owner => self
                .index
                .find_classes(p.rsplit('/').next().unwrap_or(p), CLASS_COMPLETION_LIMIT)
                .into_iter()
                .map(|e| {
                    completion(
                        e.simple_name.clone(),
                        Some(e.internal_name.clone()),
                        Some(e.fqn()),
                        format!("{} {} {}", e.simple_name, e.fqn(), e.internal_name),
                        AwNamespaceHelper::insert_owner(&e.internal_name, target, r),
                        McCompletionKind::Class,
                        format!("0300_{}", e.simple_name),
                        McCompletionMetadata {
                            source: "aw.class".into(),
                            owner: Some(e.internal_name),
                            name: Some(e.simple_name),
                            namespace: target.map(|n| format!("{n:?}").to_uppercase()),
                            descriptor: None,
                        },
                    )
                })
                .collect(),
            AwSyntaxSlot::Name | AwSyntaxSlot::Descriptor => {
                let Some(owner) = c.owner.as_deref().and_then(|o| {
                    AwNamespaceHelper::resolve_owner_in_index(
                        o,
                        c.file_namespace,
                        self.index,
                        mappings,
                    )
                }) else {
                    return vec![];
                };
                let Some(kind) = c.kind else { return vec![] };
                let candidates = match kind {
                    AccessWidenerKind::Method => self
                        .index
                        .get_methods(&owner)
                        .into_iter()
                        .map(|m| {
                            (
                                m.name,
                                m.descriptor,
                                m.readable_signature,
                                McCompletionKind::Value,
                            )
                        })
                        .collect::<Vec<_>>(),
                    AccessWidenerKind::Field => self
                        .index
                        .get_fields(&owner)
                        .into_iter()
                        .map(|f| {
                            let label = format!("{}: {}", f.name, f.readable_type);
                            (f.name, f.descriptor, label, McCompletionKind::Field)
                        })
                        .collect(),
                    _ => vec![],
                };
                candidates
                    .into_iter()
                    .filter(|(name, desc, _, _)| {
                        if c.slot == AwSyntaxSlot::Name {
                            name.starts_with(p)
                        } else {
                            Some(name) == c.name.as_ref() && desc.starts_with(p)
                        }
                    })
                    .map(|(name, desc, label, ck)| {
                        let is_desc = c.slot == AwSyntaxSlot::Descriptor;
                        completion(
                            if is_desc { desc.clone() } else { label.clone() },
                            Some(if is_desc {
                                if kind == AccessWidenerKind::Field {
                                    label.split(": ").nth(1).unwrap_or(&label).into()
                                } else {
                                    label.clone()
                                }
                            } else {
                                owner.clone()
                            }),
                            if is_desc { None } else { Some(desc.clone()) },
                            if is_desc {
                                desc.clone()
                            } else {
                                format!("{name} {label}")
                            },
                            if is_desc {
                                AwNamespaceHelper::insert_descriptor(&desc, target, r)
                            } else {
                                AwNamespaceHelper::insert_member_name(
                                    &owner, &name, &desc, kind, target, r,
                                )
                            },
                            ck,
                            if is_desc {
                                format!("0500_{desc}")
                            } else {
                                format!("0400_{name}")
                            },
                            McCompletionMetadata {
                                source: if is_desc {
                                    "aw.descriptor"
                                } else if kind == AccessWidenerKind::Method {
                                    "aw.method"
                                } else {
                                    "aw.field"
                                }
                                .into(),
                                owner: Some(owner.clone()),
                                name: Some(name),
                                descriptor: Some(desc),
                                namespace: if is_desc {
                                    None
                                } else {
                                    target.map(|n| format!("{n:?}").to_uppercase())
                                },
                            },
                        )
                    })
                    .collect()
            }
        }
    }
}
pub struct AccessWidenerEditor;
impl AccessWidenerEditor {
    pub fn format_class_entry(directive: AccessWidenerDirective, owner: &str) -> String {
        format!("{} class {}", directive.token(), owner)
    }
    pub fn format_method_entry(
        directive: AccessWidenerDirective,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> String {
        format!(
            "{} method {} {} {}",
            directive.token(),
            owner,
            name,
            descriptor
        )
    }
    pub fn format_field_entry(
        directive: AccessWidenerDirective,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> String {
        format!(
            "{} field {} {} {}",
            directive.token(),
            owner,
            name,
            descriptor
        )
    }
    pub fn format_entry(e: &AccessWidenerEntry) -> String {
        if e.kind == AccessWidenerKind::Class {
            format!("{} class {}", e.directive.token(), e.owner)
        } else {
            format!(
                "{} {} {} {} {}",
                e.directive.token(),
                e.kind.token(),
                e.owner,
                e.name.as_deref().unwrap_or(""),
                e.descriptor.as_deref().unwrap_or("")
            )
        }
    }
    pub fn entry_key(e: &AccessWidenerEntry) -> String {
        let mut key = format!(
            "{}|{}|{}",
            e.directive.token().to_uppercase(),
            e.kind.token().to_uppercase(),
            e.owner
        );
        if e.kind != AccessWidenerKind::Class {
            key.push_str(&format!(
                "|{}|{}",
                e.name.as_deref().unwrap_or(""),
                e.descriptor.as_deref().unwrap_or("")
            ));
        }
        key
    }
    pub fn insert_entry(
        text: &str,
        entry: &AccessWidenerEntry,
        after_line: Option<usize>,
    ) -> String {
        let line = Self::format_entry(entry);
        let nl = newline(text);
        let at = after_line
            .and_then(|n| line_bounds(text, n.max(1)))
            .map_or(text.len(), |(_, _, end)| end);
        let prefix = if at > 0 && !text[..at].ends_with('\n') {
            nl
        } else {
            ""
        };
        format!("{}{}{}{}{}", &text[..at], prefix, line, nl, &text[at..])
    }
    pub fn replace_line(text: &str, line: usize, new_line: &str) -> String {
        replace_line(text, line, new_line)
    }
    pub fn remove_line(text: &str, line: usize) -> String {
        remove_line(text, line)
    }
}
pub struct AccessWidenerDefinitionService<'a> {
    index: &'a dyn ClassIndex,
    mappings: Option<&'a ProjectMappingContext>,
}
impl<'a> AccessWidenerDefinitionService<'a> {
    pub fn new(index: &'a dyn ClassIndex, mappings: Option<&'a ProjectMappingContext>) -> Self {
        Self { index, mappings }
    }
    pub fn definitions_at(
        &self,
        text: &str,
        line: usize,
        character: usize,
    ) -> Vec<McDefinitionTarget> {
        AwContextExtractor::to_offset(text, line, character)
            .map_or_else(Vec::new, |o| self.definitions_at_offset(text, o))
    }
    pub fn definitions_at_offset(&self, text: &str, offset: usize) -> Vec<McDefinitionTarget> {
        let Some(c) = AwContextExtractor::extract_at_offset(text, offset) else {
            return vec![];
        };
        if c.is_header_line {
            return vec![];
        }
        let Some(owner) = c.owner.as_deref().and_then(|o| {
            AwNamespaceHelper::resolve_owner_in_index(
                o,
                c.file_namespace,
                self.index,
                self.mappings,
            )
        }) else {
            return vec![];
        };
        let range = range(text, c.value_start_offset, c.value_end_offset);
        if c.slot == AwSyntaxSlot::Owner {
            return vec![definition(
                self.index,
                &owner,
                MemberKind::Class,
                None,
                None,
                range,
            )];
        }
        if !matches!(c.slot, AwSyntaxSlot::Name | AwSyntaxSlot::Descriptor) {
            return vec![];
        }
        let Some(kind) = c.kind else { return vec![] };
        let Some(name) = c.name else { return vec![] };
        let descriptor = c.descriptor.filter(|d| !d.is_empty());
        if c.slot == AwSyntaxSlot::Descriptor && descriptor.is_none() {
            return vec![];
        }
        let candidates = match kind {
            AccessWidenerKind::Method => self
                .index
                .get_methods(&owner)
                .into_iter()
                .filter(|m| m.name == name)
                .map(|m| m.descriptor)
                .collect::<Vec<_>>(),
            AccessWidenerKind::Field => self
                .index
                .get_fields(&owner)
                .into_iter()
                .filter(|f| f.name == name)
                .map(|f| f.descriptor)
                .collect(),
            _ => return vec![],
        };
        let selected = if let Some(desc) = descriptor {
            candidates.into_iter().find(|d| *d == desc)
        } else if candidates.len() == 1 {
            candidates.into_iter().next()
        } else {
            None
        };
        selected.map_or_else(Vec::new, |d| {
            vec![definition(
                self.index,
                &owner,
                kind.member_kind(),
                Some(name),
                Some(d),
                range,
            )]
        })
    }
}
pub struct AccessWidenerReferenceService<'a> {
    index: &'a dyn ClassIndex,
    mappings: Option<&'a ProjectMappingContext>,
}
impl<'a> AccessWidenerReferenceService<'a> {
    pub fn new(index: &'a dyn ClassIndex, mappings: Option<&'a ProjectMappingContext>) -> Self {
        Self { index, mappings }
    }
    pub fn find_references(
        &self,
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
    ) -> Vec<McReferenceLocation> {
        let mut out = vec![];
        for source in sources {
            let ns = AwContextExtractor::parse_file_namespace(&source.text);
            for (i, _) in source.text.split('\n').enumerate().skip(1) {
                let Some((s, e, _)) = line_bounds(&source.text, i + 1) else {
                    continue;
                };
                let t = tokens(&source.text[s..e], s);
                if t.len() < 3 {
                    continue;
                }
                let owner = t[2].text;
                if owner != target.owner_internal_name
                    && AwNamespaceHelper::resolve_owner_in_index(
                        owner,
                        ns,
                        self.index,
                        self.mappings,
                    )
                    .as_deref()
                        != Some(&target.owner_internal_name)
                {
                    continue;
                }
                let (start, end, src) = match target.kind {
                    MemberKind::Class => (
                        t[2].start,
                        t[2].end,
                        if t[1].text == "class" {
                            "aw.class"
                        } else {
                            "aw.owner"
                        },
                    ),
                    MemberKind::Method | MemberKind::Field => {
                        let kind = if target.kind == MemberKind::Method {
                            "method"
                        } else {
                            "field"
                        };
                        if t.len() < 5
                            || t[1].text != kind
                            || AccessWidenerDirective::parse(t[0].text).is_none()
                            || Some(t[3].text) != target.name.as_deref()
                            || target.descriptor.as_deref().is_some_and(|d| d != t[4].text)
                        {
                            continue;
                        }
                        (
                            s,
                            t[4].end,
                            if target.kind == MemberKind::Method {
                                "aw.method"
                            } else {
                                "aw.field"
                            },
                        )
                    }
                    _ => continue,
                };
                out.push(McReferenceLocation {
                    document_uri: source.document_uri.clone(),
                    range: range(&source.text, start, end),
                    metadata: metadata(&[("source", src.into())]),
                });
            }
        }
        out
    }
}
pub struct AccessWidenerCodeActionService;
impl AccessWidenerCodeActionService {
    pub fn fixes_for_diagnostics(
        diagnostics: &[McDiagnostic],
        uri: &str,
        text: &str,
        index: Option<&dyn ClassIndex>,
        mappings: Option<&ProjectMappingContext>,
    ) -> Vec<McFix> {
        let mut fixes = vec![];
        for d in diagnostics {
            let Some(line) = d.metadata.get("line").and_then(|s| s.parse::<usize>().ok()) else {
                continue;
            };
            if d.code == AwDiagnosticCodes::DUPLICATE_ENTRY {
                fixes.push(McFix::RemoveDuplicateAccessWidenerEntryFix {
                    title: "Remove duplicate access widener entry".into(),
                    document_uri: uri.into(),
                    line_number: line,
                });
                continue;
            }
            let Some(index) = index else { continue };
            let Ok(file) = AccessWidenerParser::parse(text) else {
                continue;
            };
            let Some(e) = file.entries.iter().find(|e| e.line == line) else {
                continue;
            };
            if d.code == AwDiagnosticCodes::NAMESPACE_MISMATCH {
                let Some(m) = mappings else { continue };
                let Some(owner) = AwNamespaceHelper::resolve_owner_in_index(
                    &e.owner,
                    Some(file.namespace),
                    index,
                    Some(m),
                ) else {
                    continue;
                };
                let resolver = Some(m.resolver.as_ref());
                let mut remapped = e.clone();
                remapped.owner =
                    AwNamespaceHelper::insert_owner(&owner, Some(file.namespace), resolver);
                remapped.name = e.name.as_ref().map(|n| {
                    AwNamespaceHelper::insert_member_name(
                        &owner,
                        n,
                        e.descriptor.as_deref().unwrap_or(""),
                        e.kind,
                        Some(file.namespace),
                        resolver,
                    )
                });
                remapped.descriptor = e.descriptor.as_ref().map(|d| {
                    AwNamespaceHelper::insert_descriptor(d, Some(file.namespace), resolver)
                });
                fixes.push(McFix::RemapAccessWidenerEntryFix {
                    title: format!(
                        "Remap access widener entry to {} namespace",
                        format!("{:?}", file.namespace).to_lowercase()
                    ),
                    document_uri: uri.into(),
                    line_number: line,
                    new_line: AccessWidenerEditor::format_entry(&remapped),
                });
            } else if d.code == AwDiagnosticCodes::INVALID_DESCRIPTOR
                || d.code == AwDiagnosticCodes::UNRESOLVED_MEMBER
            {
                let Some(owner) = AwNamespaceHelper::resolve_owner_in_index(
                    &e.owner,
                    Some(file.namespace),
                    index,
                    None,
                ) else {
                    continue;
                };
                let Some(name) = e.name.as_deref() else {
                    continue;
                };
                let descriptor = match e.kind {
                    AccessWidenerKind::Method => index
                        .get_methods(&owner)
                        .into_iter()
                        .find(|m| m.name == name)
                        .map(|m| m.descriptor),
                    AccessWidenerKind::Field => index
                        .get_fields(&owner)
                        .into_iter()
                        .find(|f| f.name == name)
                        .map(|f| f.descriptor),
                    _ => None,
                };
                let Some(descriptor) = descriptor else {
                    continue;
                };
                let Some((s, end, _)) = line_bounds(text, line) else {
                    continue;
                };
                let ts = tokens(&text[s..end], s);
                let (start_offset, end_offset) = ts.get(4).map_or_else(
                    || {
                        let at = ts.last().map_or(s, |t| t.end);
                        (at, at)
                    },
                    |t| (t.start, t.end),
                );
                fixes.push(if d.code == AwDiagnosticCodes::INVALID_DESCRIPTOR {
                    McFix::FixAccessWidenerDescriptorFix {
                        title: "Fix access widener descriptor".into(),
                        document_uri: uri.into(),
                        start_offset,
                        end_offset,
                        descriptor,
                    }
                } else {
                    McFix::AddAccessWidenerDescriptorFix {
                        title: "Add missing access widener descriptor".into(),
                        document_uri: uri.into(),
                        start_offset,
                        end_offset,
                        descriptor,
                    }
                });
            }
        }
        fixes
    }
    pub fn generate_entry_fix(
        uri: &str,
        entry: &AccessWidenerEntry,
        insert_line: Option<usize>,
    ) -> McFix {
        McFix::GenerateAccessWidenerEntryFix {
            title: "Generate access widener entry".into(),
            document_uri: uri.into(),
            insert_line: insert_line.unwrap_or(1),
            entry: AccessWidenerEditor::format_entry(entry),
        }
    }
    pub fn apply_fix(fix: &McFix, text: &str) -> Option<McFix> {
        let (uri, edits) = match fix {
            McFix::GenerateAccessWidenerEntryFix {
                document_uri,
                insert_line,
                entry,
                ..
            } => {
                let at = line_bounds(text, (*insert_line).max(1)).map_or(text.len(), |(s, _, _)| s);
                let nl = newline(text);
                let prefix = if at > 0 && !text[..at].ends_with('\n') {
                    nl
                } else {
                    ""
                };
                (
                    document_uri,
                    vec![McTextEdit {
                        start_offset: at,
                        end_offset: at,
                        new_text: format!("{prefix}{entry}{nl}"),
                    }],
                )
            }
            McFix::RemapAccessWidenerEntryFix {
                document_uri,
                line_number,
                new_line,
                ..
            } => {
                let (s, e, _) = line_bounds(text, *line_number)?;
                let old = tokens(&text[s..e], s);
                let new = tokens(new_line, 0);
                if old.len() != new.len() {
                    return None;
                }
                (
                    document_uri,
                    old.iter()
                        .zip(new)
                        .filter(|(a, b)| a.text != b.text)
                        .map(|(a, b)| McTextEdit {
                            start_offset: a.start,
                            end_offset: a.end,
                            new_text: b.text.into(),
                        })
                        .collect(),
                )
            }
            McFix::FixAccessWidenerDescriptorFix {
                document_uri,
                start_offset,
                end_offset,
                descriptor,
                ..
            } => {
                if !text.is_char_boundary(*start_offset)
                    || !text.is_char_boundary(*end_offset)
                    || start_offset > end_offset
                {
                    return None;
                }
                (
                    document_uri,
                    vec![McTextEdit {
                        start_offset: *start_offset,
                        end_offset: *end_offset,
                        new_text: descriptor.clone(),
                    }],
                )
            }
            McFix::AddAccessWidenerDescriptorFix {
                document_uri,
                start_offset,
                end_offset,
                descriptor,
                ..
            } => {
                if !text.is_char_boundary(*start_offset)
                    || !text.is_char_boundary(*end_offset)
                    || start_offset > end_offset
                {
                    return None;
                }
                (
                    document_uri,
                    vec![McTextEdit {
                        start_offset: *start_offset,
                        end_offset: *end_offset,
                        new_text: format!(" {descriptor}"),
                    }],
                )
            }
            McFix::RemoveDuplicateAccessWidenerEntryFix {
                document_uri,
                line_number,
                ..
            } => {
                let (s, _, e) = line_bounds(text, *line_number)?;
                (
                    document_uri,
                    vec![McTextEdit {
                        start_offset: s,
                        end_offset: e,
                        new_text: String::new(),
                    }],
                )
            }
            _ => return None,
        };
        Some(workspace_edit(fix, uri, edits))
    }
}
