//! Root semantic agent hook runtime for provider manifests and project activations.

mod activation_store;
mod cache_paths;
mod classifier;
mod codex_config;
mod command;
mod event_state;
mod hook_config;
mod profile_registry;
mod protocol;
mod protocol_activation;
mod provider_manifest;
pub mod source_access;
mod source_dump_range;
mod source_selector;
mod tool_action;

pub use crate::activation_store::{
    default_activation_path, discover_activation_path, load_activation, load_or_sync_activation,
    parse_hook_activation, write_activation,
};
pub use cache_paths::{project_hook_cache_dir, project_hook_state_dir};
pub use classifier::{HookClassificationRequest, classify_hook, classify_hook_with_config};
pub use codex_config::{
    CodexUserTrustStatus, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, claude_hook_block, codex_hook_block,
    codex_user_trust_state_status, default_claude_settings_path, install_codex_user_trust_state,
    merge_claude_settings, merge_codex_config, validate_claude_settings_json,
    validate_codex_config_toml,
};
pub use dev_context::{ActiveContextRecord, record_active_context};
pub use event_state::{append_hook_event_state, remove_incompatible_hook_event_state};
pub use hook_config::{
    ClientHookConfig, default_client_config_path, default_client_config_template,
    load_client_config,
};
pub use profile_registry::{remove_legacy_codex_hook_cache_files, write_profile_registry};
pub use protocol::{
    ActionPolicy, AgentHookError, CommandTemplate, DecisionKind, DecisionRoute, DecisionRouteKind,
    DecisionSubject, HOOK_ACTIVATION_SCHEMA_ID, HOOK_ACTIVATION_SCHEMA_VERSION,
    HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION,
    HookDecision, HookPolicy, HookRoutes, PROVIDER_MANIFEST_SCHEMA_ID,
    PROVIDER_MANIFEST_SCHEMA_VERSION, ReasonKind, StdinMode, parse_payload,
    render_platform_response,
};
pub(crate) use protocol_activation::SourceSelectorKind;
pub use protocol_activation::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime, ManifestSourceDefaults, ProviderManifest, parse_activation,
    provider_manifest_digest,
};
pub use provider_manifest::{build_default_activation, builtin_provider_manifests};
pub(crate) use source_selector::{SourceSelectorMatch, collect_source_selector_matches};
pub(crate) use tool_action::{
    OperationIntent, ToolAction, collect_tool_actions, payload_string, subject_for_action,
};
mod dev_context;
