//! Separate semantic-v1 child. The host owns parsing, identities, member solving and publication.
pub mod commands;
pub mod completion_lines;
pub mod completion_transport;
pub mod host_facts;
mod policy;
pub mod provider_commands;
mod server;
pub mod transport;
pub mod wire;
pub use server::run;
