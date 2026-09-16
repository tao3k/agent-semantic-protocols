#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Root semantic agent hook runtime for provider manifests and project activations.

#[cfg(feature = "compiler")]
pub mod aot_compiler;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub mod aot_evaluator;
#[cfg(feature = "evaluator")]
mod aot_evaluator_cli;
#[cfg(feature = "evaluator")]
mod hook_binary;
#[cfg(feature = "evaluator")]
mod search_playbook_pretool;
#[cfg(feature = "evaluator")]
mod search_subagent_output_contract;
#[cfg(feature = "evaluator")]
#[doc(hidden)]
pub use search_playbook_pretool::evaluate as evaluate_search_playbook_pretool;

#[cfg(feature = "compiler")]
pub use aot_evaluator_cli::AotHookEvaluationReceipt;
#[cfg(feature = "evaluator")]
pub use aot_evaluator_cli::evaluate_payload_from_serving_config;
#[cfg(feature = "compiler")]
pub use aot_evaluator_cli::evaluate_payload_with_policy_bundle;
#[cfg(feature = "compiler")]
pub use aot_evaluator_cli::evaluate_payload_with_policy_bundle_and_state_home;
#[cfg(feature = "compiler")]
pub use aot_evaluator_cli::evaluate_payload_with_policy_bundle_and_state_home_with_receipt;
#[cfg(feature = "evaluator")]
pub use aot_evaluator_cli::main_entry as run_aot_evaluator_cli;
#[cfg(feature = "evaluator")]
pub use hook_binary::run_from_env as run_hook_binary_from_env;

#[cfg(feature = "compiler")]
mod active_artifact_receipt;
#[cfg(feature = "compiler")]
pub use active_artifact_receipt::ActiveAspArtifactReconciliation;
#[cfg(feature = "compiler")]
pub use active_artifact_receipt::rebind_active_asp_binary_receipt_if_present;
#[cfg(feature = "compiler")]
mod classifier;
#[cfg(feature = "compiler")]
mod codex_config;
#[cfg(feature = "compiler")]
pub use codex_config::codex_hook_block_with_binary;
#[cfg(feature = "compiler")]
mod codex_global_config;
#[cfg(feature = "compiler")]
pub use codex_global_config::CodexGlobalHookTrustStatus;
#[cfg(feature = "compiler")]
pub use codex_global_config::codex_global_hook_binary_path;
#[cfg(feature = "compiler")]
pub use codex_global_config::codex_global_hook_block_with_binary;
#[cfg(feature = "compiler")]
pub use codex_global_config::codex_global_hook_config_present;
#[cfg(feature = "compiler")]
pub use codex_global_config::codex_global_hook_trust_state_status;
#[cfg(feature = "compiler")]
pub use codex_global_config::merge_codex_global_hook_trust_config;
#[cfg(feature = "compiler")]
pub use codex_global_config::remove_codex_global_hook_trust_config;
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
pub use execution_failure::HookExecutionFailure;
#[cfg(feature = "compiler")]
pub use execution_failure::HookExecutionFailureKind;
#[cfg(feature = "compiler")]
pub use execution_failure::HookExecutionPhase;
#[cfg(feature = "compiler")]
pub use execution_failure::render_codex_execution_failure;
#[cfg(feature = "compiler")]
mod publication_identity;
#[cfg(feature = "compiler")]
pub use command::semantic_shell_tokens;
#[cfg(feature = "compiler")]
pub use publication_identity::HOOK_RUNTIME_IDENTITY_SCHEMA_ID;
#[cfg(feature = "compiler")]
pub use publication_identity::HookRuntimeIdentityReceipt;
#[cfg(feature = "compiler")]
pub use publication_identity::hook_runtime_identity_receipt;
#[cfg(feature = "compiler")]
pub use publication_identity::is_generation_bound_hook_deny;
#[cfg(feature = "compiler")]
pub use publication_identity::is_typed_hook_deny;
#[cfg(feature = "compiler")]
#[cfg(feature = "compiler")]
pub use tool_action::codex_tool_event_requires_policy_evaluation;
#[cfg(feature = "compiler")]
#[cfg(feature = "compiler")]
pub use tool_action::direct_source_read_paths;
#[cfg(feature = "compiler")]
pub use tool_action_host_binding::bind_plugin_host_matcher;
#[cfg(feature = "compiler")]
mod event_replay;
#[cfg(feature = "compiler")]
mod event_state;
#[cfg(feature = "compiler")]
mod event_state_subagent_model_drift;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::ReasoningAssessment;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::ReasoningEvidence;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::ReasoningEvidenceSource;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::ReasoningEvidenceVisibility;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::ReasoningVerdict;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::reduce_reasoning_evidence;
#[cfg(feature = "compiler")]
mod hook_config;
#[cfg(feature = "compiler")]
mod hook_config_agent_org;
#[cfg(feature = "compiler")]
mod hook_config_global;
#[cfg(feature = "compiler")]
mod hook_recovery_admission;
#[cfg(feature = "compiler")]
#[cfg(feature = "compiler")]
pub mod host_native_handoff;
#[cfg(feature = "compiler")]
pub use hook_recovery_admission::canonical_recovery_admission;
#[cfg(feature = "compiler")]
mod hook_workspace_candidate;
#[cfg(feature = "compiler")]
pub use hook_workspace_candidate::hook_workspace_candidate;
#[cfg(feature = "compiler")]
pub use hook_workspace_candidate::normalize_workspace_path;
#[cfg(feature = "compiler")]
mod match_policy_conformance;
#[cfg(feature = "compiler")]
pub mod policy_testing;
#[cfg(feature = "compiler")]
mod protocol;
#[cfg(feature = "compiler")]
mod provider_install_artifact;
#[cfg(feature = "compiler")]
mod provider_projection;
#[cfg(feature = "compiler")]
mod provider_routing;
#[cfg(feature = "compiler")]
mod shell_read_decision_shard;
#[cfg(feature = "compiler")]
mod structured_projection_decision_shard;
#[cfg(feature = "compiler")]
pub use provider_install_artifact::installed_provider_artifact_digest;
#[cfg(feature = "compiler")]
mod provider_manifest;
#[cfg(feature = "compiler")]
mod provider_registry;

#[cfg(any(feature = "compiler", feature = "evaluator"))]
mod reader_probe;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub use reader_probe::ReaderProbeAccess;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub use reader_probe::ReaderProbeObservation;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub use reader_probe::bind_reader_probe_observation;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub use reader_probe::diagnose_reader_probe;
#[cfg(any(feature = "compiler", feature = "evaluator"))]
pub use reader_probe::diagnose_reader_probe_with_state_home;
#[cfg(all(any(feature = "compiler", feature = "evaluator"), target_os = "macos"))]
pub use reader_probe::materialize_reader_probe_fixture;
#[cfg(all(any(feature = "compiler", feature = "evaluator"), target_os = "macos"))]
pub use reader_probe::reader_probe_fixture_bytes;

#[cfg(feature = "compiler")]
pub use provider_registry::registered_language_ids;
#[cfg(feature = "compiler")]
pub use provider_registry::semantic_registry_digest;
#[cfg(feature = "compiler")]
mod execute_rule_facts;
#[cfg(feature = "compiler")]
mod source_selector;
#[cfg(feature = "compiler")]
mod tool_action;
#[cfg(feature = "compiler")]
mod tool_action_host_binding;

#[cfg(feature = "compiler")]
pub use crate::active_artifact_receipt::ActiveAspArtifactMaterialization;
#[cfg(feature = "compiler")]
pub use crate::active_artifact_receipt::active_asp_artifact_receipt_path;
#[cfg(feature = "compiler")]
pub use crate::active_artifact_receipt::materialize_active_asp_artifact_receipt;
#[cfg(feature = "compiler")]
pub use crate::active_artifact_receipt::verify_active_asp_artifact_receipt;
#[cfg(feature = "compiler")]
pub use classifier::HookClassificationRequest;
#[cfg(feature = "compiler")]
pub use classifier::ShellCommandKey;
#[cfg(feature = "compiler")]
pub use classifier::ShellReadSourceKey;
#[cfg(feature = "compiler")]
pub use classifier::classify_hook;
#[cfg(feature = "compiler")]
pub use classifier::classify_hook_with_config;
#[cfg(feature = "compiler")]
pub use classifier::materialize_source_access_deny_message;
#[cfg(feature = "compiler")]
pub use classifier::rebind_command_decision_to_payload;
#[cfg(feature = "compiler")]
pub use classifier::rebind_command_decision_to_payload_with_keys;
#[cfg(feature = "compiler")]
pub use classifier::shell_command_key;
#[cfg(feature = "compiler")]
pub use classifier::shell_command_keys;
#[cfg(feature = "compiler")]
pub use classifier::shell_read_source_key;
#[cfg(feature = "compiler")]
pub use classifier::shell_read_source_keys;
#[cfg(feature = "compiler")]
pub use codex_config::CodexUserTrustStatus;
#[cfg(feature = "compiler")]
pub use codex_config::ROOT_BLOCK_BEGIN;
#[cfg(feature = "compiler")]
pub use codex_config::ROOT_BLOCK_END;
#[cfg(feature = "compiler")]
pub use codex_config::claude_hook_block;
#[cfg(feature = "compiler")]
pub use codex_config::codex_hook_block;
#[cfg(feature = "compiler")]
pub use codex_config::codex_user_trust_state_status;
#[cfg(feature = "compiler")]
pub use codex_config::default_claude_settings_path;
#[cfg(feature = "compiler")]
pub use codex_config::install_codex_user_trust_state;
#[cfg(feature = "compiler")]
pub use codex_config::merge_claude_settings;
#[cfg(feature = "compiler")]
pub use codex_config::merge_codex_config;
#[cfg(feature = "compiler")]
pub use codex_config::remove_codex_managed_hook_config;
#[cfg(feature = "compiler")]
pub use codex_config::validate_claude_settings_json;
#[cfg(feature = "compiler")]
pub use codex_config::validate_codex_config_toml;
#[cfg(feature = "compiler")]
pub use codex_project_trust::install_codex_user_project_trust;
#[cfg(feature = "compiler")]
pub use dev_context::ActiveContextRecord;
#[cfg(feature = "compiler")]
pub use dev_context::record_active_context;
#[cfg(feature = "compiler")]
pub use event_state::HookSessionAgentRoute;
#[cfg(feature = "compiler")]
pub use event_state::append_hook_event_state;
#[cfg(feature = "compiler")]
pub use event_state::append_reader_probe_event_state;
#[cfg(feature = "compiler")]
pub use event_state::apply_repeated_deny_replay;
#[cfg(feature = "compiler")]
pub use event_state::has_recorded_subagent_context;
#[cfg(feature = "compiler")]
pub use event_state::latest_hook_session_agent_route;
#[cfg(feature = "compiler")]
pub use event_state::latest_hook_session_agent_route_for_root;
#[cfg(feature = "compiler")]
pub use event_state::latest_hook_session_agent_route_for_root_matching_rules;
#[cfg(feature = "compiler")]
pub use event_state::remove_incompatible_hook_event_state;
#[cfg(feature = "compiler")]
pub use event_state::try_append_hook_event_state;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::SubagentModelDriftObservation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::SubagentProfileDriftObservation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::SubagentRuntimeDriftObservation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::SubagentRuntimeRebindObservation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::SubagentRuntimeRebindVerifiedObservation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::UnmanagedSubagentStartObservation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::latest_subagent_model_drift;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::latest_subagent_profile_drift;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::latest_subagent_runtime_drift;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::latest_subagent_runtime_rebind_observation;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::latest_subagent_runtime_rebind_verified;
#[cfg(feature = "compiler")]
pub use event_state_subagent_model_drift::latest_unmanaged_subagent_start;
#[cfg(feature = "compiler")]
pub use hook_config::ClientHookConfig;
#[cfg(feature = "compiler")]
pub use hook_config::DurableHookConfigArtifact;
#[cfg(feature = "compiler")]
pub use hook_config::MaterializedDecisionShards;
#[cfg(feature = "compiler")]
pub use hook_config::default_client_config_path;
#[cfg(feature = "compiler")]
pub use hook_config::default_client_config_projection_digest;
#[cfg(feature = "compiler")]
pub use hook_config::default_client_config_template;
#[cfg(feature = "compiler")]
pub use hook_config::hook_runtime_artifact_fingerprint;
#[cfg(feature = "compiler")]
pub use hook_config::load_client_config;
#[cfg(feature = "compiler")]
pub use hook_config::load_client_config_for_matcher_publication;
#[cfg(feature = "compiler")]
pub use hook_config::load_client_config_for_project;
#[cfg(feature = "compiler")]
pub use hook_config::load_client_config_for_project_with_executable_capabilities;
#[cfg(feature = "compiler")]
pub use hook_config::load_client_config_overlay_for_project;
#[cfg(feature = "compiler")]
pub use hook_config::load_embedded_client_config_for_project;
#[cfg(feature = "compiler")]
pub(crate) use hook_config_agent_org::AgentOrgArtifactsArchiveWarning;
#[cfg(feature = "compiler")]
pub(crate) use hook_config_agent_org::AgentOrgArtifactsRecovery;
#[cfg(feature = "compiler")]
pub(crate) use hook_config_agent_org::CompiledAgentOrgArtifactsConfig;
#[cfg(feature = "compiler")]
pub use hook_config_global::default_global_client_config_path;
#[cfg(feature = "compiler")]
pub use match_policy_conformance::validate_match_policy_rule_coverage;
#[cfg(feature = "compiler")]
pub use protocol::ActionPolicy;
#[cfg(feature = "compiler")]
pub use protocol::AgentHookError;
#[cfg(feature = "compiler")]
pub use protocol::CANONICAL_SCHEMA_AUTHORITY;
#[cfg(feature = "compiler")]
pub use protocol::CommandTemplate;
#[cfg(feature = "compiler")]
pub use protocol::DecisionKind;
#[cfg(feature = "compiler")]
pub use protocol::DecisionRoute;
#[cfg(feature = "compiler")]
pub use protocol::DecisionRouteKind;
#[cfg(feature = "compiler")]
pub use protocol::DecisionSubject;
#[cfg(feature = "compiler")]
pub use protocol::HOOK_DECISION_SCHEMA_ID;
#[cfg(feature = "compiler")]
pub use protocol::HOOK_DECISION_SCHEMA_VERSION;
#[cfg(feature = "compiler")]
pub use protocol::HOOK_PROTOCOL_ID;
#[cfg(feature = "compiler")]
pub use protocol::HOOK_PROTOCOL_VERSION;
#[cfg(feature = "compiler")]
pub use protocol::HookDecision;
#[cfg(feature = "compiler")]
pub use protocol::HookPolicy;
#[cfg(feature = "compiler")]
pub use protocol::ReasonKind;
#[cfg(feature = "compiler")]
pub use protocol::StdinMode;
#[cfg(feature = "compiler")]
pub use protocol::parse_payload;
#[cfg(feature = "compiler")]
pub use protocol::render_codex_permission_request;
#[cfg(feature = "compiler")]
pub use protocol::render_codex_pre_tool_deny;
#[cfg(feature = "compiler")]
pub use protocol::render_platform_response;
#[cfg(feature = "compiler")]
pub use protocol::subagent_deny_message;
#[cfg(feature = "compiler")]
pub use provider_manifest::project_agent_config_path;
#[cfg(feature = "compiler")]
pub use provider_projection::HookProviderProjection;
#[cfg(feature = "compiler")]
pub use provider_projection::HookRuntime;
#[cfg(feature = "compiler")]
pub use shell_read_decision_shard::CommandDecisionShard;
#[cfg(feature = "compiler")]
pub(crate) use source_selector::collect_source_selector_matches;
#[cfg(feature = "compiler")]
pub use structured_projection_decision_shard::StructuredProjectionDecisionShard;
#[cfg(feature = "compiler")]
pub(crate) use tool_action::OperationIntent;
#[cfg(feature = "compiler")]
pub(crate) use tool_action::ToolAction;
#[cfg(feature = "compiler")]
pub(crate) use tool_action::collect_tool_actions;
#[cfg(feature = "compiler")]
pub(crate) use tool_action::payload_string;
#[cfg(feature = "compiler")]
pub(crate) use tool_action::subject_for_action;
#[cfg(feature = "compiler")]
pub use tool_action::workspace_mutation_paths;
#[cfg(feature = "compiler")]
mod dev_context;
#[cfg(all(feature = "compiler", test))]
extern crate self as agent_semantic_hook;

#[cfg(feature = "compiler")]
pub use crate::provider_registry::registered_provider_id;
#[cfg(feature = "compiler")]
use agent_semantic_shell_parser as shell_parser;
#[cfg(feature = "compiler")]
mod agent_dispatch_message;
