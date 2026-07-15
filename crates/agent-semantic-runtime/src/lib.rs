#![deny(dead_code)]

//! Runtime state materialization for ASP project-local storage.

mod agent_session_identity;
mod agent_session_status;
mod agent_session_status_snapshot;
mod agent_session_validation_report;
mod async_bridge;
mod codex_app_server_sessions;
mod codex_rollout_sessions;
pub use codex_app_server_sessions::codex_app_server_child_session_metadata;
mod git;
mod graph_render;
pub mod language_owner_items;
mod runtime_source;
pub mod state;
pub mod state_core;
mod timeout_policy;

pub use agent_session_identity::{
    AgentSessionRegistrationIdentity, AgentSessionRegistrationIdentityRequest,
    agent_session_registration_identity, current_agent_runtime_root_session_id,
    has_current_agent_runtime_session,
};
pub use agent_session_status::{
    AgentRuntimeSession, AgentSessionArtifactActivity, AgentSessionArtifactStatus,
    AgentSessionHealthStatus, AgentSessionHostProbe, AgentSessionHostProbeRequest,
    AgentSessionHostStatus, AgentSessionHostStatusSource, AgentSessionNextAction,
    CodexRolloutSessionMetadata, agent_session_artifact_activity,
    agent_session_duplicate_worker_allowed, agent_session_health_status, agent_session_host_probe,
    agent_session_host_status, agent_session_host_status_reason, agent_session_host_status_source,
    agent_session_next_action, agent_session_timeout_semantics, codex_rollout_session_metadata,
    codex_rollout_session_metadata_recent, current_agent_runtime_session,
};
pub use agent_session_status_snapshot::{
    AgentSessionRuntimeStatusSnapshot, AgentSessionRuntimeStatusSnapshotRequest,
    agent_session_runtime_status_snapshot,
};
pub use agent_session_validation_report::AgentSessionValidationReport;
pub use async_bridge::runtime_block_on_current_thread;
pub use codex_rollout_sessions::{
    CodexRolloutSessionIndex, codex_rollout_session_index, codex_rollout_session_index_for_sessions,
};
pub use graph_render::{
    GraphRenderReceiptRequest, run_graph_render_packet, run_graph_render_packet_bytes,
    run_graph_render_packet_bytes_with_receipt,
};
pub use language_owner_items::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsCacheRequest, LanguageOwnerItemsDispatchPlan,
    LanguageOwnerItemsProviderOutput, LanguageOwnerItemsRuntimeOutcome,
    LanguageOwnerItemsRuntimeReceipt, compact_language_owner_items_stdout,
    language_owner_items_failure, language_owner_items_runtime_receipt,
    language_owner_items_workspace_root, language_owner_path_exists, language_owner_source_path,
    read_language_owner_items_cache, resolve_language_owner_items_runtime_outcome,
    run_language_owner_items_dispatch_plan, write_language_owner_items_cache,
};
pub use runtime_source::{
    RuntimeSourceCheckout, RuntimeSourceIndexContext, RuntimeSourceIndexContextRequest,
    RuntimeSourceIndexFile, RuntimeSourceIndexFilesRequest,
    RuntimeSourceRegistryFingerprintRequest, RuntimeSourceSpec, collect_runtime_source_index_files,
    ensure_runtime_source_checkout, ensure_runtime_source_checkout_in_client_cache,
    runtime_source_checkout_dir, runtime_source_checkout_dir_in_client_cache,
    runtime_source_index_context, runtime_source_registry_fingerprint,
};
pub use state::{
    ProjectRuntimeState, ProjectStatePaths, ensure_project_artifacts_dir,
    ensure_project_client_cache_dir, ensure_project_hook_cache_dir, ensure_project_hook_state_dir,
    ensure_project_provider_bin_dir, ensure_project_provider_lock_dir, ensure_project_runtime_home,
    project_activation_path, project_cache_home, project_cache_home_for_roots,
    project_protocol_home_path, project_runtime_state, project_state_paths,
    runtime_bin_dir_for_cache_home,
};
pub use timeout_policy::{
    RuntimeOperationTimeoutPolicy, RuntimeOperationTimeoutReceipt,
    runtime_operation_timeout_receipt,
};

#[cfg(test)]
#[path = "../tests/unit/agent_session_status.rs"]
mod agent_session_status_tests;
#[cfg(test)]
#[path = "../tests/unit/language_owner_items.rs"]
mod language_owner_items_tests;
#[cfg(test)]
#[path = "../tests/unit/timeout_policy.rs"]
mod timeout_policy_tests;
pub use state::{
    discover_project_activation_path, is_project_activation_path, project_root_for_activation_path,
};
