#![deny(dead_code)]

//! Root semantic agent hook runtime for provider manifests and project activations.

mod activation_store;
pub use activation_store::registered_language_runtime;
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
mod codex_project_trust;
mod codex_trust;
mod action_ir;
mod command;
pub use command::semantic_shell_tokens;
pub use tool_action::{codex_tool_event_requires_policy_evaluation, direct_source_read_paths};
mod event_replay;
mod event_state;
mod event_state_subagent_model_drift;
pub use event_state_subagent_model_drift::{
    ReasoningAssessment, ReasoningEvidence, ReasoningEvidenceSource, ReasoningEvidenceVisibility,
    ReasoningVerdict, reduce_reasoning_evidence,
};
mod executable;
pub(crate) use executable::resolve_executable_with_status;
mod hook_config;
mod hook_config_agent_org;
mod hook_config_global;
mod hook_policy_kernel;
mod hook_recovery_admission;
mod hook_recovery_prompt;
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
mod provider_runtime;
pub use provider_registry::registered_language_ids;
pub use provider_registry::{
    ProviderDevelopmentRegistrationV1, RegisteredProviderBinaryV1, RuntimeBinaryAdmissionDenialV1,
    RuntimeBinaryClassificationV1, RuntimeBinaryDispatchRequirementsV1,
    RuntimeBinaryIdentityBindingV1, RuntimeBinaryInvocationAuthorityV1, RuntimeBinaryProfileV1,
    SessionBindingV1, classify_runtime_executable_v1, registered_provider_binaries_v1,
    registered_provider_binary_v1, registered_provider_development_v1,
    registered_provider_matches_candidate_paths, runtime_binary_dispatch_requirements_v1,
};
pub use provider_registry::{
    materialize_provider_routes, registered_language_descriptor_digest,
    registered_query_pack_digest, schema_registry_provider_manifests, semantic_registry_digest,
};
mod execute_rule_facts;
mod runtime_profile;
pub mod source_access;
mod source_selector;
mod tool_action;

pub use crate::activation_store::{
    ActivationAdmissionDecision, ActivationAdmissionGates, ActivationAdmissionReason,
    ActivationAdmissionReceipt, DefaultActivationSync, default_activation_path,
    discover_activation_path, load_activation, load_or_refresh_default_activation,
    load_or_refresh_default_activation_with_state_home,
    load_or_refresh_default_activation_with_state_home_and_binary, load_or_sync_activation,
    load_or_sync_activation_with_state_home, parse_hook_activation, write_activation,
};
pub use crate::active_artifact_receipt::{
    ActiveAspArtifactMaterialization, active_asp_artifact_receipt_path,
    materialize_active_asp_artifact_receipt,
    verify_active_asp_artifact_receipt,
};
pub use classifier::{
    DirectReadSourceKey, HOOK_TRIGGER_PROMPT_FILE_NAME, HookClassificationRequest, HookMatcherKeys,
    ShellCommandKey, ShellReadSourceKey, classify_hook, classify_hook_with_config,
    default_hook_trigger_prompt_message, direct_read_source_extension, direct_read_source_key,
    hook_matcher_keys, hook_trigger_prompt_document,
    materialize_hook_trigger_prompt_agent_flow_for_client, merge_hook_trigger_prompt_document,
    rebind_command_decision_to_payload, rebind_command_decision_to_payload_with_keys,
    render_hook_trigger_prompt_document, runtime_binary_policy_decision_v1, shell_command_key,
    shell_command_keys, shell_read_source_key, shell_read_source_keys,
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
    remove_incompatible_hook_event_state, try_append_hook_event_state,
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
    AspSessionPolicy, ClientHookConfig, ConfiguredResidentTarget, DurableHookConfigArtifact,
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
pub(crate) use hook_recovery_prompt::CompiledRecoveryPromptConfig;
pub use match_policy_conformance::{
    MatchPolicyConformanceReport, evaluate_match_policy_conformance,
    validate_match_policy_rule_coverage,
};
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
    HookActivation, HookRuntime, ProviderExecution, ProviderManifest,
    ProviderProjectResolutionDescriptor, ProviderQueryPackDescriptor, ProviderQueryPackTermRole,
    ProviderRuntimeContractDescriptor, ProviderRuntimeContractOperationDescriptor,
    ProviderRuntimeContractTransport, ProviderSearchCapabilities, ProviderSemanticFactsDescriptor,
    ProviderSemanticFactsIntentAxis,
};
pub use protocol_activation::protocol_activation_runtime::parse_activation;
pub use provider_manifest::{
    ProviderCommandSelection, ProviderCommandSelectionScopeV1, build_default_activation,
    build_default_activation_from_selections, build_default_activation_with_state_home,
    builtin_provider_manifests, project_agent_config_path, provider_command_selections,
    provider_command_selections_for_scope, provider_command_selections_for_scope_with_state_home,
    validate_provider_manifest_contract,
};
pub use runtime_profile::{
    RUNTIME_PROFILES_PROTOCOL_ID, RUNTIME_PROFILES_PROTOCOL_VERSION, RUNTIME_PROFILES_SCHEMA_ID,
    RUNTIME_PROFILES_SCHEMA_VERSION, RuntimeProfiles, RuntimeProfilesGeneratedBy,
    RuntimeProviderHealth, RuntimeProviderHealthStatus, RuntimeProviderProfile,
    runtime_profile_command_argv, runtime_profile_invocation, runtime_profiles_for_activation,
    runtime_profiles_for_runtime, runtime_profiles_for_runtime_with_state_home,
    runtime_project_root_for_activation,
};
pub use shell_read_decision_shard::CommandDecisionShard;
pub(crate) use source_selector::{SourceSelectorMatch, collect_source_selector_matches};
pub use structured_projection_decision_shard::StructuredProjectionDecisionShard;
pub use tool_action::workspace_mutation_paths;
pub(crate) use tool_action::{
    OperationIntent, ToolAction, collect_tool_actions, payload_string, subject_for_action,
};
mod dev_context;
mod read_only_subagent;
pub use read_only_subagent::{
    CodexHookAgentId, CodexHookAgentType, ConfiguredCodexAgentName, ConfiguredResidentRole,
    HookSubagentPermissionContext, ManagedChildName, ResidentChildIdentityProof,
    ResidentChildSessionId, ResidentConfiguration, ResidentEnabled, ResidentIdentityStatus,
    ResidentLiveIdentity, ResidentRootSessionId, ResidentSandboxMode,
    classify_read_only_subagent_receipt, classify_read_only_subagent_write,
};
#[cfg(test)]
extern crate self as agent_semantic_hook;
pub use crate::provider_registry::{
    ProviderMethodArgumentValuesV1, RegisteredProviderCatalogIdentity, RegisteredProviderKind,
    registered_provider_catalog_identities, registered_provider_id_v1, registered_provider_kind,
    registered_provider_method_invocation_v1, registered_provider_method_projected_argv_v1,
    registered_provider_projection_operation,
};
use agent_semantic_shell_parser as shell_parser;
mod agent_dispatch_message;
