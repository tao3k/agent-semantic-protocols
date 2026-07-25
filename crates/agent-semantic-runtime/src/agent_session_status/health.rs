//! Resident-session host, artifact, and combined-health evidence.

use super::runtime_session::AgentRuntimeSession;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Recent ASP artifact activity for a project/workspace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionArtifactActivity {
    pub artifacts_dir: PathBuf,
    pub status: AgentSessionArtifactStatus,
    pub latest_path: Option<PathBuf>,
    pub latest_updated_at: Option<i64>,
    pub age_seconds: Option<i64>,
}

/// Artifact freshness category used as agent-session liveness evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionArtifactStatus {
    MissingArtifactsDir,
    NoArtifacts,
    Recent,
    Stale,
}

impl AgentSessionArtifactStatus {
    /// Stable kebab-case status spelling for CLI reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingArtifactsDir => "missing-artifacts-dir",
            Self::NoArtifacts => "no-artifacts",
            Self::Recent => "recent",
            Self::Stale => "stale",
        }
    }
}

/// Host status source available to ASP.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionHostStatusSource {
    Unavailable,
    CodexCli,
}

impl AgentSessionHostStatusSource {
    /// Stable kebab-case status-source spelling for CLI reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::CodexCli => "codex-cli",
        }
    }
}

/// Host session status as understood by ASP.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionHostStatus {
    Unknown,
    Active,
    Idle,
    NotLoaded,
    SystemError,
    Missing,
}

impl AgentSessionHostStatus {
    /// Stable kebab-case status spelling for CLI reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Active => "active",
            Self::Idle => "idle",
            Self::NotLoaded => "not-loaded",
            Self::SystemError => "system-error",
            Self::Missing => "missing",
        }
    }
}

/// Provider-aware host status probe result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionHostProbe {
    pub(crate) client: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) source: AgentSessionHostStatusSource,
    pub(crate) status: AgentSessionHostStatus,
    pub(crate) reason: String,
    pub(crate) raw_status: Option<String>,
}

/// Combined health summary from registry, host, and artifact evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionHealthStatus {
    MissingRegistry,
    RegistryNotRoutable,
    Healthy,
    HostHealthyArtifactStale,
    HostUnknownArtifactRecent,
    Unknown,
    Unhealthy,
}

impl AgentSessionHealthStatus {
    /// Stable kebab-case health spelling for CLI reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingRegistry => "missing-registry",
            Self::RegistryNotRoutable => "registry-not-routable",
            Self::Healthy => "healthy",
            Self::HostHealthyArtifactStale => "host-healthy-artifact-stale",
            Self::HostUnknownArtifactRecent => "host-unknown-artifact-recent",
            Self::Unknown => "unknown",
            Self::Unhealthy => "unhealthy",
        }
    }
}

/// Next action an agent should take after a resident child status check.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSessionNextAction {
    StartResidentChildAndRegister,
    RegisterExistingChildOrReplaceOnlyAfterHostConfirmsUnrecoverable,
    ResumeOrSendFollowUpToSameChild,
    ResumeOrSendFollowUpToSameChildBeforeConsideringReplacement,
}

impl AgentSessionNextAction {
    /// Stable kebab-case action spelling for CLI reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartResidentChildAndRegister => "start-resident-child-and-register",
            Self::RegisterExistingChildOrReplaceOnlyAfterHostConfirmsUnrecoverable => {
                "register-existing-child-or-replace-only-after-host-confirms-unrecoverable"
            }
            Self::ResumeOrSendFollowUpToSameChild => "resume-or-send-follow-up-to-same-child",
            Self::ResumeOrSendFollowUpToSameChildBeforeConsideringReplacement => {
                "resume-or-send-follow-up-to-same-child-before-considering-replacement"
            }
        }
    }
}

/// ASP's current host status source.
#[must_use]
pub fn agent_session_host_status_source() -> AgentSessionHostStatusSource {
    AgentSessionHostStatusSource::Unavailable
}

/// ASP's current host status.
#[must_use]
pub fn agent_session_host_status() -> AgentSessionHostStatus {
    AgentSessionHostStatus::Unknown
}

/// Host status reason when no stable public status API is available.
#[must_use]
pub fn agent_session_host_status_reason() -> &'static str {
    "no-stable-public-host-session-status-command"
}

/// Request for probing host status for one runtime session.
pub struct AgentSessionHostProbeRequest<'a> {
    session: Option<&'a AgentRuntimeSession>,
    thread_id: Option<&'a str>,
}

impl<'a> From<(Option<&'a AgentRuntimeSession>, Option<&'a str>)>
    for AgentSessionHostProbeRequest<'a>
{
    fn from((session, thread_id): (Option<&'a AgentRuntimeSession>, Option<&'a str>)) -> Self {
        Self { session, thread_id }
    }
}

/// Probe the host runtime for a session id when a provider adapter is available.
#[must_use]
pub fn agent_session_host_probe(
    request: AgentSessionHostProbeRequest<'_>,
) -> AgentSessionHostProbe {
    let session = request.session;
    let thread_id = request.thread_id;
    let Some(session) = session else {
        return AgentSessionHostProbe {
            client: None,
            thread_id: thread_id.map(str::to_string),
            source: AgentSessionHostStatusSource::Unavailable,
            status: AgentSessionHostStatus::Unknown,
            reason: "no-agent-session-env".to_string(),
            raw_status: None,
        };
    };
    let thread_id = thread_id.unwrap_or(session.recall_session_id());
    match session.client.as_str() {
        "codex" => AgentSessionHostProbe {
            client: Some(session.client.clone()),
            thread_id: Some(thread_id.to_string()),
            source: AgentSessionHostStatusSource::CodexCli,
            status: AgentSessionHostStatus::Unknown,
            reason: "codex-cli-session-status-command-not-detected".to_string(),
            raw_status: None,
        },
        _ => AgentSessionHostProbe {
            client: Some(session.client.clone()),
            thread_id: Some(thread_id.to_string()),
            source: AgentSessionHostStatusSource::Unavailable,
            status: AgentSessionHostStatus::Unknown,
            reason: "host-session-status-adapter-unavailable".to_string(),
            raw_status: None,
        },
    }
}

/// Timeout semantics for LLM-backed resident child workers.
#[must_use]
pub fn agent_session_timeout_semantics() -> &'static str {
    "timeout-is-not-duplicate-worker-trigger"
}

/// Duplicate worker policy for one resident child per root session/name.
#[must_use]
pub fn agent_session_duplicate_worker_allowed() -> bool {
    false
}

/// Resolve recent ASP artifact activity for a project.
pub fn agent_session_artifact_activity(
    project_root: impl AsRef<Path>,
    now: i64,
    stale_after_seconds: i64,
) -> Result<AgentSessionArtifactActivity, String> {
    let artifacts_dir = crate::state_core::ResolvedState::resolve(project_root.as_ref())?
        .paths
        .artifacts_dir;
    if !artifacts_dir.is_dir() {
        return Ok(AgentSessionArtifactActivity {
            artifacts_dir,
            status: AgentSessionArtifactStatus::MissingArtifactsDir,
            latest_path: None,
            latest_updated_at: None,
            age_seconds: None,
        });
    }
    let latest = latest_artifact_file(&artifacts_dir)?;
    let Some((latest_path, latest_updated_at)) = latest else {
        return Ok(AgentSessionArtifactActivity {
            artifacts_dir,
            status: AgentSessionArtifactStatus::NoArtifacts,
            latest_path: None,
            latest_updated_at: None,
            age_seconds: None,
        });
    };
    let age_seconds = now.saturating_sub(latest_updated_at);
    let status = if age_seconds <= stale_after_seconds {
        AgentSessionArtifactStatus::Recent
    } else {
        AgentSessionArtifactStatus::Stale
    };
    Ok(AgentSessionArtifactActivity {
        artifacts_dir,
        status,
        latest_path: Some(latest_path),
        latest_updated_at: Some(latest_updated_at),
        age_seconds: Some(age_seconds),
    })
}

/// Derive the agent action from registry and artifact evidence.
#[must_use]
pub fn agent_session_next_action(
    registry_entry_present: bool,
    routable: bool,
    artifact_status: AgentSessionArtifactStatus,
) -> AgentSessionNextAction {
    match (registry_entry_present, routable, artifact_status) {
        (false, _, _) => {
            AgentSessionNextAction::RegisterExistingChildOrReplaceOnlyAfterHostConfirmsUnrecoverable
        }
        (true, false, _) => {
            AgentSessionNextAction::RegisterExistingChildOrReplaceOnlyAfterHostConfirmsUnrecoverable
        }
        (true, true, AgentSessionArtifactStatus::Recent) => {
            AgentSessionNextAction::ResumeOrSendFollowUpToSameChild
        }
        (true, true, _) => {
            AgentSessionNextAction::ResumeOrSendFollowUpToSameChildBeforeConsideringReplacement
        }
    }
}

/// Combine registry, host, and artifact evidence into a conservative health state.
#[must_use]
pub fn agent_session_health_status(
    registry_entry_present: bool,
    routable: bool,
    host_status: AgentSessionHostStatus,
    artifact_status: AgentSessionArtifactStatus,
) -> AgentSessionHealthStatus {
    if !registry_entry_present {
        return AgentSessionHealthStatus::MissingRegistry;
    }
    if !routable {
        return AgentSessionHealthStatus::RegistryNotRoutable;
    }
    match (host_status, artifact_status) {
        (AgentSessionHostStatus::SystemError | AgentSessionHostStatus::Missing, _) => {
            AgentSessionHealthStatus::Unhealthy
        }
        (
            AgentSessionHostStatus::Active | AgentSessionHostStatus::Idle,
            AgentSessionArtifactStatus::Recent,
        ) => AgentSessionHealthStatus::Healthy,
        (
            AgentSessionHostStatus::Active | AgentSessionHostStatus::Idle,
            AgentSessionArtifactStatus::Stale
            | AgentSessionArtifactStatus::NoArtifacts
            | AgentSessionArtifactStatus::MissingArtifactsDir,
        ) => AgentSessionHealthStatus::HostHealthyArtifactStale,
        (AgentSessionHostStatus::Unknown, AgentSessionArtifactStatus::Recent) => {
            AgentSessionHealthStatus::HostUnknownArtifactRecent
        }
        _ => AgentSessionHealthStatus::Unknown,
    }
}

fn latest_artifact_file(root: &Path) -> Result<Option<(PathBuf, i64)>, String> {
    let mut latest: Option<(PathBuf, i64)> = None;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)
            .map_err(|error| format!("failed to read {}: {error}", dir.display()))?
        {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read artifact entry below {}: {error}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "failed to inspect artifact entry {}: {error}",
                    path.display()
                )
            })?;
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .map_err(|error| {
                    format!("failed to read artifact mtime {}: {error}", path.display())
                })?;
            let Ok(duration) = modified.duration_since(UNIX_EPOCH) else {
                continue;
            };
            let updated_at = duration.as_secs() as i64;
            if latest
                .as_ref()
                .is_none_or(|(_, current_updated_at)| updated_at > *current_updated_at)
            {
                latest = Some((path, updated_at));
            }
        }
    }
    Ok(latest)
}
