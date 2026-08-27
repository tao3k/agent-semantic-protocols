#![deny(dead_code)]

//! Root semantic agent hook runtime for provider manifests and project activations.

mod active_artifact_receipt;
pub use active_artifact_receipt::{
    ActiveAspArtifactReconciliation, rebind_active_asp_binary_receipt_if_present,
};
mod classifier;
mod codex_config;
pub use codex_config::codex_hook_block_with_binary;
mod codex_global_config;
pub use codex_global_config::{
    CodexGlobalHookTrustStatus, codex_global_hook_binary_path, codex_global_hook_block_with_binary,
    codex_global_hook_config_present, codex_global_hook_trust_state_status,
    merge_codex_global_hook_trust_config, remove_codex_global_hook_trust_config,
};
mod action_ir;
mod codex_project_trust;
mod codex_trust;
mod command;
mod execution_failure;
pub use execution_failure::{HookExecutionFailure, HookExecutionFailureKind, HookExecutionPhase};
mod publication_identity;
pub use publication_identity::{is_generation_bound_hook_deny, is_typed_hook_deny};
pub mod runtime_config;
pub use command::semantic_shell_tokens;
pub use tool_action::{codex_tool_event_requires_policy_evaluation, direct_source_read_paths};
pub use tool_action_host_binding::bind_plugin_host_matcher;
mod event_replay;
mod event_state;
mod event_state_subagent_model_drift;
pub use event_state_subagent_model_drift::{
    ReasoningAssessment, ReasoningEvidence, ReasoningEvidenceSource, ReasoningEvidenceVisibility,
    ReasoningVerdict, reduce_reasoning_evidence,
};
mod hook_config;
mod hook_config_agent_org;
mod hook_config_global;
mod hook_recovery_admission;
mod hook_recovery_prompt;
pub mod host_native_handoff;
pub use hook_recovery_admission::canonical_recovery_admission;
mod hook_workspace_candidate;
pub use hook_workspace_candidate::{hook_workspace_candidate, normalize_workspace_path};
mod match_policy_conformance;
pub mod policy_testing;
mod protocol;
mod protocol_activation;
mod shell_read_decision_shard;
mod structured_projection_decision_shard;
pub use protocol_activation::digest::provider_execution_command_digest;
pub use protocol_activation::protocol_activation_manifest::{
    ProviderDevelopmentArtifactDomain, ProviderDevelopmentDescriptor,
};
mod provider_install_artifact;
pub use provider_install_artifact::installed_provider_artifact_digest;
mod provider_manifest;
mod provider_registry;
pub use provider_registry::ProviderDevelopmentRegistration;
pub use provider_registry::registered_language_ids;
pub use provider_registry::{materialize_provider_routes, semantic_registry_digest};
mod execute_rule_facts;
mod source_selector;
mod tool_action;
mod tool_action_host_binding;

pub use crate::active_artifact_receipt::{
    ActiveAspArtifactMaterialization, active_asp_artifact_receipt_path,
    materialize_active_asp_artifact_receipt, verify_active_asp_artifact_receipt,
};
pub use classifier::{
    DirectReadSourceKey, HOOK_TRIGGER_PROMPT_FILE_NAME, HookClassificationRequest, HookMatcherKeys,
    ShellCommandKey, ShellReadSourceKey, classify_hook, classify_hook_with_config,
    default_hook_trigger_prompt_message, direct_read_source_extension, direct_read_source_key,
    hook_matcher_keys, hook_trigger_prompt_document,
    materialize_hook_trigger_prompt_agent_flow_for_client, materialize_source_access_deny_message,
    merge_hook_trigger_prompt_document, rebind_command_decision_to_payload,
    rebind_command_decision_to_payload_with_keys, rebind_direct_read_decision_to_payload,
    render_hook_trigger_prompt_document, shell_command_key, shell_command_keys,
    shell_read_source_key, shell_read_source_keys,
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
    HookSessionAgentRoute, append_hook_event_state, apply_repeated_deny_replay,
    has_recorded_subagent_context, latest_hook_session_agent_route,
    latest_hook_session_agent_route_for_root, remove_incompatible_hook_event_state,
    try_append_hook_event_state,
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
    ClientHookConfig, DurableHookConfigArtifact, MaterializedDecisionShards,
    default_client_config_path, default_client_config_projection_digest,
    default_client_config_template, hook_runtime_artifact_fingerprint, load_client_config,
    load_client_config_for_matcher_publication, load_client_config_for_project,
    load_client_config_for_project_with_executable_capabilities,
    load_client_config_overlay_for_project, load_embedded_client_config_for_project,
};
pub(crate) use hook_config_agent_org::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, CompiledAgentOrgArtifactsConfig,
};
pub use hook_config_global::default_global_client_config_path;
pub use match_policy_conformance::validate_match_policy_rule_coverage;
pub use protocol::{
    ActionPolicy, AgentHookError, CANONICAL_SCHEMA_AUTHORITY, CommandTemplate, DecisionKind,
    DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_ACTIVATION_SCHEMA_ID,
    HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, HookPolicy, HookRoutes,
    PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION, ReasonKind, StdinMode,
    parse_payload, render_platform_response, subagent_deny_message,
};
pub use protocol_activation::digest::provider_manifest_digest;
pub use protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookProviderProjection, HookRuntime, ProviderExecution, ProviderManifest,
    ProviderProjectResolutionDescriptor, ProviderQueryPackDescriptor, ProviderQueryPackTermRole,
    ProviderRuntimeContractDescriptor, ProviderRuntimeContractOperationDescriptor,
    ProviderRuntimeContractTransport, ProviderSearchCapabilities, ProviderSemanticFactsDescriptor,
    ProviderSemanticFactsIntentAxis,
};
pub use provider_manifest::project_agent_config_path;
pub use shell_read_decision_shard::CommandDecisionShard;
pub(crate) use source_selector::collect_source_selector_matches;
pub use structured_projection_decision_shard::StructuredProjectionDecisionShard;
pub use tool_action::workspace_mutation_paths;
pub(crate) use tool_action::{
    OperationIntent, ToolAction, collect_tool_actions, payload_string, subject_for_action,
};
mod dev_context;
#[cfg(test)]
extern crate self as agent_semantic_hook;

#[cfg(test)]
#[path = "../tests/unit/tool_action_functions_exec.rs"]
mod tool_action_functions_exec;

#[cfg(test)]
#[path = "../tests/unit/tool_action_workspace_mutation.rs"]
mod tool_action_workspace_mutation;
pub use crate::provider_registry::{
    RegisteredProviderKind, registered_provider_id, registered_provider_kind,
    registered_provider_method_invocation, registered_provider_projection_operation,
};
use agent_semantic_shell_parser as shell_parser;
mod agent_dispatch_message;
