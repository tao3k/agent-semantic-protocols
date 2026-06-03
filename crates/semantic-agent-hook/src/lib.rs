//! Root semantic agent hook runtime for provider manifests and project activations.

mod activation_store;
mod classifier;
mod cli;
mod codex_config;
mod command;
mod event_state;
mod protocol;
mod protocol_activation;
mod provider_manifest;
mod source_dump_range;
mod source_selector;
mod tool_action;

pub use crate::activation_store::parse_hook_activation;
pub use classifier::classify_hook;
pub use cli::{run_cli_args, run_cli_from_env};
pub use protocol::{
    ActionPolicy, AgentHookError, CommandTemplate, DecisionKind, DecisionRoute, DecisionRouteKind,
    DecisionSubject, HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION,
    HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION,
    HookDecision, HookPolicy, HookRoutes, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION, ReasonKind, StdinMode, parse_payload,
    render_platform_response,
};
pub use protocol_activation::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime, ManifestSourceDefaults, ProviderManifest, parse_activation,
    provider_manifest_digest,
};
pub use provider_manifest::builtin_provider_manifests;
mod dev_context;
