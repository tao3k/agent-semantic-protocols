//! Codex rollout JSONL session index parser.

use std::{
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
    time::UNIX_EPOCH,
};

use serde_json::Value;

use super::types::CodexRolloutActivityReport;


const ROLLOUT_INDEX_HEADER_LINE_LIMIT: usize = 32;
const ROLLOUT_INDEX_ACTIVITY_TAIL_BYTES: u64 = 256 * 1024;
pub(crate) fn parse_rollout_file_at_path(
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

pub(crate) fn rollout_index_sample_lines(rollout_path: &Path) -> Result<Vec<String>, String> {
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

pub(super) fn first_json_string(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
        .map(str::to_string)
}

pub(crate) fn parse_rollout_file(
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
                    reasoning_effort: None,
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
                    existing.reasoning_effort = first_json_string(
                        payload,
                        &["/reasoning_effort", "/reasoningEffort", "/effort"],
                    );
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
