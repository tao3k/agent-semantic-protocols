//! Runtime status helpers for agent sessions and resident child activity.

use serde::{Deserialize, Serialize};

use crate::codex_rollout_sessions::codex_rollout_paths_for_session_id;
use std::{
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const CODEX_ROLLOUT_METADATA_HEADER_LINE_LIMIT: usize = 32;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeSessionId(String);

impl RuntimeSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RuntimeSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RuntimeSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Display for RuntimeSessionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSessionStatusError(String);

impl From<String> for RuntimeSessionStatusError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<RuntimeSessionStatusError> for String {
    fn from(value: RuntimeSessionStatusError) -> Self {
        value.0
    }
}

/// Runtime-visible agent session discovered from host environment variables.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeSession {
    /// Host client that supplied the session id.
    pub client: String,
    /// Host session id, such as `CODEX_THREAD_ID` or Claude Code session id.
    pub id: String,
}

impl AgentRuntimeSession {
    /// The host-provided id used as recall identity before registry parent lookup.
    pub fn recall_session_id(&self) -> &str {
        &self.id
    }
}

/// Codex local rollout metadata for a thread/session id.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutSessionMetadata {
    pub(crate) session_id: RuntimeSessionId,
    pub(crate) rollout_path: PathBuf,
    pub(crate) rollout_created_at_unix: Option<i64>,
    pub(crate) root_session_id: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) thread_source: Option<String>,
    pub(crate) agent_role: Option<String>,
    pub(crate) agent_nickname: Option<String>,
    pub(crate) agent_path: Option<String>,
    pub(crate) spawn_depth: Option<i64>,
    pub(crate) model_provider: Option<String>,
    pub(crate) cli_version: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) collaboration_model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) sandbox_policy: Option<String>,
    pub(crate) approval_policy: Option<String>,
    pub(crate) permission_profile: Option<String>,
}

/// Resolve Codex rollout metadata for a session id from local Codex JSONL logs.
///
/// This is a passive adapter: it does not send a prompt, resume a session, or
/// depend on an experimental app-server socket.
pub fn codex_rollout_session_metadata(
    session_id: &RuntimeSessionId,
) -> Result<Option<CodexRolloutSessionMetadata>, RuntimeSessionStatusError> {
    let sessions_dir = codex_sessions_dir()?;
    if !sessions_dir.is_dir() {
        return Ok(None);
    }
    for path in codex_rollout_paths_for_session_id(&sessions_dir, session_id.as_str())? {
        if let Some(metadata) = read_codex_rollout_metadata(&path, session_id.as_str())? {
            return Ok(Some(metadata));
        }
    }
    Ok(None)
}

/// Resolve Codex rollout metadata only when it is inside a registration window.
pub fn codex_rollout_session_metadata_recent(
    session_id: &RuntimeSessionId,
    reference_unix: i64,
    max_age_seconds: i64,
) -> Result<Option<CodexRolloutSessionMetadata>, String> {
    let Some(metadata) = codex_rollout_session_metadata(session_id)? else {
        return Ok(None);
    };
    let Some(created_at) = metadata.rollout_created_at_unix else {
        return Ok(None);
    };
    let age_seconds = reference_unix - created_at;
    if (0..=max_age_seconds).contains(&age_seconds) {
        Ok(Some(metadata))
    } else {
        Ok(None)
    }
}

/// Discover the current host agent session from well-known environment ids.
#[must_use]
pub fn current_agent_runtime_session() -> Option<AgentRuntimeSession> {
    let sessions = [
        ("CODEX_THREAD_ID", "codex"),
        ("CLAUDE_CODE_SESSION_ID", "claude-code"),
        ("CLAUDE_CODE_REMOTE_SESSION_ID", "claude-code"),
    ]
    .into_iter()
    .filter_map(|(name, client)| {
        env_value(name).map(|id| AgentRuntimeSession {
            client: client.to_string(),
            id,
        })
    })
    .collect::<Vec<_>>();
    (sessions.len() == 1).then(|| sessions.into_iter().next().expect("one session"))
}

fn codex_sessions_dir() -> Result<PathBuf, String> {
    if let Some(codex_home) = env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(codex_home).join("sessions"));
    }
    env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".codex").join("sessions"))
        .ok_or_else(|| "HOME is not set; cannot locate Codex sessions".to_string())
}

fn read_codex_rollout_metadata(
    path: &Path,
    session_id: &str,
) -> Result<Option<CodexRolloutSessionMetadata>, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open Codex rollout {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut metadata = CodexRolloutSessionMetadata {
        session_id: RuntimeSessionId::from(session_id),
        rollout_path: path.to_path_buf(),
        rollout_created_at_unix: path_unix_timestamp(path)?,
        root_session_id: None,
        parent_thread_id: None,
        thread_source: None,
        agent_role: None,
        agent_nickname: None,
        agent_path: None,
        spawn_depth: None,
        model_provider: None,
        cli_version: None,
        cwd: None,
        model: None,
        collaboration_model: None,
        reasoning_effort: None,
        sandbox_policy: None,
        approval_policy: None,
        permission_profile: None,
    };
    let mut saw_matching_session_meta = false;
    for (line_index, line) in reader.lines().enumerate() {
        if line_index >= CODEX_ROLLOUT_METADATA_HEADER_LINE_LIMIT {
            break;
        }
        let line = line.map_err(|error| {
            format!(
                "failed to read Codex rollout line from {}: {error}",
                path.display()
            )
        })?;
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("session_meta") => {
                let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
                let payload_id = string_at(payload, "/id");
                let payload_session_id = string_at(payload, "/session_id");
                if payload_id.as_deref() != Some(session_id)
                    && payload_session_id.as_deref() != Some(session_id)
                {
                    continue;
                }
                saw_matching_session_meta = true;
                metadata.root_session_id = payload_session_id;
                metadata.parent_thread_id = string_at(payload, "/parent_thread_id").or_else(|| {
                    string_at(payload, "/source/subagent/thread_spawn/parent_thread_id")
                });
                metadata.thread_source = string_at(payload, "/thread_source");
                metadata.agent_role = string_at(payload, "/agent_role")
                    .or_else(|| string_at(payload, "/source/subagent/thread_spawn/agent_role"));
                metadata.agent_nickname = string_at(payload, "/agent_nickname")
                    .or_else(|| string_at(payload, "/source/subagent/thread_spawn/agent_nickname"));
                metadata.agent_path =
                    string_at(payload, "/source/subagent/thread_spawn/agent_path");
                metadata.spawn_depth = i64_at(payload, "/source/subagent/thread_spawn/depth");
                metadata.model_provider = string_at(payload, "/model_provider");
                metadata.cli_version = string_at(payload, "/cli_version");
                metadata.cwd = string_at(payload, "/cwd");
            }
            Some("turn_context") if saw_matching_session_meta => {
                let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
                if let Some(model) = string_at(payload, "/model") {
                    metadata.model = Some(model);
                }
                if let Some(collaboration_model) = string_at(payload, "/collaboration_model") {
                    metadata.collaboration_model = Some(collaboration_model);
                }
                if let Some(reasoning_effort) = string_at(payload, "/reasoning_effort")
                    .or_else(|| string_at(payload, "/reasoningEffort"))
                    .or_else(|| string_at(payload, "/effort"))
                {
                    metadata.reasoning_effort = Some(reasoning_effort);
                }
                if let Some(sandbox_policy) = string_at(payload, "/sandbox_policy/type") {
                    metadata.sandbox_policy = Some(sandbox_policy);
                }
                if let Some(approval_policy) = string_at(payload, "/approval_policy") {
                    metadata.approval_policy = Some(approval_policy);
                }
                if let Some(permission_profile) = string_at(payload, "/permission_profile/type") {
                    metadata.permission_profile = Some(permission_profile);
                }
            }
            _ => {}
        }
    }
    Ok(saw_matching_session_meta.then_some(metadata))
}

fn path_unix_timestamp(path: &Path) -> Result<Option<i64>, String> {
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect Codex rollout {}: {error}",
            path.display()
        )
    })?;
    let created = metadata.created().or_else(|_| metadata.modified()).ok();
    created
        .map(|time| {
            time.duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64)
                .map_err(|error| {
                    format!(
                        "failed to convert Codex rollout timestamp for {}: {error}",
                        path.display()
                    )
                })
        })
        .transpose()
}

fn string_at(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn i64_at(value: &serde_json::Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(serde_json::Value::as_i64)
}

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

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(non_empty_value)
}

fn non_empty_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
