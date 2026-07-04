//! Runtime-owned status snapshot for agent-session diagnostics.

use std::path::Path;

use crate::agent_session_status::{
    agent_session_artifact_activity, agent_session_duplicate_worker_allowed,
    agent_session_health_status, agent_session_host_probe, agent_session_next_action,
    agent_session_timeout_semantics, current_agent_runtime_session,
};

/// Runtime facts rendered by `asp agent session status`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSessionRuntimeStatusSnapshot {
    /// Host client that owns the probed thread, when one is known.
    pub host_client: Option<String>,
    /// Host thread id used for status probing.
    pub host_thread_id: Option<String>,
    /// Source that produced host status.
    pub host_status_source: String,
    /// Normalized host status.
    pub host_status: String,
    /// Human-readable reason for the host status.
    pub host_status_reason: String,
    /// Raw host status payload when the host exposes one.
    pub host_raw_status: Option<String>,
    /// Combined registry/host/artifact health status.
    pub health_status: String,
    /// Timeout semantics used by resident agent sessions.
    pub timeout_semantics: &'static str,
    /// Whether duplicate resident workers are allowed.
    pub duplicate_worker_allowed: bool,
    /// Artifact directory inspected for session activity.
    pub artifacts_dir: String,
    /// Normalized artifact freshness status.
    pub artifact_status: String,
    /// Staleness threshold used for artifact freshness.
    pub artifact_stale_after_seconds: i64,
    /// Latest artifact update timestamp.
    pub last_artifact_updated_at: Option<i64>,
    /// Latest artifact age in seconds.
    pub artifact_age_seconds: Option<i64>,
    /// Latest artifact path, when available.
    pub last_artifact_path: Option<String>,
    /// Suggested next action for the session.
    pub next_action: String,
}

/// Build the runtime status snapshot for one status command render.
pub fn agent_session_runtime_status_snapshot(
    project_root: &Path,
    now: i64,
    artifact_stale_after_seconds: i64,
    host_thread_id: Option<&str>,
    has_registry_record: bool,
    routable: bool,
) -> Result<AgentSessionRuntimeStatusSnapshot, String> {
    let artifacts =
        agent_session_artifact_activity(project_root, now, artifact_stale_after_seconds)?;
    let runtime_session = current_agent_runtime_session();
    let host_probe = agent_session_host_probe(runtime_session.as_ref(), host_thread_id);
    let next_action = agent_session_next_action(has_registry_record, routable, artifacts.status);
    let health_status = agent_session_health_status(
        has_registry_record,
        routable,
        host_probe.status,
        artifacts.status,
    );
    Ok(AgentSessionRuntimeStatusSnapshot {
        host_client: host_probe.client,
        host_thread_id: host_probe.thread_id,
        host_status_source: host_probe.source.as_str().to_string(),
        host_status: host_probe.status.as_str().to_string(),
        host_status_reason: host_probe.reason,
        host_raw_status: host_probe.raw_status,
        health_status: health_status.as_str().to_string(),
        timeout_semantics: agent_session_timeout_semantics(),
        duplicate_worker_allowed: agent_session_duplicate_worker_allowed(),
        artifacts_dir: artifacts.artifacts_dir.display().to_string(),
        artifact_status: artifacts.status.as_str().to_string(),
        artifact_stale_after_seconds,
        last_artifact_updated_at: artifacts.latest_updated_at,
        artifact_age_seconds: artifacts.age_seconds,
        last_artifact_path: artifacts.latest_path.map(|path| path.display().to_string()),
        next_action: next_action.as_str().to_string(),
    })
}
