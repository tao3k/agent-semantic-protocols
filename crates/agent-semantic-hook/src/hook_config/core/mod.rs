//! Coordinates hook config compilation and matching through typed child owners.

mod implementation;

#[doc = "Compiles config match primitives for this owner."]
mod compile;
#[doc = "Owns compiled matcher value types for this owner."]
mod match_types;
#[doc = "Matches activated ASP command capabilities."]
mod registered_asp;
#[doc = "Matches structured projection contracts."]
mod structured_projection;

pub use implementation::ClientHookConfig;
pub(super) use implementation::compile_config;
