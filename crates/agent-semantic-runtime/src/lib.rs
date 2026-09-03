//! Runtime state materialization for ASP project-local storage.

#![deny(dead_code)]

pub mod provider_catalog;
pub use provider_catalog::ProviderInstallReceipt;

mod agent_session_identity;
mod agent_session_status;
pub use agent_session_status::RuntimeSessionId;
mod agent_session_status_snapshot;
mod agent_session_validation_report;
mod async_bridge;
mod codex_rollout_sessions;
pub mod git;
mod graph_render;
pub mod hook_process_runtime;
mod live_corpus;
pub mod provider_workspace_artifact;
pub mod runtime_identity_monitor;
pub mod runtime_process_lifecycle;
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
    codex_rollout_session_metadata_at_path, codex_rollout_session_metadata_recent,
    current_agent_runtime_session,
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
pub use live_corpus::{
    LIVE_CORPUS_ARTIFACT_SCHEMA_ID, LIVE_CORPUS_ARTIFACT_SCHEMA_VERSION,
    LiveCorpusArtifactGitIdentity, LiveCorpusArtifactIdentity, LiveCorpusArtifactManifest,
    LiveCorpusArtifactPaths, LiveCorpusGitCheckoutQualification, LiveCorpusGitCheckoutSync,
    LiveCorpusGitRepositoryPaths, LiveCorpusLanguageExtensionEvidence,
    live_corpus_artifact_manifest, live_corpus_artifact_paths, live_corpus_git_checkout_is_clean,
    live_corpus_git_repository_paths, live_corpus_lock_digest, qualify_live_corpus_git_checkout,
    qualify_live_corpus_language_extensions, sync_live_corpus_git_checkout,
};
pub use runtime_source::{
    RuntimeSourceCheckout, RuntimeSourceIndexContext, RuntimeSourceIndexContextRequest,
    RuntimeSourceIndexFile, RuntimeSourceIndexFilesRequest,
    RuntimeSourceRegistryFingerprintRequest, RuntimeSourceSpec, collect_runtime_source_index_files,
    ensure_runtime_source_checkout, ensure_runtime_source_checkout_in_runtime_root,
    runtime_source_checkout_dir, runtime_source_checkout_dir_in_runtime_root,
    runtime_source_index_context, runtime_source_registry_fingerprint,
};
pub use state::{
    ProjectRuntimeState, ProjectStatePaths, ensure_project_artifacts_dir,
    ensure_project_client_cache_dir, ensure_project_hook_cache_dir, ensure_project_hook_state_dir,
    ensure_project_provider_bin_dir, ensure_project_provider_lock_dir, ensure_project_runtime_home,
    project_activation_path, project_protocol_home_path, project_runtime_state,
    project_runtime_state_with_state_home, project_state_paths,
    project_state_paths_with_state_home, provider_package_dir, provider_receipt_dir,
    provider_state_root, temporary_workspace_runtime_state_with_owner_and_state_home,
};
pub use state_core::resolve_state_home;
pub use timeout_policy::{
    RuntimeOperationTimeoutPolicy, RuntimeOperationTimeoutReceipt,
    runtime_operation_timeout_receipt,
};

#[cfg(test)]
#[path = "../tests/unit/agent_session_status.rs"]
mod agent_session_status_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_host_authority.rs"]
mod runtime_host_authority_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_identity_monitor.rs"]
mod runtime_identity_monitor_tests;
#[cfg(test)]
#[path = "../tests/unit/runtime_process_lifecycle.rs"]
mod runtime_process_lifecycle_tests;
#[cfg(test)]
#[path = "../tests/unit/timeout_policy.rs"]
mod timeout_policy_tests;

pub use state::{
    discover_project_activation_path, is_project_activation_path, project_root_for_activation_path,
};
pub mod runtime_artifact_identity;
