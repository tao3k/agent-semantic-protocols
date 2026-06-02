//! Root semantic agent hook runtime for language profile descriptors.

mod classifier;
mod cli;
mod codex_config;
mod command;
mod event_state;
mod protocol;
mod tool_action;

pub use classifier::classify_hook;
pub use cli::run_cli_from_env;
pub use protocol::{
    AgentHookError, CommandTemplate, DecisionKind, DecisionRoute, DecisionRouteKind,
    DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID,
    HOOK_PROTOCOL_VERSION, HookCommands, HookDecision, HookPolicy, LanguageProfile,
    PROFILE_REGISTRY_SCHEMA_ID, PROFILE_REGISTRY_SCHEMA_VERSION, ProfileRegistry, ReasonKind,
    StdinMode, merge_profile_registries, parse_payload, parse_profiles, render_platform_response,
};
