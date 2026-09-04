//! Coordinates hook config compilation and matching through typed child owners.

mod implementation;

#[doc = "Owns ranked policy candidates before the final arbitration step."]
mod policy_candidate;

#[doc = "Compiles config match primitives for this owner."]
mod compile;
pub(in crate::hook_config::core) use compile::compile_command_contains;
pub(in crate::hook_config::core) use compile::compile_globs;
#[doc = "Owns compiled matcher value types for this owner."]
mod match_types;
#[doc = "Matches activated ASP command capabilities."]
mod registered_asp;
#[doc = "Owns compiled Host dispatch role signals."]
#[doc = "Matches structured projection contracts."]
mod structured_projection;

pub use implementation::ClientHookConfig;
pub use implementation::DurableHookConfigArtifact;
pub use implementation::MaterializedDecisionShards;
pub(super) use implementation::compile_config;
pub(super) use implementation::compile_config_with_executable_capabilities;
pub(crate) use policy_candidate::HookPolicyCandidate;
