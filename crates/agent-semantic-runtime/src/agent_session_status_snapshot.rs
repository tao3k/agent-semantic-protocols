//! Runtime-owned status snapshot for agent-session diagnostics.

use std::path::Path;

use crate::agent_session_status::{
    agent_session_artifact_activity, agent_session_duplicate_worker_allowed,
    agent_session_health_status, agent_session_host_probe, agent_session_next_action,
    agent_session_timeout_semantics, current_agent_runtime_session,
};

macro_rules! status_snapshot_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(String);

        impl $name {
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

status_snapshot_text!(AgentSessionHostClient);
status_snapshot_text!(AgentSessionHostThreadId);
status_snapshot_text!(AgentSessionHostStatusSource);
status_snapshot_text!(AgentSessionHostStatus);
status_snapshot_text!(AgentSessionHostStatusReason);

/// Runtime facts rendered by `asp agent session status`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSessionRuntimeStatusSnapshot {
    /// Host client that owns the probed thread, when one is known.
    host_client: Option<AgentSessionHostClient>,
    /// Host thread id used for status probing.
    host_thread_id: Option<AgentSessionHostThreadId>,
    /// Source that produced host status.
    host_status_source: AgentSessionHostStatusSource,
    /// Normalized host status.
    host_status: AgentSessionHostStatus,
    /// Human-readable reason for the host status.
    host_status_reason: AgentSessionHostStatusReason,
    /// Raw host status payload when the host exposes one.
    host_raw_status: Option<String>,
    /// Combined registry/host/artifact health status.
    health_status: String,
    /// Timeout semantics used by resident agent sessions.
    timeout_semantics: &'static str,
    /// Whether duplicate resident workers are allowed.
    duplicate_worker_allowed: bool,
    /// Artifact directory inspected for session activity.
    artifacts_dir: String,
    /// Normalized artifact freshness status.
    artifact_status: String,
    /// Staleness threshold used for artifact freshness.
    artifact_stale_after_seconds: i64,
    /// Latest artifact update timestamp.
    last_artifact_updated_at: Option<i64>,
    /// Latest artifact age in seconds.
    artifact_age_seconds: Option<i64>,
    /// Latest artifact path, when available.
    last_artifact_path: Option<String>,
    /// Suggested next action for the session.
    next_action: String,
}

/// Request for building one runtime status snapshot.
pub struct AgentSessionRuntimeStatusSnapshotRequest<'a> {
    project_root: &'a Path,
    now: i64,
    artifact_stale_after_seconds: i64,
    host_thread_id: Option<&'a str>,
    has_registry_record: bool,
    routable: bool,
}

impl<'a> From<(&'a Path, i64, i64, Option<&'a str>, bool, bool)>
    for AgentSessionRuntimeStatusSnapshotRequest<'a>
{
    fn from(
        (
            project_root,
            now,
            artifact_stale_after_seconds,
            host_thread_id,
            has_registry_record,
            routable,
        ): (&'a Path, i64, i64, Option<&'a str>, bool, bool),
    ) -> Self {
        Self {
            project_root,
            now,
            artifact_stale_after_seconds,
            host_thread_id,
            has_registry_record,
            routable,
        }
    }
}

/// Build the runtime status snapshot for one status command render.
pub fn agent_session_runtime_status_snapshot(
    request: AgentSessionRuntimeStatusSnapshotRequest<'_>,
) -> Result<AgentSessionRuntimeStatusSnapshot, String> {
    let artifacts = agent_session_artifact_activity(
        request.project_root,
        request.now,
        request.artifact_stale_after_seconds,
    )?;
    let runtime_session = current_agent_runtime_session();
    let host_probe =
        agent_session_host_probe((runtime_session.as_ref(), request.host_thread_id).into());
    let next_action = agent_session_next_action(
        request.has_registry_record,
        request.routable,
        artifacts.status,
    );
    let health_status = agent_session_health_status(
        request.has_registry_record,
        request.routable,
        host_probe.status,
        artifacts.status,
    );
    Ok(AgentSessionRuntimeStatusSnapshot {
        host_client: host_probe.client.map(AgentSessionHostClient::from),
        host_thread_id: host_probe.thread_id.map(AgentSessionHostThreadId::from),
        host_status_source: AgentSessionHostStatusSource::from(
            host_probe.source.as_str().to_string(),
        ),
        host_status: AgentSessionHostStatus::from(host_probe.status.as_str().to_string()),
        host_status_reason: AgentSessionHostStatusReason::from(host_probe.reason),
        host_raw_status: host_probe.raw_status,
        health_status: health_status.as_str().to_string(),
        timeout_semantics: agent_session_timeout_semantics(),
        duplicate_worker_allowed: agent_session_duplicate_worker_allowed(),
        artifacts_dir: artifacts.artifacts_dir.display().to_string(),
        artifact_status: artifacts.status.as_str().to_string(),
        artifact_stale_after_seconds: request.artifact_stale_after_seconds,
        last_artifact_updated_at: artifacts.latest_updated_at,
        artifact_age_seconds: artifacts.age_seconds,
        last_artifact_path: artifacts.latest_path.map(|path| path.display().to_string()),
        next_action: next_action.as_str().to_string(),
    })
}
