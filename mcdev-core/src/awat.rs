//! Access-widener/access-transformer buffer routing. Offsets are UTF-8 byte offsets.
use crate::{at::*, aw::*, mapping::ProjectMappingContext, model::*};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwAtFileType {
    AccessWidener,
    AccessTransformer,
}
pub struct AwAtBufferSupport;
impl AwAtBufferSupport {
    pub fn detect_file_type(language_id: &str, document_uri: &str) -> Option<AwAtFileType> {
        match language_id.to_lowercase().as_str() {
            "accesswidener" => return Some(AwAtFileType::AccessWidener),
            "accesstransformer" => return Some(AwAtFileType::AccessTransformer),
            _ => (),
        }
        let path = document_uri.rsplit('/').next()?.to_lowercase();
        if path.ends_with(".accesswidener") || path.ends_with(".aw") {
            Some(AwAtFileType::AccessWidener)
        } else if path.ends_with("_at.cfg")
            || path == "accesstransformer.cfg"
            || path.ends_with(".at")
        {
            Some(AwAtFileType::AccessTransformer)
        } else {
            None
        }
    }
}
pub struct AwAtFacadeRequest<'a> {
    pub buffer_text: &'a str,
    pub line: usize,
    pub character: usize,
    pub document_uri: &'a str,
    pub file_type: AwAtFileType,
}
pub struct AwAtServiceFacade<'a> {
    pub class_index: &'a dyn ClassIndex,
    pub mapping_context: Option<&'a ProjectMappingContext>,
}
impl<'a> AwAtServiceFacade<'a> {
    pub fn new(
        class_index: &'a dyn ClassIndex,
        mapping_context: Option<&'a ProjectMappingContext>,
    ) -> Self {
        Self {
            class_index,
            mapping_context,
        }
    }
    pub fn complete(&self, request: &AwAtFacadeRequest<'_>) -> Vec<McCompletionItem> {
        clamped_position_to_offset(request.buffer_text, request.line, request.character)
            .map_or_else(Vec::new, |o| {
                self.complete_at_offset(request.file_type, request.buffer_text, o)
            })
    }
    pub fn complete_at_offset(
        &self,
        file_type: AwAtFileType,
        text: &str,
        offset: usize,
    ) -> Vec<McCompletionItem> {
        match file_type {
            AwAtFileType::AccessWidener => AwContextExtractor::extract_at_offset(text, offset)
                .map_or_else(Vec::new, |c| {
                    AccessWidenerCompletionService::new(self.class_index).complete(
                        &c,
                        self.mapping_context,
                        Default::default(),
                    )
                }),
            AwAtFileType::AccessTransformer => AtContextExtractor::extract_at_offset(text, offset)
                .map_or_else(Vec::new, |c| {
                    AccessTransformerCompletionService::new(self.class_index, self.mapping_context)
                        .complete(&c)
                }),
        }
    }
    pub fn diagnose(&self, request: &AwAtFacadeRequest<'_>) -> Vec<McDiagnostic> {
        self.analyze(request.file_type, request.buffer_text, request.document_uri)
    }
    pub fn analyze(&self, file_type: AwAtFileType, text: &str, uri: &str) -> Vec<McDiagnostic> {
        match file_type {
            AwAtFileType::AccessWidener => AccessWidenerDiagnosticsService::new(self.class_index)
                .analyze(text, uri, self.mapping_context),
            AwAtFileType::AccessTransformer => {
                AccessTransformerDiagnosticsService::new(self.class_index, self.mapping_context)
                    .analyze(text, uri)
            }
        }
    }
    pub fn definitions_at(&self, request: &AwAtFacadeRequest<'_>) -> Vec<McDefinitionTarget> {
        clamped_position_to_offset(request.buffer_text, request.line, request.character)
            .map_or_else(Vec::new, |o| {
                self.definitions_at_offset(request.file_type, request.buffer_text, o)
            })
    }
    pub fn definitions_at_offset(
        &self,
        file_type: AwAtFileType,
        text: &str,
        offset: usize,
    ) -> Vec<McDefinitionTarget> {
        match file_type {
            AwAtFileType::AccessWidener => {
                AccessWidenerDefinitionService::new(self.class_index, self.mapping_context)
                    .definitions_at_offset(text, offset)
            }
            AwAtFileType::AccessTransformer => {
                AccessTransformerDefinitionService::new(self.class_index, self.mapping_context)
                    .definitions_at_offset(text, offset)
            }
        }
    }
    pub fn find_references(
        &self,
        target: &McDefinitionTarget,
        sources: &[SourceScanEntry],
    ) -> Vec<McReferenceLocation> {
        sources
            .iter()
            .flat_map(
                |s| match AwAtBufferSupport::detect_file_type("", &s.document_uri) {
                    Some(AwAtFileType::AccessWidener) => {
                        AccessWidenerReferenceService::new(self.class_index, self.mapping_context)
                            .find_references(target, std::slice::from_ref(s))
                    }
                    Some(AwAtFileType::AccessTransformer) => {
                        AccessTransformerReferenceService::new(self.class_index)
                            .find_references(target, std::slice::from_ref(s))
                    }
                    None => vec![],
                },
            )
            .collect()
    }
    pub fn code_actions(&self, request: &AwAtFacadeRequest<'_>, code: Option<&str>) -> Vec<McFix> {
        let diagnostics = self
            .diagnose(request)
            .into_iter()
            .filter(|d| code.is_none_or(|c| c == d.code))
            .collect::<Vec<_>>();
        match request.file_type {
            AwAtFileType::AccessWidener => AccessWidenerCodeActionService::fixes_for_diagnostics(
                &diagnostics,
                request.document_uri,
                request.buffer_text,
                Some(self.class_index),
                self.mapping_context,
            ),
            AwAtFileType::AccessTransformer => {
                AccessTransformerCodeActionService::new(self.class_index, self.mapping_context)
                    .fixes_for_diagnostics(&diagnostics, request.document_uri, request.buffer_text)
            }
        }
    }
    pub fn apply_fix(&self, fix: &McFix, text: &str) -> Option<McFix> {
        AccessWidenerCodeActionService::apply_fix(fix, text).or_else(|| {
            AccessTransformerCodeActionService::new(self.class_index, self.mapping_context)
                .apply_fix(fix, text)
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Token<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}
pub(crate) fn tokens(line: &str, start: usize) -> Vec<Token<'_>> {
    let content = line.split('#').next().unwrap_or("");
    let mut result = vec![];
    let mut begin = None;
    for (at, ch) in content
        .char_indices()
        .chain(std::iter::once((content.len(), ' ')))
    {
        if ch.is_whitespace() {
            if let Some(b) = begin.take() {
                result.push(Token {
                    text: &content[b..at],
                    start: start + b,
                    end: start + at,
                });
            }
        } else if begin.is_none() {
            begin = Some(at);
        }
    }
    result
}
pub(crate) fn line_bounds(text: &str, line: usize) -> Option<(usize, usize, usize)> {
    if line == 0 {
        return None;
    }
    let start = if line == 1 {
        0
    } else {
        text.match_indices('\n').nth(line - 2)?.0 + 1
    };
    let raw_end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    let end = if raw_end > start && text.as_bytes()[raw_end - 1] == b'\r' {
        raw_end - 1
    } else {
        raw_end
    };
    Some((
        start,
        end,
        if raw_end < text.len() {
            raw_end + 1
        } else {
            raw_end
        },
    ))
}
pub(crate) fn clamped_position_to_offset(
    text: &str,
    line: usize,
    character: usize,
) -> Option<usize> {
    let (start, end, _) = line_bounds(text, line.checked_add(1)?)?;
    position_to_offset(
        text,
        McTextPosition {
            line,
            character: character.min(text[start..end].encode_utf16().count()),
        },
    )
}
pub(crate) fn range(text: &str, start: usize, end: usize) -> McTextRange {
    McTextRange {
        start: offset_to_position(text, start).expect("validated token start"),
        end: offset_to_position(text, end).expect("validated token end"),
    }
}
pub(crate) fn line_range(text: &str, line: usize) -> McTextRange {
    let (s, e, _) = line_bounds(text, line).unwrap_or((0, text.len(), text.len()));
    range(text, s, e)
}
pub(crate) fn token_range(text: &str, line: usize, index: usize) -> McTextRange {
    let Some((s, e, _)) = line_bounds(text, line) else {
        return line_range(text, line);
    };
    tokens(&text[s..e], s)
        .get(index)
        .map_or_else(|| line_range(text, line), |t| range(text, t.start, t.end))
}
pub(crate) fn metadata(pairs: &[(&str, String)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
pub(crate) fn diagnostic(
    text: &str,
    line: usize,
    token: Option<usize>,
    code: &str,
    severity: McSeverity,
    message: String,
    extra: &[(&str, String)],
) -> McDiagnostic {
    McDiagnostic {
        code: code.into(),
        severity,
        message,
        range: token.map_or_else(|| line_range(text, line), |t| token_range(text, line, t)),
        metadata: metadata(extra),
    }
}
pub(crate) fn completion(
    label: String,
    detail: Option<String>,
    documentation: Option<String>,
    filter_text: String,
    insert_text: String,
    kind: McCompletionKind,
    sort_key: String,
    metadata: McCompletionMetadata,
) -> McCompletionItem {
    McCompletionItem {
        label,
        detail,
        documentation,
        filter_text,
        insert_text,
        kind,
        sort_key,
        metadata,
        additional_edits: vec![],
        insert_text_format: McCompletionInsertTextFormat::PlainText,
    }
}
pub(crate) fn definition(
    index: &dyn ClassIndex,
    owner: &str,
    kind: MemberKind,
    name: Option<String>,
    descriptor: Option<String>,
    source_range: McTextRange,
) -> McDefinitionTarget {
    McDefinitionTarget {
        kind,
        owner_internal_name: owner.into(),
        owner_fqn: Some(
            index
                .find_class(owner)
                .map_or_else(|| owner.replace('/', "."), |c| c.fqn()),
        ),
        name,
        descriptor,
        namespace: MappingNamespace::Named,
        source_range: Some(source_range),
        direct_source_document_uri: None,
    }
}
pub(crate) fn workspace_edit(fix: &McFix, uri: &str, edits: Vec<McTextEdit>) -> McFix {
    McFix::WorkspaceEditFix {
        title: fix.title().into(),
        kind: fix.kind().into(),
        document_uri: uri.into(),
        edits,
        metadata: BTreeMap::new(),
    }
}
pub(crate) fn replace_line(text: &str, line: usize, new_line: &str) -> String {
    let Some((s, e, _)) = line_bounds(text, line) else {
        return text.into();
    };
    format!("{}{}{}", &text[..s], new_line, &text[e..])
}
pub(crate) fn remove_line(text: &str, line: usize) -> String {
    let Some((s, _, end)) = line_bounds(text, line) else {
        return text.into();
    };
    format!("{}{}", &text[..s], &text[end..])
}
pub(crate) fn newline(text: &str) -> &str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}
