#![deny(dead_code)]

//! Root semantic agent hook runtime for provider manifests and project activations.

#[cfg(feature = "compiler")]
pub mod aot_compiler;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub mod aot_evaluator;
#[cfg(feature = "evaluator")]
mod aot_evaluator_cli;
#[cfg(feature = "evaluator")]
mod aot_session_route;
#[cfg(feature = "evaluator")]
mod hook_binary;

#[cfg(feature = "evaluator")]
pub use aot_evaluator_cli::main_entry as run_aot_evaluator_cli;
#[cfg(feature = "evaluator")]
pub use aot_evaluator_cli::{
    evaluate_payload_from_embedded, payload_has_process_no_agent_assignment,
};
#[cfg(feature = "evaluator")]
pub use aot_session_route::{
    AotHookSessionRouteReceipt, publish_aot_hook_session_route, read_aot_hook_session_route,
};
#[cfg(feature = "evaluator")]
pub use hook_binary::run_from_env as run_hook_binary_from_env;

#[cfg(feature = "compiler")]
mod active_artifact_receipt;
#[cfg(feature = "compiler")]
pub use active_artifact_receipt::{
    ActiveAspArtifactReconciliation, rebind_active_asp_binary_receipt_if_present,
};
#[cfg(feature = "compiler")]
mod classifier;
#[cfg(feature = "compiler")]
mod codex_config;
#[cfg(feature = "compiler")]
pub use codex_config::codex_hook_block_with_binary;
#[cfg(feature = "compiler")]
mod codex_global_config;
#[cfg(feature = "compiler")]
pub use codex_global_config::{
    CodexGlobalHookTrustStatus, codex_global_hook_binary_path, codex_global_hook_block_with_binary,
    codex_global_hook_config_present, codex_global_hook_trust_state_status,
    merge_codex_global_hook_trust_config, remove_codex_global_hook_trust_config,
};
#[cfg(feature = "compiler")]
mod action_ir;
#[cfg(feature = "compiler")]
mod codex_project_trust;
#[cfg(feature = "compiler")]
mod codex_trust;
#[cfg(feature = "compiler")]
mod command;
#[cfg(feature = "compiler")]
mod execution_failure;
#[cfg(feature = "compiler")]
pub use execution_failure::{
    HookExecutionFailure, HookExecutionFailureKind, HookExecutionPhase,
    render_codex_execution_failure,
};
#[cfg(feature = "compiler")]
mod publication_identity;
#[cfg(feature = "compiler")]
pub use command::semantic_shell_tokens;
#[cfg(feature = "compiler")]
pub use publication_identity::{is_generation_bound_hook_deny, is_typed_hook_deny};
#[cfg(feature = "compiler")]
#[cfg(feature = "compiler")]
pub use tool_action::{codex_tool_event_requires_policy_evaluation, direct_source_read_paths};
#[cfg(feature = "compiler")]
pub use tool_action_host_binding::bind_plugin_host_matcher;
#[cfg(feature = "compiler")]
mod event_replay;
#[cfg(feature = "compiler")]
mod event_state;
#[cfg(feature = "compiler")]
mod event_state_subagent_model_drift;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::{
    ReasoningAssessment, ReasoningEvidence, ReasoningEvidenceSource, ReasoningEvidenceVisibility,
    ReasoningVerdict, reduce_reasoning_evidence,
};
#[cfg(feature = "compiler")]
mod hook_config;
#[cfg(feature = "compiler")]
mod hook_config_agent_org;
#[cfg(feature = "compiler")]
mod hook_config_global;
#[cfg(feature = "compiler")]
mod hook_recovery_admission;
#[cfg(feature = "compiler")]
mod hook_recovery_prompt;
#[cfg(feature = "compiler")]
pub mod host_native_handoff;
#[cfg(feature = "compiler")]
pub use hook_recovery_admission::canonical_recovery_admission;
#[cfg(feature = "compiler")]
mod hook_workspace_candidate;
#[cfg(feature = "compiler")]
pub use hook_workspace_candidate::{hook_workspace_candidate, normalize_workspace_path};
#[cfg(feature = "compiler")]
mod match_policy_conformance;
#[cfg(feature = "compiler")]
pub mod policy_testing;
#[cfg(feature = "compiler")]
mod protocol;
#[cfg(feature = "compiler")]
mod protocol_activation;
#[cfg(feature = "compiler")]
mod shell_read_decision_shard;
#[cfg(feature = "compiler")]
mod structured_projection_decision_shard;
#[cfg(feature = "compiler")]
pub use protocol_activation::digest::provider_execution_command_digest;
#[cfg(feature = "compiler")]
pub use protocol_activation::protocol_activation_manifest::{
    ProviderDevelopmentArtifactDomain, ProviderDevelopmentDescriptor,
};
#[cfg(feature = "compiler")]
mod provider_install_artifact;
#[cfg(feature = "compiler")]
pub use provider_install_artifact::installed_provider_artifact_digest;
#[cfg(feature = "compiler")]
mod provider_manifest;
#[cfg(feature = "compiler")]
mod provider_registry;

#[cfg(any(feature = "compiler", feature = "evaluator"))]
mod reader_probe;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub use reader_probe::{
    ReaderProbeAccess, ReaderProbeObservation, bind_reader_probe_observation,
    diagnose_reader_probe, diagnose_reader_probe_with_state_home,
};
#[cfg(all(any(feature = "compiler", feature = "evaluator"), target_os = "macos"))]
pub use reader_probe::{materialize_reader_probe_fixture, reader_probe_fixture_bytes};

#[cfg(feature = "compiler")]
pub use provider_registry::ProviderDevelopmentRegistration;
#[cfg(feature = "compiler")]
pub use provider_registry::registered_language_ids;
#[cfg(feature = "compiler")]
pub use provider_registry::{materialize_provider_routes, semantic_registry_digest};
#[cfg(feature = "compiler")]
mod execute_rule_facts;
#[cfg(feature = "compiler")]
mod source_selector;
#[cfg(feature = "compiler")]
mod tool_action;
#[cfg(feature = "compiler")]
mod tool_action_host_binding;

#[cfg(feature = "compiler")]
pub use crate::active_artifact_receipt::{
    ActiveAspArtifactMaterialization, active_asp_artifact_receipt_path,
    materialize_active_asp_artifact_receipt, verify_active_asp_artifact_receipt,
};
#[cfg(feature = "compiler")]
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
#[cfg(feature = "compiler")]
pub use codex_config::{
    CodexUserTrustStatus, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, claude_hook_block, codex_hook_block,
    codex_user_trust_state_status, default_claude_settings_path, install_codex_user_trust_state,
    merge_claude_settings, merge_codex_config, remove_codex_managed_hook_config,
    validate_claude_settings_json, validate_codex_config_toml,
};
#[cfg(feature = "compiler")]
pub use codex_project_trust::install_codex_user_project_trust;
#[cfg(feature = "compiler")]
pub use dev_context::{ActiveContextRecord, record_active_context};
#[cfg(feature = "compiler")]
pub use event_state::{
    HookSessionAgentRoute, append_hook_event_state, apply_repeated_deny_replay,
    has_recorded_subagent_context, latest_hook_session_agent_route,
    latest_hook_session_agent_route_for_root,
    latest_hook_session_agent_route_for_root_matching_rules, remove_incompatible_hook_event_state,
    try_append_hook_event_state,
};
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::{
    SubagentModelDriftObservation, SubagentProfileDriftObservation,
    SubagentRuntimeDriftObservation, SubagentRuntimeRebindObservation,
    SubagentRuntimeRebindVerifiedObservation, UnmanagedSubagentStartObservation,
    latest_subagent_model_drift, latest_subagent_profile_drift, latest_subagent_runtime_drift,
    latest_subagent_runtime_rebind_observation, latest_subagent_runtime_rebind_verified,
    latest_unmanaged_subagent_start,
};
#[cfg(feature = "compiler")]
pub use hook_config::{
    ClientHookConfig, DurableHookConfigArtifact, MaterializedDecisionShards,
    default_client_config_path, default_client_config_projection_digest,
    default_client_config_template, hook_runtime_artifact_fingerprint, load_client_config,
    load_client_config_for_matcher_publication, load_client_config_for_project,
    load_client_config_for_project_with_executable_capabilities,
    load_client_config_overlay_for_project, load_embedded_client_config_for_project,
};
#[cfg(feature = "compiler")]
pub(crate) use hook_config_agent_org::{
    AgentOrgArtifactsArchiveWarning, AgentOrgArtifactsRecovery, CompiledAgentOrgArtifactsConfig,
};
#[cfg(feature = "compiler")]
pub use hook_config_global::default_global_client_config_path;
#[cfg(feature = "compiler")]
pub use match_policy_conformance::validate_match_policy_rule_coverage;
#[cfg(feature = "compiler")]
pub use protocol::{
    ActionPolicy, AgentHookError, CANONICAL_SCHEMA_AUTHORITY, CommandTemplate, DecisionKind,
    DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_ACTIVATION_SCHEMA_ID,
    HOOK_ACTIVATION_SCHEMA_VERSION, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, HookPolicy, HookRoutes,
    PROVIDER_MANIFEST_SCHEMA_ID, PROVIDER_MANIFEST_SCHEMA_VERSION, ReasonKind, StdinMode,
    parse_payload, render_codex_permission_request, render_codex_pre_tool_deny,
    render_platform_response, subagent_deny_message,
};
#[cfg(feature = "compiler")]
pub use protocol_activation::digest::provider_manifest_digest;
#[cfg(feature = "compiler")]
pub use protocol_activation::protocol_activation_manifest::{
    ActivatedProvider, ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy,
    HookActivation, HookProviderProjection, HookRuntime, ProviderExecution, ProviderManifest,
    ProviderProjectResolutionDescriptor, ProviderQueryPackDescriptor, ProviderQueryPackTermRole,
    ProviderRuntimeContractDescriptor, ProviderRuntimeContractOperationDescriptor,
    ProviderRuntimeContractTransport, ProviderSearchCapabilities, ProviderSemanticFactsDescriptor,
    ProviderSemanticFactsIntentAxis,
};
#[cfg(feature = "compiler")]
pub use provider_manifest::project_agent_config_path;
#[cfg(feature = "compiler")]
pub use shell_read_decision_shard::CommandDecisionShard;
#[cfg(feature = "compiler")]
pub(crate) use source_selector::collect_source_selector_matches;
#[cfg(feature = "compiler")]
pub use structured_projection_decision_shard::StructuredProjectionDecisionShard;
#[cfg(feature = "compiler")]
pub use tool_action::workspace_mutation_paths;
#[cfg(feature = "compiler")]
pub(crate) use tool_action::{
    OperationIntent, ToolAction, collect_tool_actions, payload_string, subject_for_action,
};
#[cfg(feature = "compiler")]
mod dev_context;
#[cfg(all(feature = "compiler", test))]
extern crate self as agent_semantic_hook;

#[cfg(all(feature = "compiler", test))]
#[path = "../tests/unit/tool_action_functions_exec.rs"]
mod tool_action_functions_exec;

#[cfg(all(feature = "compiler", test))]
#[path = "../tests/unit/tool_action_workspace_mutation.rs"]
mod tool_action_workspace_mutation;
#[cfg(feature = "compiler")]
pub use crate::provider_registry::{
    RegisteredProviderKind, registered_provider_id, registered_provider_kind,
    registered_provider_method_invocation, registered_provider_projection_operation,
};
#[cfg(feature = "compiler")]
use agent_semantic_shell_parser as shell_parser;
#[cfg(feature = "compiler")]
mod agent_dispatch_message;
