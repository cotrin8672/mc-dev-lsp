use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchContextType {
    Custom,
    Inject,
    ModifyArg,
    ModifyArgs,
    ModifyConstant,
    ModifyExpressionValue,
    ModifyReceiver,
    ModifyReturnValue,
    ModifyVariable,
    Redirect,
    Slice,
    WrapOperation,
    WrapWithCondition,
}
impl MatchContextType {
    pub fn as_official_name(self) -> &'static str {
        match self {
            Self::Custom => "CUSTOM",
            Self::Inject => "INJECT",
            Self::ModifyArg => "MODIFY_ARG",
            Self::ModifyArgs => "MODIFY_ARGS",
            Self::ModifyConstant => "MODIFY_CONSTANT",
            Self::ModifyExpressionValue => "MODIFY_EXPRESSION_VALUE",
            Self::ModifyReceiver => "MODIFY_RECEIVER",
            Self::ModifyReturnValue => "MODIFY_RETURN_VALUE",
            Self::ModifyVariable => "MODIFY_VARIABLE",
            Self::Redirect => "REDIRECT",
            Self::Slice => "SLICE",
            Self::WrapOperation => "WRAP_OPERATION",
            Self::WrapWithCondition => "WRAP_WITH_CONDITION",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeConstraint {
    Exact(String),
    IntLike,
}
impl TypeConstraint {
    pub fn exact(value: impl Into<String>) -> Self {
        Self::Exact(value.into())
    }
    pub fn descriptor(&self) -> &str {
        match self {
            Self::Exact(value) => value,
            Self::IntLike => "Lint-like;",
        }
    }
    pub fn size(&self) -> usize {
        if matches!(self.descriptor(), "J" | "D") {
            2
        } else {
            1
        }
    }
    pub fn matches(&self, descriptor: &str) -> bool {
        self.descriptor() == descriptor || (*self == Self::IntLike && is_int_like(descriptor))
    }
}
pub(crate) fn is_int_like(value: &str) -> bool {
    matches!(value, "I" | "Z" | "B" | "S" | "C" | "Lint-like;")
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DecorationKey {
    SimpleExpressionType,
    SimpleOperationArgs,
    SimpleOperationParamNames,
    SimpleOperationReturnType,
    IsStringConcatExpression,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorationValue {
    TypeConstraint(TypeConstraint),
    TypeConstraints(Vec<TypeConstraint>),
    ParamNames(Vec<String>),
    Flag(bool),
}
pub type Decorations = BTreeMap<DecorationKey, DecorationValue>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstructionMetadata {
    MethodInvocation {
        owner_internal_name: String,
        name: String,
        descriptor: String,
        is_interface: bool,
    },
    FieldAccess {
        owner_internal_name: String,
        name: String,
        descriptor: String,
    },
    TypeOperation {
        type_internal_name: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionMatch {
    pub expression_index: usize,
    pub expression_start_offset: usize,
    pub original_instruction_opcode: u8,
    pub original_instruction_index: usize,
    pub captured_type: TypeConstraint,
    pub decorations: Decorations,
    pub instruction_metadata: Option<InstructionMetadata>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchResult {
    Available(Vec<ExpressionMatch>),
    Unavailable {
        reason: String,
        cause: Option<String>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MemberKind {
    Field,
    Method,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberCandidate {
    pub kind: MemberKind,
    pub owner_internal_name: String,
    pub name: String,
    pub descriptor: String,
    pub is_interface: bool,
    pub original_instruction_opcode: u8,
    pub original_instruction_index: usize,
    pub decorations: Decorations,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberCompletionResult {
    Available(Vec<MemberCandidate>),
    Unavailable {
        reason: String,
        cause: Option<String>,
    },
}
#[derive(Debug, Clone)]
pub struct Cancellation(pub Arc<dyn std::error::Error + Send + Sync>);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationPhase {
    LocalAnalysis,
    FlowInterpretation,
    Matching,
    MatchFailureRecheck,
}
impl std::fmt::Display for Cancellation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Cancellation {}
pub trait CancellationChecker: Send + Sync {
    fn check_cancelled(&self) -> Result<(), Cancellation>;
    fn check_phase(&self, _phase: CancellationPhase) -> Result<(), Cancellation> {
        self.check_cancelled()
    }
}
impl<F: Fn() -> Result<(), Cancellation> + Send + Sync> CancellationChecker for F {
    fn check_cancelled(&self) -> Result<(), Cancellation> {
        self()
    }
}
pub struct NoCancellation;
impl CancellationChecker for NoCancellation {
    fn check_cancelled(&self) -> Result<(), Cancellation> {
        Ok(())
    }
}
pub const NO_CANCELLATION: NoCancellation = NoCancellation;
pub type CommonSuperClassResolver<'a> = dyn Fn(&str, &str) -> Option<String> + Send + Sync + 'a;
pub struct MatchContext<'a> {
    pub context_type: MatchContextType,
    pub identifier_pool: &'a super::IdentifierPool,
    pub allow_incomplete_list_inputs: bool,
    pub common_super_class: &'a CommonSuperClassResolver<'a>,
    pub cancellation: &'a dyn CancellationChecker,
}
#[derive(Debug)]
pub(crate) enum EngineError {
    Unavailable(String),
    Cancelled(Cancellation),
}
impl From<noak::error::DecodeError> for EngineError {
    fn from(value: noak::error::DecodeError) -> Self {
        Self::Unavailable(format!("Failed to parse class bytecode: {value}"))
    }
}
impl From<Cancellation> for EngineError {
    fn from(value: Cancellation) -> Self {
        Self::Cancelled(value)
    }
}
impl From<crate::descriptor::DescriptorParseError> for EngineError {
    fn from(value: crate::descriptor::DescriptorParseError) -> Self {
        Self::Unavailable(value.to_string())
    }
}
pub(crate) type EngineResult<T> = Result<T, EngineError>;
