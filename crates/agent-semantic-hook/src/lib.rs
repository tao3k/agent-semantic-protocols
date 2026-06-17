#![deny(dead_code)]

//! Root semantic agent hook runtime for provider manifests and project activations.

mod activation_store;
mod classifier;
mod codex_config;
mod codex_project_trust;
mod codex_trust;
mod command;
mod event_state;
mod executable;
mod hook_config;
mod profile_registry;
mod protocol;
mod protocol_activation;
mod provider_manifest;
mod provider_registry;
mod runtime_profile;
pub mod source_access;
mod source_dump_range;
mod source_selector;
mod tool_action;

pub use crate::activation_store::{
    DefaultActivationSync, default_activation_path, discover_activation_path, load_activation,
    load_or_refresh_default_activation, load_or_sync_activation, parse_hook_activation,
    write_activation,
};
pub use classifier::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, HookClassificationRequest, classify_hook,
    classify_hook_with_config, default_hook_trigger_prompt_message, hook_trigger_prompt_document,
    merge_hook_trigger_prompt_document, render_hook_trigger_prompt_document,
};
pub use codex_config::{
    CodexUserTrustStatus, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, claude_hook_block, codex_hook_block,
    codex_user_trust_state_status, default_claude_settings_path, install_codex_user_trust_state,
    merge_claude_settings, merge_codex_asp_explorer_role_config, merge_codex_config,
    remove_codex_managed_hook_blocks, validate_claude_settings_json, validate_codex_config_toml,
};
pub use codex_project_trust::install_codex_user_project_trust;
pub use dev_context::{ActiveContextRecord, record_active_context};
pub use event_state::{
    append_hook_event_state, apply_repeated_deny_replay, has_recorded_subagent_context,
    remove_incompatible_hook_event_state,
};
pub use hook_config::{
    ClientHookConfig, default_client_config_path, default_client_config_template,
    default_client_config_template_for_source_extensions, load_client_config,
};
pub use profile_registry::remove_retired_codex_hook_cache_files;
pub use protocol::{
    ActionPolicy, AgentHookError, CommandTemplate, DecisionKind, DecisionRoute, DecisionRouteKind,
    DecisionSubject, HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION,
    HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION,
    HookDecision, HookPolicy, HookRoutes, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION, ReasonKind, StdinMode, parse_payload,
    render_platform_response, subagent_deny_message,
};
pub(crate) use protocol_activation::SourceSelectorKind;
pub use protocol_activation::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime, ManifestSourceDefaults, ProviderExecution, ProviderManifest,
    ProviderSearchCapabilities, parse_activation, provider_manifest_digest,
};
pub use provider_manifest::{
    ProviderCommandSelection, build_default_activation, builtin_provider_manifests,
    provider_command_selections,
};
pub use runtime_profile::{
    RUNTIME_PROFILES_PROTOCOL_ID, RUNTIME_PROFILES_PROTOCOL_VERSION, RUNTIME_PROFILES_SCHEMA_ID,
    RUNTIME_PROFILES_SCHEMA_VERSION, RuntimeProfiles, RuntimeProfilesGeneratedBy,
    RuntimeProviderHealth, RuntimeProviderHealthStatus, RuntimeProviderProfile,
    runtime_profile_command_argv, runtime_profile_invocation, runtime_profiles_for_activation,
    runtime_profiles_for_runtime, runtime_project_root_for_activation,
};
pub(crate) use source_selector::{SourceSelectorMatch, collect_source_selector_matches};
pub(crate) use tool_action::{
    OperationIntent, ToolAction, collect_tool_actions, payload_string, subject_for_action,
};
mod dev_context;
