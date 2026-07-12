//! Codex rollout JSONL lookup and header parsing.

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const ROLLOUT_SESSION_META_HEAD_BYTES: u64 = 128 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexRolloutSessionMeta {
    #[serde(rename = "rolloutPath")]
    pub(crate) rollout_path: String,
    #[serde(rename = "eventTimestamp", skip_serializing_if = "Option::is_none")]
    pub(crate) event_timestamp: Option<String>,
    #[serde(rename = "childSessionId", skip_serializing_if = "Option::is_none")]
    pub(crate) child_session_id: Option<String>,
    #[serde(rename = "sourceSessionId", skip_serializing_if = "Option::is_none")]
    pub(crate) source_session_id: Option<String>,
    #[serde(rename = "parentThreadId", skip_serializing_if = "Option::is_none")]
    pub(crate) parent_thread_id: Option<String>,
    #[serde(
        rename = "threadSpawnParentId",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) thread_spawn_parent_id: Option<String>,
    #[serde(rename = "subagentDepth", skip_serializing_if = "Option::is_none")]
    pub(crate) subagent_depth: Option<u64>,
    #[serde(rename = "agentNickname", skip_serializing_if = "Option::is_none")]
    pub(crate) agent_nickname: Option<String>,
    #[serde(rename = "agentRole", skip_serializing_if = "Option::is_none")]
    pub(crate) agent_role: Option<String>,
    #[serde(rename = "agentPath", skip_serializing_if = "Option::is_none")]
    pub(crate) agent_path: Option<String>,
    #[serde(rename = "cwd", skip_serializing_if = "Option::is_none")]
    pub(crate) cwd: Option<String>,
    #[serde(rename = "originator", skip_serializing_if = "Option::is_none")]
    pub(crate) originator: Option<String>,
    #[serde(rename = "cliVersion", skip_serializing_if = "Option::is_none")]
    pub(crate) cli_version: Option<String>,
    #[serde(rename = "relationshipKind")]
    pub(crate) relationship_kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexRolloutSessionActivity {
    pub(crate) status: String,
    #[serde(rename = "lastEventAt", skip_serializing_if = "Option::is_none")]
    pub(crate) last_event_at: Option<String>,
    #[serde(rename = "lastEventKind", skip_serializing_if = "Option::is_none")]
    pub(crate) last_event_kind: Option<String>,
    #[serde(rename = "lastTerminalEvent", skip_serializing_if = "Option::is_none")]
    pub(crate) last_terminal_event: Option<String>,
    #[serde(rename = "turnCompleteResumable")]
    pub(crate) turn_complete_resumable: bool,
    #[serde(rename = "turnRunningOrActive")]
    pub(crate) turn_running_or_active: bool,
    #[serde(rename = "pendingToolCallCount")]
    pub(crate) pending_tool_call_count: usize,
    #[serde(rename = "scannedBytes")]
    pub(crate) scanned_bytes: usize,
}

#[derive(Default)]
pub(crate) struct CodexRolloutSessionActivityState {
    last_event_at: Option<String>,
    last_event_kind: Option<String>,
    last_terminal_event: Option<String>,
    pending_tool_calls: BTreeSet<String>,
    observed_open_turn: bool,
    scanned_bytes: usize,
}

impl CodexRolloutSessionActivityState {
    pub(crate) fn observe_event(&mut self, value: &Value, scanned_bytes: usize) {
        self.scanned_bytes += scanned_bytes;
        let Some(kind) = codex_event_kind(value) else {
            return;
        };
        self.last_event_at = json_string_pointer(value, "/timestamp");
        if is_terminal_event_kind(&kind) {
            self.last_terminal_event = Some(kind.clone());
            self.pending_tool_calls.clear();
            self.observed_open_turn = false;
        } else if kind == "task_started" {
            self.observed_open_turn = true;
        } else if kind == "function_call" {
            if let Some(call_id) = json_string_pointer(value, "/payload/call_id") {
                self.pending_tool_calls.insert(call_id);
            }
            self.observed_open_turn = true;
        } else if kind == "function_call_output" {
            if let Some(call_id) = json_string_pointer(value, "/payload/call_id") {
                self.pending_tool_calls.remove(&call_id);
            }
            self.observed_open_turn = true;
        } else if self.last_event_kind.is_some() {
            self.observed_open_turn = true;
        }
        self.last_event_kind = Some(kind);
    }

    pub(crate) fn finish(&self) -> CodexRolloutSessionActivity {
        let turn_complete_resumable = self
            .last_event_kind
            .as_deref()
            .is_some_and(is_terminal_event_kind);
        let pending_tool_call_count = self.pending_tool_calls.len();
        let turn_running_or_active =
            pending_tool_call_count > 0 || (self.observed_open_turn && !turn_complete_resumable);
        let status = if pending_tool_call_count > 0 {
            "tool-running"
        } else if turn_complete_resumable {
            "idle-resumable"
        } else if self.observed_open_turn {
            "agent-active"
        } else {
            "silent"
        };

        CodexRolloutSessionActivity {
            status: status.to_string(),
            last_event_at: self.last_event_at.clone(),
            last_event_kind: self.last_event_kind.clone(),
            last_terminal_event: self.last_terminal_event.clone(),
            turn_complete_resumable,
            turn_running_or_active,
            pending_tool_call_count,
            scanned_bytes: self.scanned_bytes,
        }
    }
}

pub(crate) fn fast_rollout_path_for_session_id(session_id: &str) -> Option<PathBuf> {
    let codex_sessions_dir = codex_sessions_dir()?;
    fast_rollout_path_for_session_id_in(&codex_sessions_dir, session_id)
}

#[derive(Clone, Debug)]
pub(crate) enum CodexRolloutSessionLiveness {
    Resumable(CodexRolloutSessionActivity),
    Active(CodexRolloutSessionActivity),
    Unknown(CodexRolloutSessionActivity),
    Missing,
    Unavailable(String),
}

/// Resolve one known session through its UUID-v7 date bucket, then parse its
/// recent JSONL events. A non-terminal tail is deliberately conservative: it
/// never authorizes replacement creation.
pub(crate) fn rollout_session_liveness_for_session_id(
    session_id: &str,
) -> CodexRolloutSessionLiveness {
    let Some(path) = fast_rollout_path_for_session_id(session_id) else {
        return CodexRolloutSessionLiveness::Missing;
    };
    rollout_session_liveness_at_path(&path)
}

#[cfg(test)]
pub(crate) fn rollout_session_liveness_for_session_id_in(
    codex_sessions_dir: &Path,
    session_id: &str,
) -> CodexRolloutSessionLiveness {
    let Some(path) = fast_rollout_path_for_session_id_in(codex_sessions_dir, session_id) else {
        return CodexRolloutSessionLiveness::Missing;
    };
    rollout_session_liveness_at_path(&path)
}

fn rollout_session_liveness_at_path(path: &Path) -> CodexRolloutSessionLiveness {
    let activity = match rollout_session_activity(path) {
        Ok(activity) => activity,
        Err(error) => return CodexRolloutSessionLiveness::Unavailable(error),
    };
    if activity.turn_complete_resumable {
        CodexRolloutSessionLiveness::Resumable(activity)
    } else if activity.turn_running_or_active {
        CodexRolloutSessionLiveness::Active(activity)
    } else {
        CodexRolloutSessionLiveness::Unknown(activity)
    }
}

fn rollout_session_activity(path: &Path) -> Result<CodexRolloutSessionActivity, String> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    const ACTIVITY_TAIL_BYTES: u64 = 64 * 1024;

    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("failed to open rollout {}: {error}", path.display()))?;
    let len = file
        .metadata()
        .map_err(|error| format!("failed to stat rollout {}: {error}", path.display()))?
        .len();
    let start = len.saturating_sub(ACTIVITY_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("failed to seek rollout {}: {error}", path.display()))?;
    let mut tail = String::new();
    file.read_to_string(&mut tail)
        .map_err(|error| format!("failed to read rollout {}: {error}", path.display()))?;
    if start > 0
        && let Some(index) = tail.find('\n')
    {
        tail = tail[index + 1..].to_string();
    }

    let mut state = CodexRolloutSessionActivityState::default();
    for line in tail.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        state.observe_event(&value, line.len() + 1);
    }
    Ok(state.finish())
}

pub(crate) fn fast_rollout_path_for_session_id_in(
    codex_sessions_dir: &Path,
    session_id: &str,
) -> Option<PathBuf> {
    if !codex_sessions_dir.is_dir() || session_id.trim().is_empty() {
        return None;
    }

    let filename_matcher = rollout_filename_matcher(session_id)?;
    for search_root in rollout_search_roots_for_session_id(codex_sessions_dir, session_id) {
        let Ok(entries) = fs::read_dir(&search_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_file() && is_rollout_filename_match(&path, &filename_matcher) {
                return Some(path);
            }
        }
    }

    None
}

fn rollout_search_roots_for_session_id(
    codex_sessions_dir: &Path,
    session_id: &str,
) -> Vec<PathBuf> {
    let Some(unix_day) = uuid_v7_unix_day(session_id) else {
        return Vec::new();
    };
    (-1..=1)
        .map(|offset| rollout_date_dir(codex_sessions_dir, unix_day + offset))
        .collect()
}

fn uuid_v7_unix_day(session_id: &str) -> Option<i64> {
    let timestamp_hex = session_id
        .chars()
        .filter(|character| *character != '-')
        .take(12)
        .collect::<String>();
    if timestamp_hex.len() != 12 {
        return None;
    }
    let unix_millis = i64::from_str_radix(&timestamp_hex, 16).ok()?;
    Some(unix_millis.div_euclid(86_400_000))
}

fn rollout_date_dir(codex_sessions_dir: &Path, unix_day: i64) -> PathBuf {
    let (year, month, day) = civil_from_unix_day(unix_day);
    codex_sessions_dir
        .join(format!("{year:04}"))
        .join(format!("{month:02}"))
        .join(format!("{day:02}"))
}

fn civil_from_unix_day(unix_day: i64) -> (i32, u32, u32) {
    let days = unix_day + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 }.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524
        - day_of_era / 146_096)
        .div_euclid(365);
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * month_prime + 2).div_euclid(5) + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    (year as i32, month as u32, day as u32)
}

pub(crate) fn rollout_session_meta(rollout_path: &Path) -> Option<CodexRolloutSessionMeta> {
    let head = read_rollout_head(rollout_path, ROLLOUT_SESSION_META_HEAD_BYTES).ok()?;
    for line in head.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("session_meta") {
            continue;
        }
        return Some(CodexRolloutSessionMeta::from_value(
            rollout_path.display().to_string(),
            &value,
        ));
    }
    None
}

impl CodexRolloutSessionMeta {
    fn from_value(rollout_path: String, value: &Value) -> Self {
        let thread_spawn = "/payload/source/subagent/thread_spawn";
        let relationship_kind = if value.pointer(thread_spawn).is_some() {
            "subagent"
        } else {
            "root"
        };
        Self {
            rollout_path,
            event_timestamp: json_string_pointer(value, "/timestamp"),
            child_session_id: json_string_pointer(value, "/payload/id"),
            source_session_id: json_string_pointer(value, "/payload/session_id"),
            parent_thread_id: json_string_pointer(value, "/payload/parent_thread_id"),
            thread_spawn_parent_id: json_string_pointer(
                value,
                &format!("{thread_spawn}/parent_thread_id"),
            ),
            subagent_depth: json_u64_pointer(value, &format!("{thread_spawn}/depth")),
            agent_nickname: json_string_pointer(value, &format!("{thread_spawn}/agent_nickname")),
            agent_role: json_string_pointer(value, &format!("{thread_spawn}/agent_role")),
            agent_path: json_string_pointer(value, &format!("{thread_spawn}/agent_path")),
            cwd: json_string_pointer(value, "/payload/cwd"),
            originator: json_string_pointer(value, "/payload/originator"),
            cli_version: json_string_pointer(value, "/payload/cli_version"),
            relationship_kind: relationship_kind.to_string(),
        }
    }
}

fn rollout_filename_matcher(session_id: &str) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(&format!("*{}*.jsonl", glob_literal(session_id))).ok()?);
    builder.build().ok()
}

fn is_rollout_filename_match(path: &Path, filename_matcher: &GlobSet) -> bool {
    path.file_name()
        .and_then(|file_name| file_name.to_str())
        .is_some_and(|file_name| filename_matcher.is_match(file_name))
}

fn glob_literal(value: &str) -> String {
    let mut literal = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '*' | '?' | '[' | ']' | '{' | '}' | '\\' => {
                literal.push('\\');
                literal.push(ch);
            }
            _ => literal.push(ch),
        }
    }
    literal
}

fn json_string_pointer(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn json_u64_pointer(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}

fn codex_event_kind(value: &Value) -> Option<String> {
    let top_level_kind = json_string_pointer(value, "/type")?;
    match top_level_kind.as_str() {
        "event_msg" => json_string_pointer(value, "/payload/type")
            .or_else(|| json_string_pointer(value, "/payload/event"))
            .or_else(|| json_string_pointer(value, "/payload/message"))
            .or(Some(top_level_kind)),
        "response_item" => json_string_pointer(value, "/payload/type")
            .or_else(|| json_string_pointer(value, "/payload/item/type"))
            .or(Some(top_level_kind)),
        _ => Some(top_level_kind),
    }
}

fn is_terminal_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "task_complete" | "turn_complete" | "agent_turn_complete" | "completed"
    )
}

fn read_rollout_head(path: &Path, max_bytes: u64) -> Result<String, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to open rollout {}: {error}", path.display()))?;
    let mut buffer = String::new();
    file.take(max_bytes)
        .read_to_string(&mut buffer)
        .map_err(|error| format!("failed to read rollout head {}: {error}", path.display()))?;
    Ok(buffer)
}

fn codex_sessions_dir() -> Option<PathBuf> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(codex_home).join("sessions"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex/sessions"))
}
