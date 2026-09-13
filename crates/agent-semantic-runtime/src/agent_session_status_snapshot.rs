// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime-owned status snapshot for agent-session diagnostics.

use std::path::Path;

use crate::agent_session_status::AgentSessionArtifactStatus;
use crate::agent_session_status::AgentSessionHealthStatus;
use crate::agent_session_status::AgentSessionNextAction;
use crate::agent_session_status::agent_session_artifact_activity;
use crate::agent_session_status::agent_session_duplicate_worker_allowed;
use crate::agent_session_status::agent_session_health_status;
use crate::agent_session_status::agent_session_host_probe;
use crate::agent_session_status::agent_session_next_action;
use crate::agent_session_status::agent_session_timeout_semantics;
use crate::agent_session_status::current_agent_runtime_session;

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentSessionHostRawStatus(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentSessionTimeoutSemantics(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentSessionDuplicateWorkerPolicy(bool);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentSessionStaleAfterSeconds(i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentSessionArtifactUpdatedAt(i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentSessionArtifactAgeSeconds(i64);

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
    host_raw_status: Option<AgentSessionHostRawStatus>,
    /// Combined registry/host/artifact health status.
    health_status: AgentSessionHealthStatus,
    /// Timeout semantics used by resident agent sessions.
    timeout_semantics: AgentSessionTimeoutSemantics,
    /// Whether duplicate resident workers are allowed.
    duplicate_worker_allowed: AgentSessionDuplicateWorkerPolicy,
    /// Artifact directory inspected for session activity.
    artifacts_dir: std::path::PathBuf,
    /// Normalized artifact freshness status.
    artifact_status: AgentSessionArtifactStatus,
    /// Staleness threshold used for artifact freshness.
    artifact_stale_after_seconds: AgentSessionStaleAfterSeconds,
    /// Latest artifact update timestamp.
    last_artifact_updated_at: Option<AgentSessionArtifactUpdatedAt>,
    /// Latest artifact age in seconds.
    artifact_age_seconds: Option<AgentSessionArtifactAgeSeconds>,
    /// Latest artifact path, when available.
    last_artifact_path: Option<std::path::PathBuf>,
    /// Suggested next action for the session.
    next_action: AgentSessionNextAction,
}

impl AgentSessionRuntimeStatusSnapshot {
    pub fn host_client(&self) -> Option<&AgentSessionHostClient> {
        self.host_client.as_ref()
    }

    pub fn host_thread_id(&self) -> Option<&AgentSessionHostThreadId> {
        self.host_thread_id.as_ref()
    }

    pub fn host_status_source(&self) -> &AgentSessionHostStatusSource {
        &self.host_status_source
    }

    pub fn host_status(&self) -> &AgentSessionHostStatus {
        &self.host_status
    }

    pub fn host_status_reason(&self) -> &AgentSessionHostStatusReason {
        &self.host_status_reason
    }

    pub fn host_raw_status(&self) -> Option<&str> {
        self.host_raw_status.as_ref().map(|value| value.0.as_str())
    }

    pub fn health_status(&self) -> AgentSessionHealthStatus {
        self.health_status
    }

    pub fn timeout_semantics(&self) -> &'static str {
        self.timeout_semantics.0
    }

    pub fn duplicate_worker_allowed(&self) -> bool {
        self.duplicate_worker_allowed.0
    }

    pub fn artifacts_dir(&self) -> &std::path::Path {
        &self.artifacts_dir
    }

    pub fn artifact_status(&self) -> AgentSessionArtifactStatus {
        self.artifact_status
    }

    pub fn artifact_stale_after_seconds(&self) -> i64 {
        self.artifact_stale_after_seconds.0
    }

    pub fn last_artifact_updated_at(&self) -> Option<i64> {
        self.last_artifact_updated_at.map(|value| value.0)
    }

    pub fn artifact_age_seconds(&self) -> Option<i64> {
        self.artifact_age_seconds.map(|value| value.0)
    }

    pub fn last_artifact_path(&self) -> Option<&std::path::Path> {
        self.last_artifact_path.as_deref()
    }

    pub fn next_action(&self) -> AgentSessionNextAction {
        self.next_action
    }
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
        host_raw_status: host_probe.raw_status.map(AgentSessionHostRawStatus),
        health_status,
        timeout_semantics: AgentSessionTimeoutSemantics(agent_session_timeout_semantics()),
        duplicate_worker_allowed: AgentSessionDuplicateWorkerPolicy(
            agent_session_duplicate_worker_allowed(),
        ),
        artifacts_dir: artifacts.artifacts_dir,
        artifact_status: artifacts.status,
        artifact_stale_after_seconds: AgentSessionStaleAfterSeconds(
            request.artifact_stale_after_seconds,
        ),
        last_artifact_updated_at: artifacts
            .latest_updated_at
            .map(AgentSessionArtifactUpdatedAt),
        artifact_age_seconds: artifacts.age_seconds.map(AgentSessionArtifactAgeSeconds),
        last_artifact_path: artifacts.latest_path,
        next_action,
    })
}
