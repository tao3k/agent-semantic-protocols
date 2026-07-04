//! Codex rollout JSONL session index parser.

use std::{
    collections::BTreeMap,
    collections::{BTreeSet, VecDeque},
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::CodexRolloutSessionMetadata;

/// Root-scoped index derived from Codex local rollout JSONL files.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutSessionIndex {
    pub root_session_id: String,
    pub sessions_dir: PathBuf,
    pub scanned_rollout_count: usize,
    pub skipped_rollout_count: usize,
    pub records: Vec<CodexRolloutSessionMetadata>,
    pub activity_by_session: BTreeMap<String, CodexRolloutActivityReport>,
    pub missing_rollout_by_session: BTreeMap<String, String>,
}

/// Compact heartbeat/event entry parsed from one rollout JSONL stream.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutActivityHeartbeat {
    pub at: Option<i64>,
    pub kind: String,
    pub turn_id: Option<String>,
}

/// Liveness summary derived from a single exact Codex rollout JSONL file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexRolloutActivityReport {
    pub status: String,
    pub rollout_path: PathBuf,
    pub last_event_at: Option<i64>,
    pub last_event_kind: Option<String>,
    pub last_heartbeat_at: Option<i64>,
    pub last_heartbeat_kind: Option<String>,
    pub recent_heartbeats: Vec<CodexRolloutActivityHeartbeat>,
    pub seconds_since_heartbeat: Option<i64>,
    pub current_turn_id: Option<String>,
    pub last_running_session_id: Option<String>,
    pub running_session_closed: bool,
    pub last_terminal_event: Option<String>,
    pub agent_instruction: Option<String>,
    pub scanned_line_count: usize,
}

/// Build a deterministic root-scoped index from Codex rollout JSONL files.
///
/// The rollout JSONL stream is the source of truth for subagent topology. The
/// process environment only gives the current thread id; this index recovers
/// parent, source, role, model, and spawn-depth facts from `session_meta` and
/// `turn_context` records.
pub fn codex_rollout_session_index(
    root_session_id: &str,
) -> Result<Option<CodexRolloutSessionIndex>, String> {
    let sessions_dir = codex_sessions_dir()?;
    if !sessions_dir.is_dir() {
        return Ok(None);
    }

    let mut scanned_rollout_count = 0usize;
    let mut skipped_rollout_count = 0usize;
    let mut latest_by_session = BTreeMap::<String, CodexRolloutSessionMetadata>::new();
    let mut activity_by_session = BTreeMap::<String, CodexRolloutActivityReport>::new();
    let mut missing_rollout_by_session = BTreeMap::<String, String>::new();
    let mut seen_session_ids = BTreeSet::<String>::new();
    let mut queue = VecDeque::from([root_session_id.to_string()]);
    let now = current_unix_timestamp()?;

    while let Some(session_id) = queue.pop_front() {
        if !seen_session_ids.insert(session_id.clone()) {
            continue;
        }
        let path = match codex_rollout_paths_for_session_id(&sessions_dir, &session_id) {
            Ok(paths) => paths
                .into_iter()
                .next()
                .expect("rollout path lookup returns at least one path or errors"),
            Err(error) if is_missing_rollout_error(&error) => {
                missing_rollout_by_session.insert(session_id.clone(), error);
                skipped_rollout_count += 1;
                continue;
            }
            Err(error) => return Err(error),
        };
        scanned_rollout_count += 1;
        activity_by_session.insert(
            session_id.clone(),
            read_codex_rollout_activity_report(&path, now)?,
        );

        for child_session_id in read_spawned_agent_ids(&path)? {
            if !seen_session_ids.contains(&child_session_id) {
                queue.push_back(child_session_id);
            }
        }

        let Some(metadata) = read_codex_rollout_metadata_any(&path)? else {
            skipped_rollout_count += 1;
            continue;
        };

        if metadata.session_id != root_session_id {
            if metadata.root_session_id.as_deref() != Some(root_session_id) {
                skipped_rollout_count += 1;
                continue;
            }
            let replace = latest_by_session
                .get(&metadata.session_id)
                .map(|existing| codex_rollout_metadata_newer(&metadata, existing))
                .unwrap_or(true);
            if replace {
                latest_by_session.insert(metadata.session_id.clone(), metadata);
            }
        }
    }

    let mut records = latest_by_session.into_values().collect::<Vec<_>>();
    records.sort_by(|left, right| {
        left.rollout_created_at_unix
            .cmp(&right.rollout_created_at_unix)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });

    Ok(Some(CodexRolloutSessionIndex {
        root_session_id: root_session_id.to_string(),
        sessions_dir,
        scanned_rollout_count,
        skipped_rollout_count,
        records,
        activity_by_session,
        missing_rollout_by_session,
    }))
}

fn is_missing_rollout_error(error: &str) -> bool {
    error.starts_with("Codex rollout invariant broken: no rollout JSONL found for session ")
}

pub(crate) fn codex_rollout_paths_for_session_id(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = rg_rollout_paths_for_session_id(sessions_dir, session_id)?;
    if paths.is_empty() {
        return Err(format!(
            "Codex rollout invariant broken: no rollout JSONL found for session {session_id} under {}",
            sessions_dir.display()
        ));
    }
    paths.retain(|path| path.is_file());
    paths.sort();
    paths.dedup();
    paths.reverse();
    Ok(paths)
}

fn rg_rollout_paths_for_session_id(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let glob = format!("**/rollout-*{session_id}.jsonl");
    let output = match Command::new("rg")
        .arg("--files")
        .arg("--glob")
        .arg(glob)
        .arg(sessions_dir)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "failed to run rg for Codex sessions dir {}: {error}",
                sessions_dir.display()
            ));
        }
    };
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(format!(
            "rg failed while locating Codex rollout for session {session_id}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let path = PathBuf::from(line);
            if path.is_absolute() {
                path
            } else {
                sessions_dir.join(path)
            }
        })
        .collect())
}

fn read_spawned_agent_ids(path: &Path) -> Result<Vec<String>, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open Codex rollout {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut ids = Vec::new();
    for line in reader.lines() {
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
        let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
        if value.get("type").and_then(serde_json::Value::as_str) != Some("response_item")
            || string_at(payload, "/type").as_deref() != Some("function_call_output")
        {
            continue;
        }
        let Some(output) = string_at(payload, "/output") else {
            continue;
        };
        let Ok(output_json) = serde_json::from_str::<serde_json::Value>(&output) else {
            continue;
        };
        if let Some(agent_id) = string_at(&output_json, "/agent_id") {
            ids.push(agent_id);
        }
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn read_codex_rollout_metadata_any(
    path: &Path,
) -> Result<Option<CodexRolloutSessionMetadata>, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open Codex rollout {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut metadata = None::<CodexRolloutSessionMetadata>;

    for line in reader.lines() {
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
                let Some(session_id) =
                    string_at(payload, "/id").or_else(|| string_at(payload, "/session_id"))
                else {
                    continue;
                };
                let mut parsed = CodexRolloutSessionMetadata {
                    session_id,
                    rollout_path: path.to_path_buf(),
                    rollout_created_at_unix: path_unix_timestamp(path)?,
                    root_session_id: string_at(payload, "/session_id"),
                    parent_thread_id: string_at(payload, "/parent_thread_id").or_else(|| {
                        string_at(payload, "/source/subagent/thread_spawn/parent_thread_id")
                    }),
                    thread_source: string_at(payload, "/thread_source"),
                    agent_role: string_at(payload, "/agent_role")
                        .or_else(|| string_at(payload, "/source/subagent/thread_spawn/agent_role")),
                    agent_nickname: string_at(payload, "/agent_nickname").or_else(|| {
                        string_at(payload, "/source/subagent/thread_spawn/agent_nickname")
                    }),
                    agent_path: string_at(payload, "/source/subagent/thread_spawn/agent_path"),
                    spawn_depth: i64_at(payload, "/source/subagent/thread_spawn/depth"),
                    model_provider: string_at(payload, "/model_provider"),
                    cli_version: string_at(payload, "/cli_version"),
                    cwd: string_at(payload, "/cwd"),
                    model: None,
                    collaboration_model: None,
                    sandbox_policy: None,
                    approval_policy: None,
                    permission_profile: None,
                };
                if parsed.root_session_id.is_none() {
                    parsed.root_session_id = Some(parsed.session_id.clone());
                }
                metadata = Some(parsed);
            }
            Some("turn_context") => {
                let Some(metadata) = metadata.as_mut() else {
                    continue;
                };
                let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
                if let Some(model) = string_at(payload, "/model") {
                    metadata.model = Some(model);
                }
                if let Some(collaboration_model) = string_at(payload, "/collaboration_model") {
                    metadata.collaboration_model = Some(collaboration_model);
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

    Ok(metadata)
}

fn read_codex_rollout_activity_report(
    path: &Path,
    now: i64,
) -> Result<CodexRolloutActivityReport, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open Codex rollout {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut state = CodexRolloutActivityState::new(path.to_path_buf());

    for line in reader.lines() {
        let line = line.map_err(|error| {
            format!(
                "failed to read Codex rollout line from {}: {error}",
                path.display()
            )
        })?;
        state.scanned_line_count += 1;
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        state.observe(&value);
    }

    if state.last_event_at.is_none() {
        state.last_event_at = path_unix_timestamp(path)?;
    }
    if state.last_heartbeat_at.is_none() {
        state.last_heartbeat_at = state.last_event_at;
    }
    Ok(state.finish(now))
}

struct CodexRolloutActivityState {
    rollout_path: PathBuf,
    last_event_at: Option<i64>,
    last_event_kind: Option<String>,
    last_heartbeat_at: Option<i64>,
    last_heartbeat_kind: Option<String>,
    recent_heartbeats: Vec<CodexRolloutActivityHeartbeat>,
    current_turn_id: Option<String>,
    last_running_session_id: Option<String>,
    running_session_closed: bool,
    last_terminal_event: Option<String>,
    agent_instruction: Option<String>,
    scanned_line_count: usize,
}

impl CodexRolloutActivityState {
    fn new(rollout_path: PathBuf) -> Self {
        Self {
            rollout_path,
            last_event_at: None,
            last_event_kind: None,
            last_heartbeat_at: None,
            last_heartbeat_kind: None,
            recent_heartbeats: Vec::new(),
            current_turn_id: None,
            last_running_session_id: None,
            running_session_closed: false,
            last_terminal_event: None,
            agent_instruction: None,
            scanned_line_count: 0,
        }
    }

    fn observe(&mut self, value: &serde_json::Value) {
        let Some(kind) = rollout_event_kind(value) else {
            return;
        };
        let at = rollout_event_timestamp(value);
        self.last_event_at = at.or(self.last_event_at);
        self.last_event_kind = Some(kind.clone());
        if let Some(turn_id) = rollout_turn_id(value) {
            self.current_turn_id = Some(turn_id);
        }
        if let Some(instruction) = rollout_agent_instruction(value) {
            self.agent_instruction = Some(instruction);
        }
        if let Some(agent_id) = spawned_agent_id_from_event(value) {
            self.last_running_session_id = Some(agent_id);
            self.running_session_closed = false;
        }
        if let Some(terminal_event) = rollout_terminal_event(value) {
            self.last_terminal_event = Some(terminal_event);
            self.running_session_closed = self.last_running_session_id.is_some();
        }
        if rollout_event_is_heartbeat(value) {
            self.last_heartbeat_at = at.or(self.last_heartbeat_at);
            self.last_heartbeat_kind = Some(kind.clone());
            self.recent_heartbeats.push(CodexRolloutActivityHeartbeat {
                at,
                kind,
                turn_id: self.current_turn_id.clone(),
            });
            if self.recent_heartbeats.len() > 3 {
                self.recent_heartbeats.remove(0);
            }
        }
    }

    fn finish(self, now: i64) -> CodexRolloutActivityReport {
        let seconds_since_heartbeat = self.last_heartbeat_at.map(|at| now.saturating_sub(at));
        let status = if self.last_terminal_event.is_some() {
            "closed"
        } else if seconds_since_heartbeat.is_some_and(|seconds| seconds > 300) {
            "silent"
        } else {
            "active"
        };
        CodexRolloutActivityReport {
            status: status.to_string(),
            rollout_path: self.rollout_path,
            last_event_at: self.last_event_at,
            last_event_kind: self.last_event_kind,
            last_heartbeat_at: self.last_heartbeat_at,
            last_heartbeat_kind: self.last_heartbeat_kind,
            recent_heartbeats: self.recent_heartbeats,
            seconds_since_heartbeat,
            current_turn_id: self.current_turn_id,
            last_running_session_id: self.last_running_session_id,
            running_session_closed: self.running_session_closed,
            last_terminal_event: self.last_terminal_event,
            agent_instruction: self.agent_instruction,
            scanned_line_count: self.scanned_line_count,
        }
    }
}

fn rollout_event_kind(value: &serde_json::Value) -> Option<String> {
    value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn rollout_event_timestamp(value: &serde_json::Value) -> Option<i64> {
    i64_at(value, "/timestamp")
        .or_else(|| i64_at(value, "/ts"))
        .or_else(|| i64_at(value, "/payload/timestamp"))
        .or_else(|| i64_at(value, "/payload/ts"))
}

fn rollout_turn_id(value: &serde_json::Value) -> Option<String> {
    string_at(value, "/payload/turn_id")
        .or_else(|| string_at(value, "/payload/turnId"))
        .or_else(|| string_at(value, "/payload/id"))
}

fn rollout_agent_instruction(value: &serde_json::Value) -> Option<String> {
    string_at(value, "/payload/instructions")
        .or_else(|| string_at(value, "/payload/agent_instruction"))
        .or_else(|| string_at(value, "/payload/agentInstruction"))
}

fn spawned_agent_id_from_event(value: &serde_json::Value) -> Option<String> {
    if value.get("type").and_then(serde_json::Value::as_str) != Some("response_item") {
        return None;
    }
    let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
    if string_at(payload, "/type").as_deref() != Some("function_call_output") {
        return None;
    }
    let output = string_at(payload, "/output")?;
    let output_json = serde_json::from_str::<serde_json::Value>(&output).ok()?;
    string_at(&output_json, "/agent_id")
}

fn rollout_terminal_event(value: &serde_json::Value) -> Option<String> {
    let kind = value.get("type").and_then(serde_json::Value::as_str)?;
    let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
    let status = string_at(payload, "/status")
        .or_else(|| string_at(payload, "/state"))
        .unwrap_or_default();
    if matches!(
        status.as_str(),
        "closed" | "complete" | "completed" | "failed" | "cancelled"
    ) {
        return Some(format!("{kind}:{status}"));
    }
    if matches!(kind, "session_closed" | "thread_closed" | "turn_cancelled") {
        return Some(kind.to_string());
    }
    None
}

fn rollout_event_is_heartbeat(value: &serde_json::Value) -> bool {
    matches!(
        value.get("type").and_then(serde_json::Value::as_str),
        Some("session_meta" | "turn_context" | "event_msg" | "response_item")
    )
}

fn codex_sessions_dir() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(home).join("sessions"));
    }
    let Some(home) = env::var_os("HOME") else {
        return Err("CODEX_HOME and HOME are unset; cannot locate Codex sessions".to_string());
    };
    Ok(PathBuf::from(home).join(".codex").join("sessions"))
}

fn codex_rollout_metadata_newer(
    candidate: &CodexRolloutSessionMetadata,
    existing: &CodexRolloutSessionMetadata,
) -> bool {
    candidate
        .rollout_created_at_unix
        .cmp(&existing.rollout_created_at_unix)
        .then_with(|| candidate.rollout_path.cmp(&existing.rollout_path))
        .is_gt()
}

fn path_unix_timestamp(path: &Path) -> Result<Option<i64>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to stat Codex rollout {}: {error}", path.display()))?;
    let modified = metadata.modified().map_err(|error| {
        format!(
            "failed to read modified time for Codex rollout {}: {error}",
            path.display()
        )
    })?;
    let duration = modified.duration_since(UNIX_EPOCH).map_err(|error| {
        format!(
            "Codex rollout {} has invalid modified time: {error}",
            path.display()
        )
    })?;
    Ok(Some(duration.as_secs() as i64))
}

fn current_unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before UNIX_EPOCH: {error}"))?;
    Ok(duration.as_secs() as i64)
}

fn string_at(value: &serde_json::Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn i64_at(value: &serde_json::Value, pointer: &str) -> Option<i64> {
    value.pointer(pointer).and_then(serde_json::Value::as_i64)
}
