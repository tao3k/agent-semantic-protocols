//! Coordinates hook config compilation and matching through typed child owners.

mod implementation;

#[doc = "Owns ranked policy candidates before the final arbitration step."]
mod policy_candidate;

#[doc = "Compiles config match primitives for this owner."]
mod compile;
#[doc = "Resolves configured resident targets from compiled rules."]
mod configured_resident;
#[doc = "Owns compiled matcher value types for this owner."]
mod match_types;
#[doc = "Matches activated ASP command capabilities."]
mod registered_asp;
#[doc = "Owns configured resident target identities."]
mod resident_target;
#[doc = "Matches structured projection contracts."]
mod structured_projection;

pub(super) use implementation::compile_config;
pub use implementation::{ClientHookConfig, DurableHookConfigArtifact};
pub(crate) use policy_candidate::HookPolicyCandidate;
pub use resident_target::ConfiguredResidentTarget;
