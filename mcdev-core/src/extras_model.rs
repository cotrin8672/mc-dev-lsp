//! Shared normalized MixinExtras facts, independent of the host and transport.
use crate::mixin_source::MixinAnnotation;
use crate::McTextRange;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ops::Range;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerParameterSpec {
    pub name: String,
    pub type_descriptor: String,
    pub readable_type: String,
    pub is_operation: bool,
    pub operation_generic_descriptor: Option<String>,
    pub generic_type_descriptor: Option<String>,
    pub accepted_type_descriptors: BTreeSet<String>,
    pub accepted_operation_generic_descriptors: BTreeSet<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerSignatureSpec {
    pub return_type_descriptor: String,
    pub readable_return_type: String,
    pub parameters: Vec<HandlerParameterSpec>,
    pub operation_call_args: Vec<String>,
    pub optional_captured_target_parameters: Vec<HandlerParameterSpec>,
    pub accepted_return_type_descriptors: BTreeSet<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalSugarSpec {
    pub args_only: bool,
    pub index: Option<i32>,
    pub ordinal: Option<i32>,
    pub names: BTreeSet<String>,
    pub print: bool,
    pub type_class_name: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareSugarSpec {
    pub value: Option<String>,
    pub namespace: Option<String>,
    pub namespace_specified: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandlerParameterSugarSpec {
    Local(LocalSugarSpec),
    Share(ShareSugarSpec),
    Cancellable,
}
pub type HandlerSugarSpec = HandlerParameterSugarSpec;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerParameterDeclaration {
    pub name: String,
    pub type_name: String,
    pub type_descriptor: Option<String>,
    pub is_operation: bool,
    pub operation_generic_name: Option<String>,
    pub is_sugar: bool,
    pub sugar_spec: Option<HandlerParameterSugarSpec>,
    pub range: Option<McTextRange>,
    pub sugar_annotation_range: Option<McTextRange>,
    pub has_coerce: bool,
    pub coerce_annotation_range: Option<McTextRange>,
    pub operation_generic_descriptor: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerMethodDeclaration {
    pub method_name: String,
    pub return_type_name: String,
    pub return_type_descriptor: Option<String>,
    pub parameters: Vec<HandlerParameterDeclaration>,
    pub range: McTextRange,
    pub is_static: bool,
    pub has_method_level_coerce: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostHandlerSource {
    pub document_uri: String,
    pub sha256: String,
    pub qualified_name: String,
    pub targets: Vec<String>,
    pub sites: crate::host_source::HostFact<Vec<MixinExtrasAnnotationSite>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slice_selections: Vec<HostSliceSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostShareSource {
    pub text: String,
    pub handlers: HostHandlerSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSliceSelection {
    pub site_index: usize,
    pub slice: HostMethodSlice,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMethodSlice {
    pub id: String,
    pub from: Option<HostSliceAt>,
    pub to: Option<HostSliceAt>,
    pub range: McTextRange,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSliceAt {
    pub value: String,
    pub target: Option<String>,
    pub args: Vec<String>,
    pub id: Option<String>,
    pub ordinal: Option<i32>,
    pub shift: AtShiftSpec,
    pub range: McTextRange,
    pub target_range: Option<McTextRange>,
    pub ordinal_range: Option<McTextRange>,
}
impl HostHandlerSource {
    pub fn validate_source(&self, source: &str) -> Result<(), crate::host_source::HostSourceError> {
        use crate::host_source::HostSourceError;
        use sha2::{Digest, Sha256};
        if self.sha256 != format!("{:x}", Sha256::digest(source.as_bytes())) {
            return Err(HostSourceError::Failure(
                "normalized handler source digest is stale".into(),
            ));
        }
        Ok(())
    }
    pub fn checked_sites(
        &self,
        source: &str,
    ) -> Result<&[MixinExtrasAnnotationSite], crate::host_source::HostSourceError> {
        use crate::host_source::{HostFact, HostSourceError};
        self.validate_source(source)?;
        match &self.sites {
            HostFact::Known(sites) => {
                for handler in sites.iter().filter_map(|site| site.handler_method.as_ref()) {
                    if handler.return_type_descriptor.is_none()
                        || handler
                            .parameters
                            .iter()
                            .any(|parameter| parameter.type_descriptor.is_none())
                    {
                        return Err(HostSourceError::Failure(
                            "normalized handler type facts are unavailable".into(),
                        ));
                    }
                    for parameter in &handler.parameters {
                        if parameter.is_operation
                            && parameter.operation_generic_descriptor.is_none()
                        {
                            return Err(HostSourceError::Failure(
                                "Operation generic type facts are unavailable".into(),
                            ));
                        }
                        if parameter.is_sugar
                            && matches!(
                                parameter.sugar_spec,
                                Some(
                                    HandlerParameterSugarSpec::Local(_)
                                        | HandlerParameterSugarSpec::Share(_)
                                )
                            )
                            && parameter.type_descriptor.as_deref()
                                == Some("Lcom/llamalad7/mixinextras/sugar/ref/LocalRef;")
                        {
                            return Err(HostSourceError::Failure(
                                "LocalRef generic value type facts are unavailable".into(),
                            ));
                        }
                    }
                }
                Ok(sites)
            }
            HostFact::Unavailable { reason } => Err(HostSourceError::Failure(reason.clone())),
            HostFact::Missing => Err(HostSourceError::Failure(
                "normalized handler facts are unavailable".into(),
            )),
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtShiftSpec {
    #[default]
    Before,
    After,
    By {
        offset: i32,
    },
    Unresolved,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MixinExtrasAnnotation {
    ModifyExpressionValue,
    ModifyReturnValue,
    ModifyReceiver,
    WrapOperation,
    WrapWithCondition,
    WrapMethod,
    Inject,
    Redirect,
    ModifyArg,
    ModifyArgs,
    ModifyVariable,
    ModifyConstant,
    Local,
    Share,
    Cancellable,
    Definition,
    Definitions,
    Expression,
    Expressions,
}
impl MixinExtrasAnnotation {
    pub const ALL: [Self; 19] = [
        Self::ModifyExpressionValue,
        Self::ModifyReturnValue,
        Self::ModifyReceiver,
        Self::WrapOperation,
        Self::WrapWithCondition,
        Self::WrapMethod,
        Self::Inject,
        Self::Redirect,
        Self::ModifyArg,
        Self::ModifyArgs,
        Self::ModifyVariable,
        Self::ModifyConstant,
        Self::Local,
        Self::Share,
        Self::Cancellable,
        Self::Definition,
        Self::Definitions,
        Self::Expression,
        Self::Expressions,
    ];
    pub fn simple_name(self) -> &'static str {
        match self {
            Self::ModifyExpressionValue => "ModifyExpressionValue",
            Self::ModifyReturnValue => "ModifyReturnValue",
            Self::ModifyReceiver => "ModifyReceiver",
            Self::WrapOperation => "WrapOperation",
            Self::WrapWithCondition => "WrapWithCondition",
            Self::WrapMethod => "WrapMethod",
            Self::Inject => "Inject",
            Self::Redirect => "Redirect",
            Self::ModifyArg => "ModifyArg",
            Self::ModifyArgs => "ModifyArgs",
            Self::ModifyVariable => "ModifyVariable",
            Self::ModifyConstant => "ModifyConstant",
            Self::Local => "Local",
            Self::Share => "Share",
            Self::Cancellable => "Cancellable",
            Self::Definition => "Definition",
            Self::Definitions => "Definitions",
            Self::Expression => "Expression",
            Self::Expressions => "Expressions",
        }
    }
    pub fn from_simple_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.simple_name() == name)
    }
    pub fn from_mixin_annotation(a: MixinAnnotation) -> Option<Self> {
        a.is_injector()
            .then(|| Self::from_simple_name(a.simple_name()))
            .flatten()
    }
    pub fn is_injector(self) -> bool {
        matches!(
            self,
            Self::ModifyExpressionValue
                | Self::ModifyReturnValue
                | Self::ModifyReceiver
                | Self::WrapOperation
                | Self::WrapWithCondition
                | Self::WrapMethod
        )
    }
    pub fn is_sugar_handler_injector(self) -> bool {
        self.is_injector()
            || matches!(
                self,
                Self::Inject
                    | Self::Redirect
                    | Self::ModifyArg
                    | Self::ModifyArgs
                    | Self::ModifyVariable
                    | Self::ModifyConstant
            )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinExtrasAnnotationSite {
    pub annotation: MixinExtrasAnnotation,
    pub method_attribute: String,
    pub at_value: Option<String>,
    pub at_target: Option<String>,
    pub at_args: Vec<String>,
    pub at_id: Option<String>,
    pub at_ordinal: Option<i32>,
    pub at_shift: AtShiftSpec,
    pub has_constant_selector: bool,
    pub annotation_range: McTextRange,
    pub handler_method: Option<HandlerMethodDeclaration>,
    pub expand_zero_conditions: BTreeSet<i32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionAnnotationParseIssue {
    pub attribute: String,
    pub raw_value: String,
    pub message: String,
    pub body_range: Range<usize>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinExtrasDefinition {
    pub id: Option<String>,
    pub raw_method_references: Vec<String>,
    pub raw_field_references: Vec<String>,
    pub class_literal_type_names: Vec<String>,
    pub local_specs: Vec<LocalSugarSpec>,
    pub remap: Option<bool>,
    pub body_length: usize,
    pub parse_issues: Vec<DefinitionAnnotationParseIssue>,
    pub source_range: Option<Range<usize>>,
    pub id_content_range: Option<Range<usize>>,
    pub id_source_range: Option<Range<usize>>,
}
impl MixinExtrasDefinition {
    pub fn attach_id_content_range(mut self, range: Option<Range<usize>>) -> Self {
        self.id_content_range = range;
        self
    }
    pub fn with_parse_issues(mut self, issues: Vec<DefinitionAnnotationParseIssue>) -> Self {
        self.parse_issues = issues;
        self
    }
    pub fn with_semantic_source_range(mut self, range: Option<Range<usize>>) -> Self {
        self.source_range = range;
        self
    }
    pub fn with_id_source_range(mut self, body_start: usize) -> Self {
        self.source_range = Some(body_start..body_start + self.body_length);
        self.id_source_range = self
            .id_content_range
            .as_ref()
            .map(|r| body_start + r.start..body_start + r.end);
        self
    }
    pub fn with_id_source_range_from_handler_region(
        self,
        handler_start: usize,
        body_start: usize,
    ) -> Self {
        self.with_id_source_range(handler_start + body_start)
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinExtrasDefinitionIndex {
    pub definitions: Vec<MixinExtrasDefinition>,
}
impl MixinExtrasDefinitionIndex {
    pub fn definitions_with_id(&self, id: &str) -> Vec<&MixinExtrasDefinition> {
        self.definitions
            .iter()
            .filter(|d| d.id.as_deref() == Some(id))
            .collect()
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinExtrasExpression {
    pub id: String,
    pub values: Vec<String>,
    /// Authoritative JVM units when a value has no lossless Rust String projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values_utf16: Option<Vec<Vec<u16>>>,
}
impl MixinExtrasExpression {
    pub fn from_utf16(id: String, units: Vec<Vec<u16>>) -> Self {
        match units
            .iter()
            .map(|u| String::from_utf16(u))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(values) => Self {
                id,
                values,
                values_utf16: None,
            },
            Err(_) => Self {
                id,
                values: vec![],
                values_utf16: Some(units),
            },
        }
    }
    pub fn checked_values(&self) -> Result<Vec<String>, std::string::FromUtf16Error> {
        self.values_utf16.as_ref().map_or_else(
            || Ok(self.values.clone()),
            |units| units.iter().map(|u| String::from_utf16(u)).collect(),
        )
    }
    pub fn values_utf16(&self) -> Vec<Vec<u16>> {
        self.values_utf16.clone().unwrap_or_else(|| {
            self.values
                .iter()
                .map(|s| s.encode_utf16().collect())
                .collect()
        })
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixinExtrasExpressionIndex {
    pub expressions: Vec<MixinExtrasExpression>,
}
impl MixinExtrasExpressionIndex {
    pub fn values_for_id_utf16(&self, id: &str) -> Vec<Vec<u16>> {
        self.expressions
            .iter()
            .filter(|e| e.id == id)
            .flat_map(MixinExtrasExpression::values_utf16)
            .collect()
    }
    pub fn values_for_id(&self, id: &str) -> Vec<String> {
        self.expressions
            .iter()
            .filter(|e| e.id == id)
            .flat_map(|e| e.values.clone())
            .collect()
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionContext {
    pub expression_index: MixinExtrasExpressionIndex,
    pub definition_index: MixinExtrasDefinitionIndex,
}
impl ExpressionContext {
    pub fn values_for_at_id_utf16(&self, id: Option<&str>) -> Vec<Vec<u16>> {
        self.expression_index.values_for_id_utf16(id.unwrap_or(""))
    }
    pub fn values_for_at_id(&self, id: Option<&str>) -> Vec<String> {
        self.expression_index.values_for_id(id.unwrap_or(""))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedMixinExtrasContext {
    pub handler_range: McTextRange,
    pub context: ExpressionContext,
}
pub fn select_resolved_mixin_extras_context<'a>(
    site: &MixinExtrasAnnotationSite,
    contexts: &'a [ResolvedMixinExtrasContext],
) -> Option<&'a ResolvedMixinExtrasContext> {
    let contains = |a: McTextRange, b: McTextRange| a.start <= b.start && b.end <= a.end;
    let by_annotation: Vec<_> = contexts
        .iter()
        .filter(|c| contains(c.handler_range, site.annotation_range))
        .collect();
    if by_annotation.len() == 1 {
        return by_annotation.into_iter().next();
    }
    if !by_annotation.is_empty() {
        return None;
    }
    let handler = site.handler_method.as_ref()?.range;
    let by_handler: Vec<_> = contexts
        .iter()
        .filter(|c| {
            contains(c.handler_range, handler)
                || (c.handler_range.start < handler.end && handler.start < c.handler_range.end)
        })
        .collect();
    (by_handler.len() == 1).then(|| by_handler[0])
}

// Shared local-capture facts. The locals service owns these appended types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalCaptureCandidate {
    pub slot_index: i32,
    pub descriptor: String,
    pub name: Option<String>,
    pub is_argument: bool,
    pub ordinal: Option<i32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalCaptureSnapshot {
    pub instruction_occurrence_index: i32,
    pub candidates: Vec<LocalCaptureCandidate>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalDiscriminatorResolution {
    Resolved(LocalCaptureCandidate),
    NotFound,
    Ambiguous(Vec<LocalCaptureCandidate>),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalCaptureError {
    MissingClassBytes,
    EmptyClassBytes,
    CorruptClassBytes {
        detail: String,
    },
    MethodNotFound {
        method_name: String,
        method_descriptor: String,
    },
    Analysis {
        detail: String,
    },
    InvalidInstructionIndex {
        instruction_occurrence_index: i32,
        instruction_count: i32,
    },
}
impl LocalCaptureError {
    pub fn message(&self) -> String {
        match self{
        Self::MissingClassBytes=>"class bytes missing".into(),
        Self::EmptyClassBytes=>"class bytes are empty".into(),
        Self::CorruptClassBytes{detail}=>format!("class bytes corrupt: {detail}"),
        Self::MethodNotFound{method_name,method_descriptor}=>format!("method not found: {method_name}{method_descriptor}"),
        Self::Analysis{detail}=>format!("bytecode analysis failed: {detail}"),
        Self::InvalidInstructionIndex{instruction_occurrence_index,instruction_count}=>format!("invalid instruction occurrence index {instruction_occurrence_index} (instruction count={instruction_count})"),
    }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalCaptureResult {
    Success {
        snapshots: Vec<LocalCaptureSnapshot>,
    },
    Failure {
        error: LocalCaptureError,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalCaptureValidationPoint {
    pub owner: String,
    pub target_method: crate::MethodIndexEntry,
    pub site: MixinExtrasAnnotationSite,
    pub expression_instruction_indices: Option<BTreeSet<i32>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalCaptureValidationResult {
    Resolved {
        point: LocalCaptureValidationPoint,
        snapshots: Vec<LocalCaptureSnapshot>,
        candidates: Vec<LocalCaptureCandidate>,
    },
    NotFound {
        point: LocalCaptureValidationPoint,
        snapshots: Vec<LocalCaptureSnapshot>,
    },
    Ambiguous {
        point: LocalCaptureValidationPoint,
        snapshots: Vec<LocalCaptureSnapshot>,
    },
    Unavailable {
        point: LocalCaptureValidationPoint,
        snapshots: Vec<LocalCaptureSnapshot>,
    },
}
impl LocalCaptureValidationResult {
    pub fn snapshots(&self) -> &[LocalCaptureSnapshot] {
        match self {
            Self::Resolved { snapshots, .. }
            | Self::NotFound { snapshots, .. }
            | Self::Ambiguous { snapshots, .. }
            | Self::Unavailable { snapshots, .. } => snapshots,
        }
    }
    pub fn point(&self) -> &LocalCaptureValidationPoint {
        match self {
            Self::Resolved { point, .. }
            | Self::NotFound { point, .. }
            | Self::Ambiguous { point, .. }
            | Self::Unavailable { point, .. } => point,
        }
    }
}
