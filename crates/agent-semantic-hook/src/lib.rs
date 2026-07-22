#![deny(dead_code)]

//! Root semantic agent hook runtime for provider manifests and project activations.

mod activation_store;
mod active_artifact_receipt;
mod classifier;
mod codex_config;
mod codex_plugin_trust;
pub use codex_config::codex_hook_block_with_binary;
pub use codex_plugin_trust::install_codex_user_plugin_trust_state;
pub use codex_plugin_trust::{CodexPluginTrustStatus, codex_user_plugin_trust_state_status};
mod codex_global_config;
pub use codex_global_config::{
    codex_global_hook_block_with_binary, merge_codex_global_hook_trust_config,
    remove_codex_global_hook_trust_config,
};
mod codex_project_trust;
mod codex_trust;
mod command;
pub use command::semantic_shell_tokens;
mod event_replay;
mod event_state;
mod event_state_subagent_model_drift;
pub use event_state_subagent_model_drift::{
    ReasoningAssessment, ReasoningEvidence, ReasoningEvidenceSource, ReasoningEvidenceVisibility,
    ReasoningVerdict, reduce_reasoning_evidence,
};
mod executable;
mod hook_config;
mod hook_config_agent_org;
mod hook_config_global;
mod hook_recovery_prompt;
mod protocol;
mod protocol_activation;
pub use protocol_activation::digest::provider_execution_command_digest;
mod provider_manifest;
mod provider_registry;
pub use provider_registry::registered_language_ids;
pub use provider_registry::{materialize_provider_routes, semantic_registry_digest};
mod runtime_profile;
pub mod source_access;
mod source_selector;
mod tool_action;

pub use crate::activation_store::{
    DefaultActivationSync, default_activation_path, discover_activation_path, load_activation,
    load_or_refresh_default_activation, load_or_sync_activation, parse_hook_activation,
    write_activation,
};
pub use crate::active_artifact_receipt::{
    ActiveAspArtifactInput, ActiveAspArtifactMaterialization, active_asp_artifact_receipt_path,
    materialize_active_asp_artifact_receipt,
    materialize_active_asp_artifact_receipt_for_current_process,
    verify_active_asp_artifact_receipt,
};
pub use classifier::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, HookClassificationRequest, classify_hook,
    classify_hook_with_config, default_hook_trigger_prompt_message, hook_trigger_prompt_document,
    materialize_hook_trigger_prompt_agent_flow_for_client, merge_hook_trigger_prompt_document,
    render_hook_trigger_prompt_document,
};
pub use codex_config::{
    CodexUserTrustStatus, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, claude_hook_block, codex_hook_block,
    codex_user_trust_state_status, default_claude_settings_path, install_codex_user_trust_state,
    merge_claude_settings, merge_codex_config, remove_codex_managed_hook_config,
    validate_claude_settings_json, validate_codex_config_toml,
};
pub use codex_project_trust::install_codex_user_project_trust;
pub use dev_context::{ActiveContextRecord, record_active_context};
pub use event_state::{
    append_hook_event_state, apply_repeated_deny_replay, has_recorded_subagent_context,
    remove_incompatible_hook_event_state,
};
pub use event_state_subagent_model_drift::{
    SubagentModelDriftObservation, SubagentProfileDriftObservation,
    SubagentRuntimeDriftObservation, SubagentRuntimeRebindObservation,
    SubagentRuntimeRebindVerifiedObservation, UnmanagedSubagentStartObservation,
    latest_subagent_model_drift, latest_subagent_profile_drift, latest_subagent_runtime_drift,
    latest_subagent_runtime_rebind_observation, latest_subagent_runtime_rebind_verified,
    latest_unmanaged_subagent_start,
};
pub use hook_config::{
    AspSessionPolicy, ClientHookConfig, default_client_config_path, default_client_config_template,
    load_client_config, load_client_config_for_project, load_embedded_client_config_for_project,
};
pub use hook_config_global::default_global_client_config_path;
pub use protocol::{
    ActionPolicy, AgentHookError, CANONICAL_SCHEMA_AUTHORITY, CommandTemplate, DecisionKind,
    DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_ACTIVATION_SCHEMA_ID,
    HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, HookPolicy, HookRoutes,
    PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION, ReasonKind, StdinMode,
    parse_payload, render_platform_response, subagent_deny_message,
};
pub use protocol_activation::digest::provider_manifest_digest;
pub(crate) use protocol_activation::protocol_activation_manifest::SourceSelectorKind;
pub use protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookRuntime, ManifestSourceDefaults, ProviderExecution, ProviderManifest,
    ProviderQueryPackDescriptor, ProviderSearchCapabilities, ProviderSemanticFactsDescriptor,
    ProviderSemanticFactsIntentAxis,
};
pub use protocol_activation::protocol_activation_runtime::parse_activation;
pub use provider_manifest::{
    ProviderCommandSelection, build_default_activation, builtin_provider_manifests,
    project_agent_config_path, provider_command_selections, validate_provider_manifest_contract,
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
mod read_only_subagent;
pub use read_only_subagent::{
    HookSubagentPermissionContext, classify_read_only_subagent_receipt,
    classify_read_only_subagent_write,
};
#[cfg(test)]
extern crate self as agent_semantic_hook;
#[doc(hidden)]
pub use agent_semantic_command_match as command_match;
#[doc(hidden)]
pub use agent_semantic_command_match::bash as bash_command_stages;
