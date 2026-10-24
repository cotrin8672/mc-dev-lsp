use crate::dto::*;
use core::bytecode::{AtTargetCandidate, AtTargetKind};
use core::mapping::MappingResolver;
use core::mixin_completion::{AtTargetInsertFormatter, MixinClassInsertMode};
use core::mixin_edits::MixinImportEditBuilder;
use core::mixin_source::{AnnotationContext, AnnotationSlot, MixinAnnotation};
use core::{
    MappingNamespace, McCompletionInsertTextFormat, McCompletionItem, McCompletionKind,
    McCompletionReplacementRange,
};
use mcdev_core::{
    self as core, McDefinitionTarget, McDiagnostic, McFix, McReferenceLocation, McSeverity,
    McTextEdit, McTextPosition, McTextRange, MemberKind,
};
use std::collections::BTreeMap;

/// Source and replacement offsets are UTF-8 bytes. Every emitted range uses UTF-16 LSP characters.
/// Legacy debug scalar offsets must be converted explicitly by the caller; they are not byte offsets.
pub struct CompletionConvertContext<'a> {
    pub source: &'a str,
    pub annotation_context: Option<&'a AnnotationContext>,
    pub class_insert_mode: MixinClassInsertMode,
    pub preferred_at_target: &'a str,
    pub mapping_resolver: Option<&'a dyn MappingResolver>,
    pub source_namespace: MappingNamespace,
    pub runtime_namespace: MappingNamespace,
    pub replacement_range: Option<McCompletionReplacementRange>,
}
impl<'a> CompletionConvertContext<'a> {
    pub fn new(source: &'a str, annotation_context: Option<&'a AnnotationContext>) -> Self {
        Self {
            source,
            annotation_context,
            class_insert_mode: MixinClassInsertMode::Import,
            preferred_at_target: "smart",
            mapping_resolver: None,
            source_namespace: MappingNamespace::Named,
            runtime_namespace: MappingNamespace::Intermediary,
            replacement_range: None,
        }
    }
}
pub struct CompletionItemConverter;
impl CompletionItemConverter {
    pub fn is_incomplete(items: &[McCompletionItem]) -> bool {
        items
            .iter()
            .filter(|i| {
                i.kind == McCompletionKind::Class
                    && matches!(
                        i.metadata.source.as_str(),
                        "mixin.target" | "mixin.targets" | "aw.class" | "at.class"
                    )
            })
            .count()
            >= core::CLASS_COMPLETION_LIMIT
    }
    pub fn to_dto(
        item: &McCompletionItem,
        context: &CompletionConvertContext<'_>,
    ) -> Result<McdevCompletionItemDto, ProtocolConversionError> {
        let insert_text = Self::resolve_insert_text(item, context);
        let replacement = context.replacement_range.or_else(|| {
            context
                .annotation_context
                .map(|a| McCompletionReplacementRange {
                    start_offset: a.value_start_offset,
                    end_offset: a.value_end_offset.max(a.value_start_offset),
                })
        });
        let edit = replacement
            .map(|range| {
                Ok(McdevTextEdit {
                    range: offset_range(
                        context.source,
                        range.start_offset,
                        range.end_offset.max(range.start_offset),
                    )?,
                    new_text: insert_text.clone(),
                })
            })
            .transpose()?;
        let mut edits = item.additional_edits.clone();
        if context.class_insert_mode == MixinClassInsertMode::Import
            && item.metadata.source == "mixin.target"
            && context.annotation_context.is_some_and(|a| {
                a.annotation == MixinAnnotation::Mixin && a.slot == AnnotationSlot::Class
            })
        {
            let fqn = item.metadata.owner.as_ref().and_then(|owner| {
                if context.annotation_context.unwrap().is_kotlin {
                    MixinImportEditBuilder::reference_for_internal_name(
                        context.source,
                        owner,
                        &Default::default(),
                    )
                    .import_fqn
                } else {
                    Some(owner.replace('/', "."))
                }
            });
            if let Some(import) =
                fqn.and_then(|fqn| MixinImportEditBuilder::build_import_edit(context.source, &fqn))
            {
                if !edits.contains(&import) {
                    edits.push(import);
                }
            }
        }
        let mut all_edits = edits.clone();
        if let Some(replacement) = replacement {
            all_edits.push(McTextEdit {
                start_offset: replacement.start_offset,
                end_offset: replacement.end_offset.max(replacement.start_offset),
                new_text: insert_text.clone(),
            });
        }
        core::validate_text_edits(context.source, &all_edits).map_err(conversion_error)?;
        Ok(McdevCompletionItemDto {
            label: item.label.clone(),
            detail: item.detail.clone(),
            documentation: item.documentation.clone(),
            filter_text: item.filter_text.clone(),
            insert_text,
            kind: match item.kind {
                McCompletionKind::Class => "class",
                McCompletionKind::Method => "method",
                McCompletionKind::Field => "field",
                McCompletionKind::Keyword => "keyword",
                McCompletionKind::Value => "value",
            }
            .into(),
            sort_key: item.sort_key.clone(),
            edit,
            additional_edits: edits
                .iter()
                .map(|e| text_edit_to_dto(context.source, e))
                .collect::<Result<_, _>>()?,
            metadata: BTreeMap::from([
                ("source".into(), Some(item.metadata.source.clone())),
                ("owner".into(), item.metadata.owner.clone()),
                ("name".into(), item.metadata.name.clone()),
                ("descriptor".into(), item.metadata.descriptor.clone()),
                ("namespace".into(), item.metadata.namespace.clone()),
            ]),
            insert_text_format: match item.insert_text_format {
                McCompletionInsertTextFormat::PlainText => "plainText",
                McCompletionInsertTextFormat::Snippet => "snippet",
            }
            .into(),
        })
    }
    pub fn to_dtos(
        items: &[McCompletionItem],
        context: &CompletionConvertContext<'_>,
    ) -> Result<Vec<McdevCompletionItemDto>, ProtocolConversionError> {
        items.iter().map(|i| Self::to_dto(i, context)).collect()
    }
    pub fn extract_annotation_context(
        source: &str,
        line: usize,
        character: usize,
    ) -> Option<AnnotationContext> {
        core::mixin_source::AnnotationContextExtractor::extract(source, line, character)
    }
    fn resolve_insert_text(
        item: &McCompletionItem,
        context: &CompletionConvertContext<'_>,
    ) -> String {
        if (context
            .preferred_at_target
            .eq_ignore_ascii_case("descriptor")
            || context.preferred_at_target.eq_ignore_ascii_case("smart"))
            && item.metadata.source == "mixin.atTarget"
            && context.mapping_resolver.is_some()
        {
            if let (Some(owner), Some(name), Some(descriptor)) = (
                &item.metadata.owner,
                &item.metadata.name,
                &item.metadata.descriptor,
            ) {
                let kind = if item.insert_text == "RETURN" {
                    AtTargetKind::Return
                } else if item.insert_text.starts_with('"') {
                    AtTargetKind::Constant
                } else if item.insert_text.contains("<init>") {
                    AtTargetKind::New
                } else if item.kind == McCompletionKind::Field {
                    AtTargetKind::Field
                } else if item.insert_text.ends_with(';')
                    && !item
                        .insert_text
                        .split_once(';')
                        .map_or(item.insert_text.as_str(), |(_, rest)| rest)
                        .contains('(')
                {
                    AtTargetKind::New
                } else {
                    AtTargetKind::Invoke
                };
                let mut candidate = AtTargetCandidate::new(kind, owner, name, descriptor);
                candidate.display_label = item.label.clone();
                candidate.detail = item.detail.clone().unwrap_or_default();
                return AtTargetInsertFormatter::format_insert(
                    &candidate,
                    context.mapping_resolver,
                    context.source_namespace,
                    context.runtime_namespace,
                );
            }
        }
        item.insert_text.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolConversionError {
    pub message: String,
}
impl std::fmt::Display for ProtocolConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProtocolConversionError {}
fn conversion_error(message: impl Into<String>) -> ProtocolConversionError {
    ProtocolConversionError {
        message: message.into(),
    }
}
pub fn position_to_dto(position: McTextPosition) -> Result<McdevPosition, ProtocolConversionError> {
    Ok(McdevPosition {
        line: i32::try_from(position.line)
            .map_err(|_| conversion_error("line exceeds protocol integer range"))?,
        character: i32::try_from(position.character)
            .map_err(|_| conversion_error("character exceeds protocol integer range"))?,
    })
}
pub fn range_to_dto(range: McTextRange) -> Result<McdevRange, ProtocolConversionError> {
    Ok(McdevRange {
        start: position_to_dto(range.start)?,
        end: position_to_dto(range.end)?,
    })
}
pub fn offset_range(
    source: &str,
    start: usize,
    end: usize,
) -> Result<McdevRange, ProtocolConversionError> {
    if start > end {
        return Err(conversion_error("reversed text edit range"));
    }
    let start = core::offset_to_position(source, start)
        .ok_or_else(|| conversion_error("invalid UTF-8 text edit start"))?;
    let end = core::offset_to_position(source, end)
        .ok_or_else(|| conversion_error("invalid UTF-8 text edit end"))?;
    range_to_dto(McTextRange { start, end })
}
pub fn text_edit_to_dto(
    source: &str,
    edit: &McTextEdit,
) -> Result<McdevTextEdit, ProtocolConversionError> {
    Ok(McdevTextEdit {
        range: offset_range(source, edit.start_offset, edit.end_offset)?,
        new_text: edit.new_text.clone(),
    })
}

pub struct DiagnosticConverter;
impl DiagnosticConverter {
    pub fn to_dto(
        diagnostic: &McDiagnostic,
    ) -> Result<McdevDiagnosticDto, ProtocolConversionError> {
        Ok(McdevDiagnosticDto {
            code: diagnostic.code.clone(),
            severity: match diagnostic.severity {
                McSeverity::Error => "error",
                McSeverity::Warning => "warning",
                McSeverity::Info => "information",
            }
            .into(),
            message: diagnostic.message.clone(),
            range: range_to_dto(diagnostic.range)?,
            metadata: diagnostic.metadata.clone(),
        })
    }
    pub fn to_dtos(
        diagnostics: &[McDiagnostic],
    ) -> Result<Vec<McdevDiagnosticDto>, ProtocolConversionError> {
        diagnostics.iter().map(Self::to_dto).collect()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDefinition {
    pub target: McDefinitionTarget,
    pub document_uri: String,
    pub range: McTextRange,
    pub resolution: McdevDefinitionResolution,
    pub resolution_message: Option<String>,
}
pub struct DefinitionConverter;
impl DefinitionConverter {
    pub fn to_location(
        resolved: &ResolvedDefinition,
    ) -> Result<McdevLocation, ProtocolConversionError> {
        let target = &resolved.target;
        let navigation = matches!(
            resolved.resolution,
            McdevDefinitionResolution::Source | McdevDefinitionResolution::Jdt
        );
        let mut metadata = BTreeMap::from([
            (
                "kind".into(),
                match target.kind {
                    MemberKind::Class => "class",
                    MemberKind::Method => "method",
                    MemberKind::Field => "field",
                    MemberKind::Constructor => "constructor",
                }
                .into(),
            ),
            ("owner".into(), target.owner_internal_name.clone()),
            ("namespace".into(), namespace_name(target.namespace).into()),
        ]);
        for (key, value) in [
            ("fqn", &target.owner_fqn),
            ("name", &target.name),
            ("descriptor", &target.descriptor),
        ] {
            if let Some(value) = value {
                metadata.insert(key.into(), value.clone());
            }
        }
        Ok(McdevLocation {
            document_uri: if navigation {
                resolved.document_uri.clone()
            } else {
                String::new()
            },
            range: range_to_dto(if navigation {
                resolved.range
            } else {
                target.source_range.unwrap_or_default()
            })?,
            metadata,
            resolution: resolved.resolution,
            resolution_message: resolved.resolution_message.clone(),
        })
    }
    pub fn to_locations(
        resolved: &[ResolvedDefinition],
    ) -> Result<Vec<McdevLocation>, ProtocolConversionError> {
        resolved.iter().map(Self::to_location).collect()
    }
    pub fn reference_to_location(
        reference: &McReferenceLocation,
    ) -> Result<McdevLocation, ProtocolConversionError> {
        Ok(McdevLocation {
            document_uri: reference.document_uri.clone(),
            range: range_to_dto(reference.range)?,
            metadata: reference.metadata.clone(),
            resolution: McdevDefinitionResolution::Source,
            resolution_message: None,
        })
    }
    pub fn references_to_locations(
        references: &[McReferenceLocation],
    ) -> Result<Vec<McdevLocation>, ProtocolConversionError> {
        references.iter().map(Self::reference_to_location).collect()
    }
}
pub fn namespace_name(namespace: core::MappingNamespace) -> &'static str {
    use core::MappingNamespace::*;
    match namespace {
        Named => "NAMED",
        Intermediary => "INTERMEDIARY",
        Official => "OFFICIAL",
        Srg => "SRG",
        Mcp => "MCP",
    }
}
pub struct CodeActionConverter;
impl CodeActionConverter {
    /// Resolve domain-specific fixes through the owning service; return the edited document's text with its workspace fix.
    pub fn to_dto<'a>(
        fix: &McFix,
        source: &'a str,
        resolve: impl FnOnce(&McFix) -> Option<(&'a str, McFix)>,
    ) -> Result<Option<McdevCodeActionDto>, ProtocolConversionError> {
        match fix {
            McFix::WorkspaceEditFix { .. } => {
                Self::workspace_edit_to_dto(fix, fix, source).map(Some)
            }
            McFix::GenerateAccessorMethodFix {
                document_uri,
                insert_offset,
                method_source,
                ..
            }
            | McFix::GenerateInvokerMethodFix {
                document_uri,
                insert_offset,
                method_source,
                ..
            } => {
                let workspace = McFix::WorkspaceEditFix {
                    title: fix.title().into(),
                    kind: fix.kind().into(),
                    document_uri: document_uri.clone(),
                    edits: vec![McTextEdit {
                        start_offset: *insert_offset,
                        end_offset: *insert_offset,
                        new_text: method_source.clone(),
                    }],
                    metadata: Default::default(),
                };
                Self::workspace_edit_to_dto(fix, &workspace, source).map(Some)
            }
            _ => resolve(fix)
                .map(|(content, workspace)| Self::workspace_edit_to_dto(fix, &workspace, content))
                .transpose(),
        }
    }
    pub fn to_dtos<'a>(
        fixes: &[McFix],
        source: &'a str,
        resolve: impl Fn(&McFix) -> Option<(&'a str, McFix)>,
    ) -> Result<Vec<McdevCodeActionDto>, ProtocolConversionError> {
        fixes
            .iter()
            .map(|fix| Self::to_dto(fix, source, &resolve))
            .filter_map(|value| match value {
                Ok(None) => None,
                Ok(Some(dto)) => Some(Ok(dto)),
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
    /// Domain services resolve symbolic fixes before conversion. The source must belong to the edited document.
    pub fn workspace_edit_to_dto(
        original: &McFix,
        resolved: &McFix,
        source: &str,
    ) -> Result<McdevCodeActionDto, ProtocolConversionError> {
        let McFix::WorkspaceEditFix {
            document_uri,
            edits,
            ..
        } = resolved
        else {
            return Err(conversion_error(
                "code action must resolve to workspace edits",
            ));
        };
        core::validate_text_edits(source, edits).map_err(conversion_error)?;
        Ok(McdevCodeActionDto {
            title: original.title().into(),
            kind: original.kind().into(),
            edits: vec![McdevWorkspaceEdit {
                document_uri: document_uri.clone(),
                edits: edits
                    .iter()
                    .map(|e| text_edit_to_dto(source, e))
                    .collect::<Result<_, _>>()?,
            }],
            metadata: Self::metadata_for(original),
        })
    }
    pub fn metadata_for(fix: &McFix) -> BTreeMap<String, String> {
        match fix {
            McFix::WorkspaceEditFix { metadata, .. } => metadata.clone(),
            McFix::AddMixinConfigEntryFix {
                config_path,
                mixin_class_name,
                mixin_package,
                ..
            } => {
                let mut result = BTreeMap::from([
                    ("mixinClass".into(), mixin_class_name.clone()),
                    ("configPath".into(), config_path.clone()),
                ]);
                if let Some(package) = mixin_package {
                    result.insert("mixinPackage".into(), package.clone());
                }
                result
            }
            McFix::GenerateAccessorMethodFix { field_name, .. } => {
                BTreeMap::from([("field".into(), field_name.clone())])
            }
            McFix::GenerateInvokerMethodFix {
                target_method_name, ..
            } => BTreeMap::from([("method".into(), target_method_name.clone())]),
            _ => BTreeMap::new(),
        }
    }
}
