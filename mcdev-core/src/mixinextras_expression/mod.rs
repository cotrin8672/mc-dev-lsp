//! MixinExtras annotation source facts and expression-backed editor services.
mod context;
mod editing;
mod members;
mod parsing;
mod support;
pub use crate::extras_model::{
    ExpressionContext, MixinExtrasDefinition, MixinExtrasDefinitionIndex, MixinExtrasExpression,
    MixinExtrasExpressionIndex,
};
pub use crate::mixin_source::{ExpressionCompletionLexer, ExpressionCompletionPosition};
pub use context::*;
pub use editing::*;
pub use members::*;
pub use parsing::*;
pub use support::*;
