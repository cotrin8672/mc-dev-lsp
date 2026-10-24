//! Legacy command runtime. Its host boundary is intentionally separate from semantic-v1.
pub mod cache;
mod completion;
mod delegate;
pub mod host;
mod runtime;
pub(crate) use completion::provisional_response;
pub use completion::{completion_from_core, completion_options};
pub use delegate::*;
pub use host::*;
pub use runtime::{CommandDispatcher, CommandEnvelope};
