use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const ROLLOUT_ACTIVITY_TAIL_BYTES: u64 = 64 * 1024;
const ROLLOUT_ACTIVITY_STALE_SECONDS: i64 = 120;
const ROLLOUT_ACTIVITY_RECENT_HEARTBEATS: usize = 3;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RolloutActivityHeartbeat {
    pub(super) at: String,
    pub(super) kind: String,
    #[serde(rename = "turnId", skip_serializing_if = "Option::is_none")]
    pub(super) turn_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RolloutActivityReport {
    pub(super) status: String,
    #[serde(rename = "rolloutPath")]
    pub(super) rollout_path: String,
    #[serde(rename = "sessionMeta", skip_serializing_if = "Option::is_none")]
    pub(super) session_meta: Option<crate::codex::rollout::CodexRolloutSessionMeta>,
    #[serde(rename = "sessionActivity", skip_serializing_if = "Option::is_none")]
    pub(super) session_activity: Option<crate::codex::rollout::CodexRolloutSessionActivity>,
    #[serde(rename = "lastEventAt", skip_serializing_if = "Option::is_none")]
    pub(super) last_event_at: Option<String>,
    #[serde(rename = "lastHeartbeatAt", skip_serializing_if = "Option::is_none")]
    pub(super) last_heartbeat_at: Option<String>,
    #[serde(rename = "lastHeartbeatKind", skip_serializing_if = "Option::is_none")]
    pub(super) last_heartbeat_kind: Option<String>,
    #[serde(rename = "recentHeartbeats", skip_serializing_if = "Vec::is_empty")]
    pub(super) recent_heartbeats: Vec<RolloutActivityHeartbeat>,
    #[serde(
        rename = "secondsSinceHeartbeat",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) seconds_since_heartbeat: Option<i64>,
    #[serde(rename = "currentTurnId", skip_serializing_if = "Option::is_none")]
    pub(super) current_turn_id: Option<String>,
    #[serde(
        rename = "lastRunningSessionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) last_running_session_id: Option<String>,
    #[serde(rename = "runningSessionClosed")]
    pub(super) running_session_closed: bool,
    #[serde(rename = "lastTerminalEvent", skip_serializing_if = "Option::is_none")]
    pub(super) last_terminal_event: Option<String>,
    #[serde(rename = "agentInstruction")]
    pub(super) agent_instruction: String,
    #[serde(rename = "scannedBytes")]
    pub(super) scanned_bytes: usize,
}

pub(super) fn rollout_activity_report(rollout_path: &Path, now: i64) -> RolloutActivityReport {
    let session_meta = crate::codex::rollout::rollout_session_meta(rollout_path);
    let rollout_path_string = rollout_path.display().to_string();
    let mut report = match read_rollout_tail(rollout_path, ROLLOUT_ACTIVITY_TAIL_BYTES) {
        Ok(tail) => summarize_rollout_tail(rollout_path_string, tail, rollout_path, now),
        Err(error) => RolloutActivityReport {
            status: "unavailable".to_string(),
            rollout_path: rollout_path_string,
            session_meta: None,
            session_activity: None,
            last_event_at: None,
            last_heartbeat_at: None,
            last_heartbeat_kind: None,
            recent_heartbeats: Vec::new(),
            seconds_since_heartbeat: None,
            current_turn_id: None,
            last_running_session_id: None,
            running_session_closed: true,
            last_terminal_event: None,
            agent_instruction: format!("rollout-heartbeat-unavailable: {error}"),
            scanned_bytes: 0,
        },
    };
    report.session_meta = session_meta;
    if let Some(activity) = report.session_activity.as_ref() {
        report.status = activity.status.clone();
        report.running_session_closed = false;
        report.seconds_since_heartbeat = None;
        report.agent_instruction = match activity.status.as_str() {
            "tool-running" | "agent-active" => "child-activity-running-wait".to_string(),
            "idle-resumable" => "child-idle-resumable-resume-existing-child".to_string(),
            _ => "child-activity-state-authoritative".to_string(),
        };
    }
    report
}

fn read_rollout_tail(path: &Path, max_bytes: u64) -> Result<String, String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("failed to open rollout {}: {error}", path.display()))?;
    let len = file
        .metadata()
        .map_err(|error| format!("failed to stat rollout {}: {error}", path.display()))?
        .len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("failed to seek rollout {}: {error}", path.display()))?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .map_err(|error| format!("failed to read rollout {}: {error}", path.display()))?;
    if start > 0
        && let Some(index) = buffer.find('\n')
    {
        buffer = buffer[index + 1..].to_string();
    }
    Ok(buffer)
}

fn summarize_rollout_tail(
    rollout_path: String,
    tail: String,
    path: &Path,
    now: i64,
) -> RolloutActivityReport {
    let mut state = RolloutActivityState::default();
    for line in tail.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let scanned_bytes = line.len() + 1;
        state.scanned_bytes += scanned_bytes;
        state.session_activity.observe_event(&value, scanned_bytes);
        state.observe_event(&value);
    }
    let session_activity = state.session_activity.finish();
    let seconds_since_heartbeat = rollout_file_age_seconds(path, now);
    let running_session_closed = state.last_terminal_event.is_some()
        || state
            .last_running_session_id
            .as_ref()
            .is_some_and(|session_id| state.running_sessions.get(session_id) == Some(&true));
    let stale = seconds_since_heartbeat
        .map(|seconds| seconds > ROLLOUT_ACTIVITY_STALE_SECONDS)
        .unwrap_or(false);
    let status = if state.last_terminal_event.as_deref() == Some("turn_aborted") {
        "turn_aborted"
    } else if state.last_terminal_event.is_some() {
        "completed"
    } else if running_session_closed {
        "runningSessionClosed"
    } else if state.last_heartbeat_at.is_some() && !stale {
        "active"
    } else if !running_session_closed && stale {
        "orphan-risk"
    } else {
        "silent"
    };
    RolloutActivityReport {
        status: status.to_string(),
        rollout_path,
        session_meta: None,
        session_activity: Some(session_activity),
        last_event_at: state.last_event_at,
        last_heartbeat_at: state.last_heartbeat_at,
        last_heartbeat_kind: state.last_heartbeat_kind,
        recent_heartbeats: state.recent_heartbeats,
        seconds_since_heartbeat,
        current_turn_id: state.current_turn_id,
        last_running_session_id: state.last_running_session_id,
        running_session_closed,
        last_terminal_event: state.last_terminal_event,
        agent_instruction: agent_instruction_for_rollout_status(status).to_string(),
        scanned_bytes: state.scanned_bytes,
    }
}

fn agent_instruction_for_rollout_status(status: &str) -> &'static str {
    match status {
        "active" => "child-has-heartbeat-wait",
        "completed" => "child-turn-complete-read-result",
        "turn_aborted" => "child-turn-aborted-review-last-response",
        "runningSessionClosed" => "child-session-closed-check-orphan-before-retry",
        "orphan-risk" => "child-silent-with-open-process-check-orphan-before-retry",
        _ => "child-silent-send-bounded-status-request-before-retry",
    }
}

#[derive(Default)]
struct RolloutActivityState {
    last_event_at: Option<String>,
    last_heartbeat_at: Option<String>,
    last_heartbeat_kind: Option<String>,
    recent_heartbeats: Vec<RolloutActivityHeartbeat>,
    current_turn_id: Option<String>,
    last_running_session_id: Option<String>,
    last_terminal_event: Option<String>,
    last_terminal_at: Option<String>,
    session_activity: crate::codex::rollout::CodexRolloutSessionActivityState,
    call_sessions: BTreeMap<String, String>,
    running_sessions: BTreeMap<String, bool>,
    scanned_bytes: usize,
}

impl RolloutActivityState {
    fn observe_event(&mut self, value: &Value) {
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(timestamp) = timestamp.as_ref() {
            self.last_event_at = Some(timestamp.clone());
        }
        let event_type = value.get("type").and_then(Value::as_str);
        let payload = value.get("payload").unwrap_or(&Value::Null);
        match event_type {
            Some("event_msg") => self.observe_event_msg(payload, timestamp.as_deref()),
            Some("turn_context") => {
                if let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) {
                    self.current_turn_id = Some(turn_id.to_string());
                }
            }
            Some("response_item") => self.observe_response_item(payload, timestamp.as_deref()),
            _ => {}
        }
    }

    fn observe_event_msg(&mut self, payload: &Value, timestamp: Option<&str>) {
        let payload_type = payload.get("type").and_then(Value::as_str);
        if let Some(turn_id) = payload.get("turn_id").and_then(Value::as_str) {
            self.current_turn_id = Some(turn_id.to_string());
        }
        match payload_type {
            Some("agent_message") | Some("token_count") => {
                self.mark_heartbeat(payload_type.unwrap_or("event_msg"), timestamp);
            }
            Some("task_complete") | Some("turn_aborted") => {
                let event = payload_type.unwrap_or("terminal").to_string();
                self.last_terminal_event = Some(event);
                self.last_terminal_at = timestamp.map(str::to_string);
                self.mark_heartbeat(payload_type.unwrap_or("terminal"), timestamp);
            }
            _ => {}
        }
    }

    fn observe_response_item(&mut self, payload: &Value, timestamp: Option<&str>) {
        match payload.get("type").and_then(Value::as_str) {
            Some("function_call") => {
                if payload.get("name").and_then(Value::as_str) == Some("write_stdin")
                    && let (Some(call_id), Some(arguments)) = (
                        payload.get("call_id").and_then(Value::as_str),
                        payload.get("arguments").and_then(Value::as_str),
                    )
                    && let Some(session_id) = session_id_from_arguments(arguments)
                {
                    self.call_sessions.insert(call_id.to_string(), session_id);
                }
            }
            Some("function_call_output") => {
                self.mark_heartbeat("function_call_output", timestamp);
                let output = payload.get("output").and_then(Value::as_str).unwrap_or("");
                if let Some(session_id) = running_session_id_from_output(output) {
                    self.running_sessions.insert(session_id.clone(), false);
                    self.last_running_session_id = Some(session_id);
                } else if let Some(call_id) = payload.get("call_id").and_then(Value::as_str)
                    && let Some(session_id) = self.call_sessions.get(call_id)
                    && (output.contains("Process exited with code")
                        || output.contains("aborted by user"))
                {
                    self.running_sessions.insert(session_id.clone(), true);
                }
            }
            _ => {}
        }
    }

    fn mark_heartbeat(&mut self, kind: &str, timestamp: Option<&str>) {
        if let Some(timestamp) = timestamp {
            self.last_heartbeat_at = Some(timestamp.to_string());
            self.recent_heartbeats.push(RolloutActivityHeartbeat {
                at: timestamp.to_string(),
                kind: kind.to_string(),
                turn_id: self.current_turn_id.clone(),
            });
            if self.recent_heartbeats.len() > ROLLOUT_ACTIVITY_RECENT_HEARTBEATS {
                self.recent_heartbeats.remove(0);
            }
        }
        self.last_heartbeat_kind = Some(kind.to_string());
    }
}

fn running_session_id_from_output(output: &str) -> Option<String> {
    digits_after(output, "Process running with session ID ")
}

fn session_id_from_arguments(arguments: &str) -> Option<String> {
    digits_after(arguments, "\"session_id\":")
}

fn digits_after(value: &str, prefix: &str) -> Option<String> {
    let start = value.find(prefix)? + prefix.len();
    let digits = value[start..]
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then_some(digits)
}

fn rollout_file_age_seconds(path: &Path, now: i64) -> Option<i64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let modified = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())?;
    Some(now.saturating_sub(modified))
}
