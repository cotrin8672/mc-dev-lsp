//! Native port of MixinExtras expressions 0.0.6. See third-party/MixinExtras-PROVENANCE.md.
mod flow;
mod matcher;
mod parser;
mod pool;
mod postprocess;
mod types;
pub use flow::{
    analyze_method, analyze_method_with_policy, decode_method, AnalysisPolicy, DecodeMethodError,
    FlowGraph, FlowNode, Instruction, InstructionOperand, JvmConstant, LocalVariable, MethodFlow,
};
pub use matcher::{
    complete_receiver_members, complete_receiver_members_utf16, current_cancellation,
    is_valid_receiver_expression, is_valid_receiver_expression_utf16, match_expression_components,
    match_expressions, match_expressions_utf16, receiver_probes, receiver_probes_utf16,
    with_cancellation,
};
pub use parser::{
    codepoint_offset_to_utf16, parse_expression, parse_expression_utf16, BinaryOperator, ExprKind,
    Expression, Name, SourceSpan, SyntaxCause, SyntaxFailure, TypeName, UnaryOperator,
};
pub use pool::*;
pub use types::*;
