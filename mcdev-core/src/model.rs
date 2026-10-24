use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MappingNamespace {
    Named,
    Intermediary,
    Official,
    Srg,
    Mcp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemberKind {
    Class,
    Method,
    Field,
    Constructor,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassRef {
    pub internal_name: String,
    pub namespace: MappingNamespace,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodRef {
    pub owner: ClassRef,
    pub name: String,
    pub descriptor: String,
    pub namespace: MappingNamespace,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldRef {
    pub owner: ClassRef,
    pub name: String,
    pub descriptor: String,
    pub namespace: MappingNamespace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McSeverity {
    Error,
    Warning,
    Info,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct McTextPosition {
    pub line: usize,
    pub character: usize,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct McTextRange {
    pub start: McTextPosition,
    pub end: McTextPosition,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McDiagnostic {
    pub code: String,
    pub severity: McSeverity,
    pub message: String,
    pub range: McTextRange,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McCompletionKind {
    Class,
    Method,
    Field,
    Keyword,
    Value,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McCompletionInsertTextFormat {
    #[default]
    PlainText,
    Snippet,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McCompletionMetadata {
    pub source: String,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub descriptor: Option<String>,
    pub namespace: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McCompletionReplacementRange {
    pub start_offset: usize,
    pub end_offset: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McCompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub filter_text: String,
    pub insert_text: String,
    pub kind: McCompletionKind,
    pub sort_key: String,
    pub metadata: McCompletionMetadata,
    #[serde(default)]
    pub additional_edits: Vec<McTextEdit>,
    #[serde(default)]
    pub insert_text_format: McCompletionInsertTextFormat,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McTextEdit {
    pub start_offset: usize,
    pub end_offset: usize,
    pub new_text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum McFix {
    WorkspaceEditFix {
        title: String,
        kind: String,
        document_uri: String,
        edits: Vec<McTextEdit>,
        metadata: BTreeMap<String, String>,
    },
    AddMixinConfigEntryFix {
        title: String,
        config_path: String,
        mixin_class_name: String,
        mixin_package: Option<String>,
        array_name: String,
    },
    AddMethodDescriptorFix {
        title: String,
        document_uri: String,
        start_offset: usize,
        end_offset: usize,
        method_name: String,
        descriptor: String,
    },
    GenerateAccessorMethodFix {
        title: String,
        document_uri: String,
        insert_offset: usize,
        method_source: String,
        field_name: String,
        is_getter: bool,
    },
    GenerateInvokerMethodFix {
        title: String,
        document_uri: String,
        insert_offset: usize,
        method_source: String,
        target_method_name: String,
    },
    AddAccessTransformerEntryFix {
        title: String,
        document_uri: String,
        modifier: String,
        owner: String,
        member_name: Option<String>,
        member_descriptor: Option<String>,
        insert_line: usize,
    },
    RemapAccessTransformerEntryFix {
        title: String,
        document_uri: String,
        line: usize,
    },
    AddAtMethodDescriptorFix {
        title: String,
        document_uri: String,
        line: usize,
        member_name: String,
        descriptor: String,
    },
    RemoveDuplicateAtEntryFix {
        title: String,
        document_uri: String,
        line: usize,
    },
    GenerateAccessWidenerEntryFix {
        title: String,
        document_uri: String,
        insert_line: usize,
        entry: String,
    },
    RemapAccessWidenerEntryFix {
        title: String,
        document_uri: String,
        line_number: usize,
        new_line: String,
    },
    AddAccessWidenerDescriptorFix {
        title: String,
        document_uri: String,
        start_offset: usize,
        end_offset: usize,
        descriptor: String,
    },
    FixAccessWidenerDescriptorFix {
        title: String,
        document_uri: String,
        start_offset: usize,
        end_offset: usize,
        descriptor: String,
    },
    RemoveDuplicateAccessWidenerEntryFix {
        title: String,
        document_uri: String,
        line_number: usize,
    },
}
impl McFix {
    pub fn title(&self) -> &str {
        match self {
            Self::WorkspaceEditFix { title, .. }
            | Self::AddMixinConfigEntryFix { title, .. }
            | Self::AddMethodDescriptorFix { title, .. }
            | Self::GenerateAccessorMethodFix { title, .. }
            | Self::GenerateInvokerMethodFix { title, .. }
            | Self::AddAccessTransformerEntryFix { title, .. }
            | Self::RemapAccessTransformerEntryFix { title, .. }
            | Self::AddAtMethodDescriptorFix { title, .. }
            | Self::RemoveDuplicateAtEntryFix { title, .. }
            | Self::GenerateAccessWidenerEntryFix { title, .. }
            | Self::RemapAccessWidenerEntryFix { title, .. }
            | Self::AddAccessWidenerDescriptorFix { title, .. }
            | Self::FixAccessWidenerDescriptorFix { title, .. }
            | Self::RemoveDuplicateAccessWidenerEntryFix { title, .. } => title,
        }
    }
    pub fn kind(&self) -> &str {
        match self {
            Self::WorkspaceEditFix { kind, .. } => kind,
            Self::AddMixinConfigEntryFix { .. } => "quickfix.mixin.config",
            Self::AddMethodDescriptorFix { .. } => "quickfix.mixin.methodDescriptor",
            Self::GenerateAccessorMethodFix { .. } => "quickfix.mixin.generateAccessor",
            Self::GenerateInvokerMethodFix { .. } => "quickfix.mixin.generateInvoker",
            Self::AddAccessTransformerEntryFix { .. } => "quickfix.at.generateEntry",
            Self::RemapAccessTransformerEntryFix { .. } => "quickfix.at.remapNamespace",
            Self::AddAtMethodDescriptorFix { .. } => "quickfix.at.addDescriptor",
            Self::RemoveDuplicateAtEntryFix { .. } => "quickfix.at.removeDuplicate",
            Self::GenerateAccessWidenerEntryFix { .. } => "quickfix.aw.generateEntry",
            Self::RemapAccessWidenerEntryFix { .. } => "quickfix.aw.remapNamespace",
            Self::AddAccessWidenerDescriptorFix { .. } => "quickfix.aw.addDescriptor",
            Self::FixAccessWidenerDescriptorFix { .. } => "quickfix.aw.fixDescriptor",
            Self::RemoveDuplicateAccessWidenerEntryFix { .. } => "quickfix.aw.removeDuplicate",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McDefinitionTarget {
    pub kind: MemberKind,
    pub owner_internal_name: String,
    pub owner_fqn: Option<String>,
    pub name: Option<String>,
    pub descriptor: Option<String>,
    pub namespace: MappingNamespace,
    pub source_range: Option<McTextRange>,
    pub direct_source_document_uri: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McReferenceLocation {
    pub document_uri: String,
    pub range: McTextRange,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceScanEntry {
    pub document_uri: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassIndexEntry {
    pub simple_name: String,
    pub package_name: String,
    pub internal_name: String,
}
impl ClassIndexEntry {
    pub fn fqn(&self) -> String {
        if self.package_name.is_empty() {
            self.simple_name.clone()
        } else {
            format!("{}.{}", self.package_name, self.simple_name)
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MethodIndexEntry {
    pub name: String,
    pub descriptor: String,
    pub is_static: bool,
    pub readable_signature: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldIndexEntry {
    pub name: String,
    pub descriptor: String,
    pub is_static: bool,
    pub readable_type: String,
}
pub const CLASS_COMPLETION_LIMIT: usize = 50;
pub trait ClassIndex: Send + Sync {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry>;
    fn find_class(&self, internal_name: &str) -> Option<ClassIndexEntry>;
    fn find_class_by_fqn(&self, fqn: &str) -> Option<ClassIndexEntry>;
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry>;
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry>;
}

/// Offsets are UTF-8 bytes; LSP characters are UTF-16 code units. Invalid boundaries are rejected.
pub fn offset_to_position(text: &str, offset: usize) -> Option<McTextPosition> {
    if !text.is_char_boundary(offset) {
        return None;
    }
    let prefix = &text[..offset];
    let line = prefix.bytes().filter(|b| *b == b'\n').count();
    let start = prefix.rfind('\n').map_or(0, |p| p + 1);
    Some(McTextPosition {
        line,
        character: text[start..offset].encode_utf16().count(),
    })
}
pub fn position_to_offset(text: &str, position: McTextPosition) -> Option<usize> {
    let start = if position.line == 0 {
        0
    } else {
        text.match_indices('\n').nth(position.line - 1)?.0 + 1
    };
    let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    let mut units = 0;
    for (at, ch) in text[start..end].char_indices() {
        if units == position.character {
            return Some(start + at);
        }
        units += ch.len_utf16();
        if units > position.character {
            return None;
        }
    }
    (units == position.character).then_some(end)
}
fn checked_text_edits<'a>(
    text: &str,
    edits: &'a [McTextEdit],
) -> Result<Vec<&'a McTextEdit>, String> {
    let mut sorted = edits.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|e| (e.start_offset, e.end_offset));
    let mut last = 0;
    for (i, edit) in sorted.iter().enumerate() {
        if edit.start_offset > edit.end_offset
            || !text.is_char_boundary(edit.start_offset)
            || !text.is_char_boundary(edit.end_offset)
        {
            return Err("invalid text edit boundary".into());
        }
        if edit.start_offset < last || (i > 0 && edit.start_offset == sorted[i - 1].start_offset) {
            return Err("overlapping text edits".into());
        }
        last = edit.end_offset;
    }
    Ok(sorted)
}
pub fn validate_text_edits(text: &str, edits: &[McTextEdit]) -> Result<(), String> {
    checked_text_edits(text, edits).map(|_| ())
}
pub fn apply_text_edits(text: &str, edits: &[McTextEdit]) -> Result<String, String> {
    let sorted = checked_text_edits(text, edits)?;
    let mut last = 0;
    let mut output = String::new();
    for edit in sorted {
        output.push_str(&text[last..edit.start_offset]);
        output.push_str(&edit.new_text);
        last = edit.end_offset;
    }
    output.push_str(&text[last..]);
    Ok(output)
}
