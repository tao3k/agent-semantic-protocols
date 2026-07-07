//! Codex rollout JSONL session index parser.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CodexRolloutSessionMetadata;

const ROLLOUT_INDEX_HEADER_LINE_LIMIT: usize = 32;
const ROLLOUT_INDEX_ACTIVITY_TAIL_BYTES: u64 = 256 * 1024;

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

pub(crate) fn codex_rollout_paths_for_session_id(
    sessions_dir: &Path,
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let search_roots = rollout_search_roots_for_session_id(sessions_dir, session_id);
    let mut paths = direct_rollout_paths_for_session_id(&search_roots, session_id)?;
    if paths.is_empty() {
        for search_root in &search_roots {
            if search_root.is_dir() {
                paths.extend(rg_rollout_paths_for_session_id(search_root, session_id)?);
            }
        }
    }
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

fn direct_rollout_paths_for_session_id(
    search_roots: &[PathBuf],
    session_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let suffix = format!("{session_id}.jsonl");
    let mut paths = Vec::new();
    for search_root in search_roots {
        if !search_root.is_dir() {
            continue;
        }
        for entry in fs::read_dir(search_root)
            .map_err(|error| format!("failed to read {}: {error}", search_root.display()))?
        {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read Codex session entry below {}: {error}",
                    search_root.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "failed to inspect Codex session entry {}: {error}",
                    path.display()
                )
            })?;
            if !file_type.is_file() {
                continue;
            }
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if file_name.starts_with("rollout-") && file_name.ends_with(&suffix) {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

fn rollout_search_roots_for_session_id(sessions_dir: &Path, session_id: &str) -> Vec<PathBuf> {
    let Some(unix_day) = uuid_v7_unix_day(session_id) else {
        return vec![sessions_dir.to_path_buf()];
    };
    (-1..=1)
        .map(|offset| rollout_date_dir(sessions_dir, unix_day + offset))
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

fn rollout_date_dir(sessions_dir: &Path, unix_day: i64) -> PathBuf {
    let (year, month, day) = civil_from_unix_day(unix_day);
    sessions_dir
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

/// Build a root-scoped Codex rollout session index.
pub fn codex_rollout_session_index(
    root_session_id: &str,
) -> Result<Option<CodexRolloutSessionIndex>, String> {
    codex_rollout_session_index_for_sessions(root_session_id, std::iter::empty::<&str>())
}

/// Build a root-scoped Codex rollout session index, including known child sessions.
pub fn codex_rollout_session_index_for_sessions<'a, I>(
    root_session_id: &str,
    session_ids: I,
) -> Result<Option<CodexRolloutSessionIndex>, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let sessions_dir = codex_sessions_dir()?;
    if !sessions_dir.is_dir() {
        return Ok(None);
    }
    let mut child_session_ids: BTreeSet<String> = session_ids
        .into_iter()
        .filter(|session_id| !session_id.is_empty() && *session_id != root_session_id)
        .map(str::to_string)
        .collect();
    let mut rollout_paths = if child_session_ids.is_empty() {
        match codex_rollout_paths_for_session_id(&sessions_dir, root_session_id) {
            Ok(paths) => paths,
            Err(error)
                if error.starts_with("Codex rollout invariant broken: no rollout JSONL found") =>
            {
                Vec::new()
            }
            Err(error) => return Err(error),
        }
    } else {
        Vec::new()
    };
    let mut missing_rollout_by_session = BTreeMap::new();
    let mut pending_session_ids = child_session_ids.iter().cloned().collect::<Vec<_>>();
    let mut processed_session_ids = BTreeSet::new();
    let mut pending_spawn_rollout_paths = rollout_paths.clone();
    let mut processed_spawn_rollout_paths = BTreeSet::new();

    while !(pending_session_ids.is_empty() && pending_spawn_rollout_paths.is_empty()) {
        while let Some(child_session_id) = pending_session_ids.pop() {
            if !processed_session_ids.insert(child_session_id.clone()) {
                continue;
            }
            match codex_rollout_paths_for_session_id(&sessions_dir, &child_session_id) {
                Ok(paths) => {
                    for path in paths {
                        if !rollout_paths.iter().any(|existing| existing == &path) {
                            pending_spawn_rollout_paths.push(path.clone());
                            rollout_paths.push(path);
                        }
                    }
                }
                Err(error)
                    if error
                        .starts_with("Codex rollout invariant broken: no rollout JSONL found") =>
                {
                    missing_rollout_by_session.insert(child_session_id, error);
                }
                Err(error) => return Err(error),
            }
        }

        let Some(rollout_path) = pending_spawn_rollout_paths.pop() else {
            continue;
        };
        if !processed_spawn_rollout_paths.insert(rollout_path.clone()) {
            continue;
        }
        let mut spawned_child_session_ids =
            thread_spawn_child_session_ids_for_rollout(&rollout_path, root_session_id)?;
        spawned_child_session_ids.extend(spawned_agent_ids_for_rollout(&rollout_path)?);
        for child_session_id in spawned_child_session_ids {
            if child_session_ids.insert(child_session_id.clone()) {
                pending_session_ids.push(child_session_id);
            }
        }
    }

    for child_session_id in child_session_ids {
        if processed_session_ids.contains(&child_session_id) {
            continue;
        }
        match codex_rollout_paths_for_session_id(&sessions_dir, &child_session_id) {
            Ok(paths) => rollout_paths.extend(paths),
            Err(error)
                if error.starts_with("Codex rollout invariant broken: no rollout JSONL found") =>
            {
                missing_rollout_by_session.insert(child_session_id, error);
            }
            Err(error) => return Err(error),
        }
    }
    rollout_paths.sort();
    rollout_paths.dedup();
    let mut records = Vec::new();
    let mut activity_by_session = BTreeMap::new();
    let mut scanned_rollout_count = 0_usize;
    let mut skipped_rollout_count = 0_usize;
    for rollout_path in rollout_paths {
        let Some((metadata, activity)) = parse_rollout_file_at_path(&rollout_path)? else {
            skipped_rollout_count += 1;
            continue;
        };
        if metadata.root_session_id.as_deref() != Some(root_session_id)
            && metadata.session_id != root_session_id
            && metadata.parent_thread_id.as_deref() != Some(root_session_id)
        {
            skipped_rollout_count += 1;
            continue;
        }
        scanned_rollout_count += 1;
        activity_by_session.insert(metadata.session_id.clone(), activity);
        if metadata.session_id == root_session_id {
            continue;
        }
        records.push(metadata);
    }
    if records.is_empty() {
        return Ok(None);
    }
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

fn thread_spawn_child_session_ids_for_rollout(
    rollout_path: &Path,
    root_session_id: &str,
) -> Result<Vec<String>, String> {
    let file = match fs::File::open(rollout_path) {
        Ok(file) => file,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut child_session_ids = BTreeSet::new();
    loop {
        line.clear();
        let read = reader.read_until(b'\n', &mut line).map_err(|error| {
            format!(
                "failed to read Codex rollout thread_spawn lines from {}: {error}",
                rollout_path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        if !bytes_contain(&line, b"thread_spawn") {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("thread_spawn") {
            continue;
        }
        let parent_matches = match payload.get("parent_thread_id").and_then(Value::as_str) {
            Some(parent_thread_id) => parent_thread_id == root_session_id,
            None => true,
        };
        if !parent_matches {
            continue;
        }
        if let Some(child_session_id) = payload.get("id").and_then(Value::as_str) {
            child_session_ids.insert(child_session_id.to_string());
        }
    }
    Ok(child_session_ids.into_iter().collect())
}

fn bytes_contain(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack.len() >= needle.len()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn spawned_agent_ids_for_rollout(rollout_path: &Path) -> Result<Vec<String>, String> {
    let lines = rollout_index_sample_lines(rollout_path)?;
    let mut ids = BTreeSet::new();
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("function_call_output") {
            continue;
        }
        let Some(output) = payload.get("output").and_then(Value::as_str) else {
            continue;
        };
        let Ok(output_json) = serde_json::from_str::<Value>(output) else {
            continue;
        };
        if let Some(agent_id) = output_json.get("agent_id").and_then(Value::as_str) {
            ids.insert(agent_id.to_string());
        }
    }
    Ok(ids.into_iter().collect())
}

fn parse_rollout_file_at_path(
    rollout_path: &Path,
) -> Result<
    Option<(
        crate::CodexRolloutSessionMetadata,
        CodexRolloutActivityReport,
    )>,
    String,
> {
    let lines = rollout_index_sample_lines(rollout_path)?;
    let line_refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
    parse_rollout_file(rollout_path, &line_refs)
}

fn rollout_index_sample_lines(rollout_path: &Path) -> Result<Vec<String>, String> {
    let metadata = match fs::metadata(rollout_path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(Vec::new()),
    };
    if metadata.len() <= ROLLOUT_INDEX_ACTIVITY_TAIL_BYTES {
        let text = match fs::read_to_string(rollout_path) {
            Ok(text) => text,
            Err(_) => return Ok(Vec::new()),
        };
        return Ok(text.lines().map(str::to_string).collect());
    }

    let file = match fs::File::open(rollout_path) {
        Ok(file) => file,
        Err(_) => return Ok(Vec::new()),
    };
    let reader = BufReader::new(file);
    let mut lines = Vec::new();
    for line in reader.lines().take(ROLLOUT_INDEX_HEADER_LINE_LIMIT) {
        let Ok(line) = line else {
            continue;
        };
        lines.push(line);
    }

    let mut file = match fs::File::open(rollout_path) {
        Ok(file) => file,
        Err(_) => return Ok(lines),
    };
    let tail_start = metadata
        .len()
        .saturating_sub(ROLLOUT_INDEX_ACTIVITY_TAIL_BYTES);
    if file.seek(SeekFrom::Start(tail_start)).is_err() {
        return Ok(lines);
    }
    let mut tail_bytes = Vec::new();
    if file.read_to_end(&mut tail_bytes).is_err() {
        return Ok(lines);
    }
    let tail_text = String::from_utf8_lossy(&tail_bytes);
    let mut tail_lines = tail_text.lines();
    if tail_start > 0 {
        tail_lines.next();
    }
    lines.extend(tail_lines.map(str::to_string));
    Ok(lines)
}

fn codex_sessions_dir() -> Result<PathBuf, String> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(codex_home).join("sessions"));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".codex").join("sessions"))
        .ok_or_else(|| "HOME is not set; cannot locate Codex sessions".to_string())
}

fn first_json_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
        .map(str::to_string)
}

fn parse_rollout_file(
    rollout_path: &Path,
    lines: &[&str],
) -> Result<
    Option<(
        crate::CodexRolloutSessionMetadata,
        CodexRolloutActivityReport,
    )>,
    String,
> {
    let mut metadata = None;
    let mut activity = None;
    let mut last_running_session_id = None;
    let mut current_turn_id = None;
    let mut last_terminal_event = None;
    let mut line_count = 0_usize;
    for line in lines {
        line_count += 1;
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                let payload = value.get("payload").unwrap_or(&Value::Null);
                let Some(session_id) = payload
                    .pointer("/id")
                    .and_then(Value::as_str)
                    .or_else(|| payload.pointer("/session_id").and_then(Value::as_str))
                    .map(str::to_string)
                else {
                    continue;
                };
                let rollout_created_at_unix = fs::metadata(rollout_path)
                    .ok()
                    .and_then(|meta| meta.created().or_else(|_| meta.modified()).ok())
                    .and_then(|time| {
                        time.duration_since(UNIX_EPOCH)
                            .ok()
                            .map(|duration| duration.as_secs() as i64)
                    });
                metadata = Some(crate::CodexRolloutSessionMetadata {
                    session_id,
                    rollout_path: rollout_path.to_path_buf(),
                    rollout_created_at_unix,
                    root_session_id: first_json_string(
                        payload,
                        &[
                            "/rootSessionId",
                            "/root_session_id",
                            "/session_id",
                            "/sourceSessionId",
                            "/source_session_id",
                            "/source/rootSessionId",
                            "/source/root_session_id",
                            "/source/session_id",
                            "/source/sourceSessionId",
                            "/source/source_session_id",
                            "/source/subagent/threadSpawn/rootSessionId",
                            "/source/subagent/thread_spawn/root_session_id",
                        ],
                    ),
                    parent_thread_id: first_json_string(
                        payload,
                        &[
                            "/parentThreadId",
                            "/parent_thread_id",
                            "/threadSpawnParentId",
                            "/thread_spawn_parent_id",
                            "/source/threadSpawnParentId",
                            "/source/thread_spawn_parent_id",
                            "/source/threadSpawn/parentThreadId",
                            "/source/thread_spawn/parent_thread_id",
                            "/source/subagent/threadSpawnParentId",
                            "/source/subagent/thread_spawn_parent_id",
                            "/source/subagent/threadSpawn/parentThreadId",
                            "/source/subagent/thread_spawn/parent_thread_id",
                        ],
                    ),
                    thread_source: payload
                        .pointer("/thread_source")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    agent_role: payload
                        .pointer("/agent_role")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| {
                            payload
                                .pointer("/source/subagent/thread_spawn/agent_role")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        }),
                    agent_nickname: payload
                        .pointer("/agent_nickname")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| {
                            payload
                                .pointer("/source/subagent/thread_spawn/agent_nickname")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                        }),
                    agent_path: payload
                        .pointer("/source/subagent/thread_spawn/agent_path")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    spawn_depth: payload
                        .pointer("/source/subagent/thread_spawn/depth")
                        .and_then(Value::as_i64),
                    model_provider: payload
                        .pointer("/model_provider")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    cli_version: payload
                        .pointer("/cli_version")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    cwd: payload
                        .pointer("/cwd")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    model: None,
                    collaboration_model: None,
                    sandbox_policy: None,
                    approval_policy: None,
                    permission_profile: None,
                });
            }
            Some("turn_context") => {
                if let Some(existing) = metadata.as_mut() {
                    let payload = value.get("payload").unwrap_or(&Value::Null);
                    existing.model = payload
                        .pointer("/model")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    existing.collaboration_model = payload
                        .pointer("/collaboration_model")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    existing.sandbox_policy = payload
                        .pointer("/sandbox_policy/type")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    existing.approval_policy = payload
                        .pointer("/approval_policy")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    existing.permission_profile = payload
                        .pointer("/permission_profile/type")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            Some("response_item") => {
                let payload = value.get("payload").unwrap_or(&Value::Null);
                if let Some(output) = payload.get("output").and_then(Value::as_str)
                    && let Ok(output_value) = serde_json::from_str::<Value>(output)
                    && let Some(agent_id) = output_value.get("agent_id").and_then(Value::as_str)
                {
                    last_running_session_id = Some(agent_id.to_string());
                }
            }
            Some("event_msg") => {
                let payload = value.get("payload").unwrap_or(&Value::Null);
                if let Some(status) = payload.get("status").and_then(Value::as_str)
                    && status == "closed"
                {
                        last_terminal_event = Some("event_msg:closed".to_string());
                        current_turn_id = payload
                            .get("turn_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or(current_turn_id);
                        activity = Some(CodexRolloutActivityReport {
                            status: "closed".to_string(),
                            rollout_path: rollout_path.to_path_buf(),
                            last_event_at: payload.get("timestamp").and_then(Value::as_i64),
                            last_event_kind: Some("event_msg".to_string()),
                            last_heartbeat_at: payload.get("timestamp").and_then(Value::as_i64),
                            last_heartbeat_kind: Some("event_msg".to_string()),
                            recent_heartbeats: Vec::new(),
                            seconds_since_heartbeat: None,
                            current_turn_id: current_turn_id.clone(),
                            last_running_session_id: last_running_session_id.clone(),
                            running_session_closed: true,
                            last_terminal_event: last_terminal_event.clone(),
                            agent_instruction: None,
                            scanned_line_count: line_count,
                        });
                }
            }
            _ => {}
        }
    }
    let Some(metadata) = metadata else {
        return Ok(None);
    };
    let activity = activity.unwrap_or_else(|| CodexRolloutActivityReport {
        status: "active".to_string(),
        rollout_path: rollout_path.to_path_buf(),
        last_event_at: None,
        last_event_kind: None,
        last_heartbeat_at: None,
        last_heartbeat_kind: None,
        recent_heartbeats: Vec::new(),
        seconds_since_heartbeat: None,
        current_turn_id,
        last_running_session_id,
        running_session_closed: false,
        last_terminal_event,
        agent_instruction: None,
        scanned_line_count: line_count,
    });
    Ok(Some((metadata, activity)))
}
