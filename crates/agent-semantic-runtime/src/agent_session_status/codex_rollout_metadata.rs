//! Runtime status helpers for agent sessions and resident child activity.

use serde::Deserialize;
use serde::Serialize;

use super::runtime_session::RuntimeSessionId;
use crate::codex_rollout_sessions::codex_rollout_paths_for_session_id;
use std::env;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

/// Maximum rollout-header lines inspected before metadata is considered absent.
const CODEX_ROLLOUT_METADATA_HEADER_LINE_LIMIT: usize = 32;

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexThreadSource(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexAgentRole(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexAgentNickname(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexAgentPath(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexSpawnDepth(u32);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexModelProvider(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexCliVersion(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexModel(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexReasoningEffort(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexSandboxPolicy(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexApprovalPolicy(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexPermissionProfile(String);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct CodexRolloutCreatedAtUnix(i64);

/// Codex local rollout metadata for a thread/session id.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutSessionMetadata {
    session_id: RuntimeSessionId,
    rollout_path: PathBuf,
    rollout_created_at_unix: Option<CodexRolloutCreatedAtUnix>,
    root_session_id: Option<RuntimeSessionId>,
    parent_thread_id: Option<RuntimeSessionId>,
    thread_source: Option<CodexThreadSource>,
    agent_role: Option<CodexAgentRole>,
    agent_nickname: Option<CodexAgentNickname>,
    agent_path: Option<CodexAgentPath>,
    spawn_depth: Option<CodexSpawnDepth>,
    model_provider: Option<CodexModelProvider>,
    cli_version: Option<CodexCliVersion>,
    cwd: Option<PathBuf>,
    model: Option<CodexModel>,
    collaboration_model: Option<CodexModel>,
    reasoning_effort: Option<CodexReasoningEffort>,
    sandbox_policy: Option<CodexSandboxPolicy>,
    approval_policy: Option<CodexApprovalPolicy>,
    permission_profile: Option<CodexPermissionProfile>,
}

pub(crate) struct RawCodexRolloutSessionMetadata {
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

impl TryFrom<RawCodexRolloutSessionMetadata> for CodexRolloutSessionMetadata {
    type Error = String;

    fn try_from(raw: RawCodexRolloutSessionMetadata) -> Result<Self, Self::Error> {
        let rollout_created_at_unix = raw
            .rollout_created_at_unix
            .map(|value| {
                (value >= 0)
                    .then_some(CodexRolloutCreatedAtUnix(value))
                    .ok_or_else(|| format!("Codex rollout timestamp must be non-negative: {value}"))
            })
            .transpose()?;
        let spawn_depth = raw
            .spawn_depth
            .map(|value| {
                u32::try_from(value)
                    .map(CodexSpawnDepth)
                    .map_err(|_| format!("Codex spawn depth is outside the u32 domain: {value}"))
            })
            .transpose()?;
        Ok(Self {
            session_id: raw.session_id,
            rollout_path: raw.rollout_path,
            rollout_created_at_unix,
            root_session_id: raw.root_session_id.map(RuntimeSessionId::from),
            parent_thread_id: raw.parent_thread_id.map(RuntimeSessionId::from),
            thread_source: raw.thread_source.map(CodexThreadSource),
            agent_role: raw.agent_role.map(CodexAgentRole),
            agent_nickname: raw.agent_nickname.map(CodexAgentNickname),
            agent_path: raw.agent_path.map(CodexAgentPath),
            spawn_depth,
            model_provider: raw.model_provider.map(CodexModelProvider),
            cli_version: raw.cli_version.map(CodexCliVersion),
            cwd: raw.cwd.map(PathBuf::from),
            model: raw.model.map(CodexModel),
            collaboration_model: raw.collaboration_model.map(CodexModel),
            reasoning_effort: raw.reasoning_effort.map(CodexReasoningEffort),
            sandbox_policy: raw.sandbox_policy.map(CodexSandboxPolicy),
            approval_policy: raw.approval_policy.map(CodexApprovalPolicy),
            permission_profile: raw.permission_profile.map(CodexPermissionProfile),
        })
    }
}

impl CodexRolloutSessionMetadata {
    pub(crate) fn fill_discovered_child_attribution(
        &mut self,
        root_session_id: &RuntimeSessionId,
        agent_path: &str,
    ) {
        self.agent_path
            .get_or_insert_with(|| CodexAgentPath(agent_path.to_owned()));
        self.root_session_id
            .get_or_insert_with(|| root_session_id.clone());
        self.parent_thread_id
            .get_or_insert_with(|| root_session_id.clone());
        if self.parent_thread_id.as_ref() == Some(root_session_id) {
            self.thread_source
                .get_or_insert_with(|| CodexThreadSource("subagent".to_owned()));
            self.spawn_depth.get_or_insert(CodexSpawnDepth(1));
        }
    }

    pub fn session_id(&self) -> &RuntimeSessionId {
        &self.session_id
    }

    pub fn rollout_path(&self) -> &Path {
        &self.rollout_path
    }

    pub(crate) fn rollout_created_at_unix(&self) -> Option<i64> {
        self.rollout_created_at_unix.map(|value| value.0)
    }

    pub fn root_session_id(&self) -> Option<&RuntimeSessionId> {
        self.root_session_id.as_ref()
    }

    pub fn parent_thread_id(&self) -> Option<&RuntimeSessionId> {
        self.parent_thread_id.as_ref()
    }

    pub fn thread_source(&self) -> Option<&str> {
        self.thread_source.as_ref().map(|value| value.0.as_str())
    }

    pub fn agent_role(&self) -> Option<&str> {
        self.agent_role.as_ref().map(|value| value.0.as_str())
    }

    pub fn agent_path(&self) -> Option<&str> {
        self.agent_path.as_ref().map(|value| value.0.as_str())
    }

    pub fn spawn_depth(&self) -> Option<u32> {
        self.spawn_depth.map(|value| value.0)
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_ref().map(|value| value.0.as_str())
    }

    pub fn collaboration_model(&self) -> Option<&str> {
        self.collaboration_model
            .as_ref()
            .map(|value| value.0.as_str())
    }

    pub fn reasoning_effort(&self) -> Option<&str> {
        self.reasoning_effort.as_ref().map(|value| value.0.as_str())
    }

    pub fn sandbox_policy(&self) -> Option<&str> {
        self.sandbox_policy.as_ref().map(|value| value.0.as_str())
    }

    pub fn approval_policy(&self) -> Option<&str> {
        self.approval_policy.as_ref().map(|value| value.0.as_str())
    }

    pub fn permission_profile(&self) -> Option<&str> {
        self.permission_profile
            .as_ref()
            .map(|value| value.0.as_str())
    }
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

/// Parse one exact Codex rollout locator and verify its embedded session id
/// against the typed `session_meta` record before returning metadata.
pub fn codex_rollout_session_metadata_at_path(
    rollout_path: &Path,
) -> Result<Option<CodexRolloutSessionMetadata>, RuntimeSessionStatusError> {
    let stem = rollout_path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| "Codex rollout locator has no UTF-8 file stem".to_owned())?;
    let session_id = stem
        .get(stem.len().saturating_sub(36)..)
        .filter(|candidate| uuid::Uuid::parse_str(candidate).is_ok())
        .ok_or_else(|| "Codex rollout locator has no canonical session UUID suffix".to_owned())?;
    read_codex_rollout_metadata(rollout_path, session_id).map_err(Into::into)
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
    let Some(created_at) = metadata.rollout_created_at_unix() else {
        return Ok(None);
    };
    let age_seconds = reference_unix - created_at;
    if (0..=max_age_seconds).contains(&age_seconds) {
        Ok(Some(metadata))
    } else {
        Ok(None)
    }
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
    let mut metadata = RawCodexRolloutSessionMetadata {
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
    let mut saw_turn_context = false;
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
                saw_turn_context = true;
            }
            // Turn contexts form the metadata header. Once history begins,
            // later event records cannot refine this passive metadata view.
            _ if saw_turn_context => break,
            _ => {}
        }
    }
    if !saw_matching_session_meta {
        return Ok(None);
    }
    Ok(Some(CodexRolloutSessionMetadata::try_from(metadata)?))
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
