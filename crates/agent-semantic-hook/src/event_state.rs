//! Append-only hook event state persisted by `asp hook`.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_runtime::ensure_project_hook_state_dir;
use fs2::FileExt;
use serde_json::{Value, json};

use crate::event_replay::{
    compact_source_access_deny_message, deny_replay_key, is_source_access_replay_key,
    recovery_ref_for_replay_key, repeated_deny_message, should_compact_source_access_deny_message,
};
use crate::protocol::{HOOK_PROTOCOL_ID, HookDecision};

pub(crate) const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";
const PROMPT_SCOPE_WINDOW_MS: u128 = 10 * 60 * 1000;
const HOOK_EVENT_SCHEMA_ID: &str = "agent.semantic-protocols.hook.event";
const DENY_REPLAY_WINDOW_MS: u128 = 3 * 60 * 1000;
const HOOK_EVENT_STATE_TAIL_BYTES: u64 = 1024 * 1024;
const HOOK_EVENT_STATE_TAIL_LINE_CAP: usize = 4096;
const HOOK_EVENT_STATE_LOCK_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEventSessionId(String);

impl HookEventSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for HookEventSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for HookEventSessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEventTranscriptPath(String);

impl HookEventTranscriptPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for HookEventTranscriptPath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for HookEventTranscriptPath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookEventStateError(String);

impl std::fmt::Display for HookEventStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HookEventStateError {}

impl From<String> for HookEventStateError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Policy selection recorded by a denied Hook event.
///
/// Resident identity is intentionally absent. `asp session` resolves the
/// selected rule's semantic role through the current managed config and agent
/// registry, so a Hook receipt cannot become a second agent registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookSessionAgentRoute {
    pub command_digest: Option<String>,
    pub config_rule_id: String,
    pub deny_evidence_ref: Option<String>,
    pub reason_kind: String,
    pub root_session_id: String,
    pub subject_command: Option<String>,
}

fn should_preserve_parser_route_message(decision: &HookDecision) -> bool {
    !decision.routes.is_empty()
        || decision.has_configured_resident_dispatch()
        || decision.fields.contains_key("agentSessionAction")
            && decision.fields.contains_key("agentSessionRoute")
}
const HOOK_EVENT_STATE_MAX_BYTES: u64 = HOOK_EVENT_STATE_TAIL_BYTES * 4;

/// Convert a repeated deny in the same source-access lane into a compact replay.
pub fn apply_repeated_deny_replay(
    project_root: &Path,
    decision: &mut HookDecision,
) -> Result<bool, String> {
    let Some(replay_key) = deny_replay_key(decision) else {
        return Ok(false);
    };
    decision.fields.insert(
        "denyReplayKey".to_string(),
        Value::String(replay_key.clone()),
    );
    let recovery_ref = recovery_ref_for_replay_key(&replay_key);
    decision.fields.insert(
        "recoveryRef".to_string(),
        Value::String(recovery_ref.clone()),
    );
    let source_access_replay = is_source_access_replay_key(&replay_key);
    let preserve_parser_route_message = should_preserve_parser_route_message(decision);
    if source_access_replay && !preserve_parser_route_message {
        insert_resident_recovery_action_fields(decision);
    }
    let compact_first_source_access_replay =
        source_access_replay && should_compact_source_access_deny_message(decision);

    if !has_recent_matching_deny(project_root, &replay_key)? {
        decision.fields.insert(
            "denyReplay".to_string(),
            Value::String("record".to_string()),
        );
        if preserve_parser_route_message {
            decision.fields.insert(
                "denyReplayMessagePolicy".to_string(),
                Value::String("preserve-parser-route".to_string()),
            );
        } else if compact_first_source_access_replay {
            decision.message = compact_source_access_deny_message(decision, &recovery_ref);
        }
        return Ok(false);
    }

    decision.fields.insert(
        "denyReplay".to_string(),
        Value::String("repeated".to_string()),
    );
    if preserve_parser_route_message {
        decision.fields.insert(
            "denyReplayMessagePolicy".to_string(),
            Value::String("preserve-parser-route".to_string()),
        );
        return Ok(true);
    }
    decision.message = if source_access_replay {
        compact_source_access_deny_message(decision, &recovery_ref)
    } else {
        repeated_deny_message(decision)
    };
    Ok(true)
}

fn insert_resident_recovery_action_fields(decision: &mut HookDecision) {
    decision
        .fields
        .entry("requiredAction".to_string())
        .or_insert_with(|| Value::String("open-org-interactive-resident-agent-window".to_string()));
    decision
        .fields
        .entry("nextAction".to_string())
        .or_insert_with(|| Value::String("run-asp-session-agent-window".to_string()));
    decision
        .fields
        .entry("forbiddenUntilResolved".to_string())
        .or_insert_with(|| Value::String("raw-source-fallback".to_string()));
    decision
        .fields
        .entry("completionReceipt".to_string())
        .or_insert_with(|| Value::String("resident-agent-host-action-receipt".to_string()));
    decision
        .fields
        .entry("agentWindowCommand".to_string())
        .or_insert_with(|| Value::String("asp session --agents choice-plane".to_string()));
}

/// Return the newest denied Hook policy selection for this root session.
pub fn latest_hook_session_agent_route(
    project_root: &Path,
) -> Result<Option<HookSessionAgentRoute>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(None);
    }
    let lines = read_hook_event_state_tail(&state_path)?;
    Ok(latest_hook_session_agent_route_from_lines(&lines))
}

fn latest_hook_session_agent_route_from_lines(lines: &[String]) -> Option<HookSessionAgentRoute> {
    lines.iter().rev().find_map(|line| {
        let event = serde_json::from_str::<Value>(line).ok()?;
        if !matches!(
            event.get("decision").and_then(Value::as_str),
            Some("deny" | "block")
        ) || event
            .pointer("/fields/agentWindowCommand")
            .and_then(Value::as_str)
            != Some("asp session --agents choice-plane")
            || event
                .pointer("/fields/choicePlaneOwner")
                .and_then(Value::as_str)
                != Some("org-contract:agent-interactive")
        {
            return None;
        }
        Some(HookSessionAgentRoute {
            command_digest: event
                .pointer("/fields/commandDigest")
                .and_then(Value::as_str)
                .map(str::to_owned),
            config_rule_id: required_event_string(&event, "/fields/configRuleId")?,
            deny_evidence_ref: event
                .pointer("/fields/recoveryRef")
                .and_then(Value::as_str)
                .map(str::to_owned),
            reason_kind: required_event_string(&event, "/reasonKind")?,
            root_session_id: required_event_string(&event, "/fields/sessionId")?,
            subject_command: event
                .pointer("/subject/command")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    })
}

fn required_event_string(event: &Value, pointer: &str) -> Option<String> {
    event
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Append one compact hook decision record to `events.jsonl`.
pub fn append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    append_hook_event_state_with_lock_timeout(project_root, decision, HOOK_EVENT_STATE_LOCK_TIMEOUT)
}

/// Try to project a decision without putting the authoritative Hook response
/// behind the diagnostic writer's normal contention budget.
pub fn try_append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    append_hook_event_state_with_lock_timeout(project_root, decision, Duration::ZERO)
}

fn append_hook_event_state_with_lock_timeout(
    project_root: &Path,
    decision: &HookDecision,
    lock_timeout: Duration,
) -> Result<PathBuf, String> {
    let state_dir = ensure_project_hook_state_dir(project_root)?;
    let state_path = state_dir.join(HOOK_EVENT_STATE_FILE);
    let writer_lock = acquire_event_state_writer(&state_dir, lock_timeout)?;
    let event = json!({
        "schemaId": HOOK_EVENT_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": decision.protocol_id,
        "protocolVersion": decision.protocol_version,
        "recordedAtUnixMs": unix_time_ms(),
        "platform": decision.platform,
        "event": decision.event,
        "decision": decision.decision,
        "reasonKind": decision.reason_kind,
        "languageIds": decision.language_ids,
        "subject": decision.subject,
        "routeKinds": decision.routes.iter().map(|route| route.kind).collect::<Vec<_>>(),
        "fields": decision.fields,
        "denyReplayKey": decision.fields.get("denyReplayKey"),
    });
    let mut line = event.to_string();
    line.push('\n');
    let state_len = fs::metadata(&state_path)
        .map(|metadata| metadata.len())
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(0)
            } else {
                Err(error)
            }
        })
        .map_err(|error| {
            format!(
                "failed to stat hook state {}: {error}",
                state_path.display()
            )
        })?;
    if state_len.saturating_add(line.len() as u64) > HOOK_EVENT_STATE_MAX_BYTES {
        let mut compacted = read_hook_event_state_tail(&state_path)?
            .into_iter()
            .filter(|retained| is_current_hook_event_state_line(retained))
            .collect::<Vec<_>>()
            .join("\n");
        if !compacted.is_empty() {
            compacted.push('\n');
        }
        compacted.push_str(&line);
        replace_hook_event_state(&state_path, compacted.as_bytes())?;
    } else {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&state_path)
            .map_err(|error| {
                format!(
                    "failed to open hook state {}: {error}",
                    state_path.display()
                )
            })?;
        file.write_all(line.as_bytes()).map_err(|error| {
            format!(
                "failed to write hook state {}: {error}",
                state_path.display()
            )
        })?;
        file.flush().map_err(|error| {
            format!(
                "failed to flush hook state {}: {error}",
                state_path.display()
            )
        })?;
    }
    FileExt::unlock(&writer_lock)
        .map_err(|error| format!("unlock Hook event writer {}: {error}", state_dir.display()))?;
    Ok(state_path)
}

fn acquire_event_state_writer(state_dir: &Path, lock_timeout: Duration) -> Result<File, String> {
    let lock_path = state_dir.join("events.jsonl.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| {
            format!(
                "open Hook event writer lock {}: {error}",
                lock_path.display()
            )
        })?;
    let started = Instant::now();
    loop {
        match lock.try_lock_exclusive() {
            Ok(()) => return Ok(lock),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && started.elapsed() < lock_timeout =>
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Err(format!(
                    "Hook event writer lock exceeded {}ms at {}",
                    lock_timeout.as_millis(),
                    lock_path.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "lock Hook event writer {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
}

fn replace_hook_event_state(state_path: &Path, content: &[u8]) -> Result<(), String> {
    let file_name = state_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(HOOK_EVENT_STATE_FILE);
    let temporary_path = state_path.with_file_name(format!(".{file_name}.tmp"));
    let replace_result = (|| {
        let mut temporary = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| {
                format!(
                    "failed to open temporary hook state {}: {error}",
                    temporary_path.display()
                )
            })?;
        temporary.write_all(content).map_err(|error| {
            format!(
                "failed to write temporary hook state {}: {error}",
                temporary_path.display()
            )
        })?;
        temporary.sync_all().map_err(|error| {
            format!(
                "failed to sync temporary hook state {}: {error}",
                temporary_path.display()
            )
        })?;
        fs::rename(&temporary_path, state_path).map_err(|error| {
            format!(
                "failed to replace hook state {} from {}: {error}",
                state_path.display(),
                temporary_path.display()
            )
        })
    })();
    if replace_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    replace_result
}

/// Return whether the current prompt/session already recorded subagent context.
pub fn has_recorded_subagent_context(
    project_root: &Path,
    session_id: Option<HookEventSessionId>,
    transcript_path: Option<HookEventTranscriptPath>,
) -> Result<bool, HookEventStateError> {
    if session_id.is_none() && transcript_path.is_none() {
        return Ok(false);
    }
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(false);
    }
    let now = unix_time_ms();
    let lines = read_hook_event_state_tail(&state_path)?;
    for line in lines.iter().rev() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_recent_for_window(&event, now, PROMPT_SCOPE_WINDOW_MS) {
            break;
        }
        if !event_matches_prompt_scope(
            &event,
            session_id.as_ref().map(HookEventSessionId::as_str),
            transcript_path
                .as_ref()
                .map(HookEventTranscriptPath::as_str),
        ) {
            continue;
        }
        if is_prompt_scope_boundary(&event) {
            break;
        }
        match event.get("event").and_then(Value::as_str) {
            Some("subagent-start") => return Ok(true),
            Some("subagent-stop") => return Ok(false),
            _ => {}
        }
        if event
            .pointer("/fields/subagentContext")
            .and_then(Value::as_bool)
            == Some(true)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Remove cached hook event state when it belongs to an older hook protocol.
pub fn remove_incompatible_hook_event_state(
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    remove_incompatible_hook_event_state_path(&state_path)
}

fn remove_incompatible_hook_event_state_path(state_path: &Path) -> Result<Option<PathBuf>, String> {
    if !state_path.is_file() {
        return Ok(None);
    }
    let file = fs::File::open(state_path).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line).map_err(|error| {
            format!(
                "failed to read hook state {}: {error}",
                state_path.display()
            )
        })?;
        if bytes == 0 {
            return Ok(None);
        }
        if line.trim().is_empty() {
            continue;
        }
        if is_current_hook_event_state_line(&line) {
            return Ok(None);
        }
        break;
    }
    fs::remove_file(state_path).map_err(|error| {
        format!(
            "failed to remove hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(Some(state_path.to_path_buf()))
}

fn is_current_hook_event_state_line(line: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    value.get("schemaId").and_then(serde_json::Value::as_str) == Some(HOOK_EVENT_SCHEMA_ID)
        && value.get("protocolId").and_then(serde_json::Value::as_str) == Some(HOOK_PROTOCOL_ID)
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn has_recent_matching_deny(project_root: &Path, replay_key: &str) -> Result<bool, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(false);
    }
    let now = unix_time_ms();
    let replay_key_json = serde_json::to_string(replay_key)
        .map_err(|error| format!("failed to encode hook replay key: {error}"))?;
    let lines = read_hook_event_state_tail(&state_path)?;
    for line in lines.iter().rev() {
        if !line.contains(&replay_key_json) {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !is_recent_event(&event, now) {
            break;
        }
        if event.get("decision").and_then(Value::as_str) == Some("deny")
            && event.get("denyReplayKey").and_then(Value::as_str) == Some(replay_key)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn read_hook_event_state_tail(state_path: &Path) -> Result<Vec<String>, String> {
    let mut file = fs::File::open(state_path).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    let file_len = file
        .metadata()
        .map_err(|error| {
            format!(
                "failed to stat hook state {}: {error}",
                state_path.display()
            )
        })?
        .len();
    let start = file_len.saturating_sub(HOOK_EVENT_STATE_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).map_err(|error| {
        format!(
            "failed to seek hook state {}: {error}",
            state_path.display()
        )
    })?;

    let mut content = Vec::new();
    file.read_to_end(&mut content).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    if start > 0 {
        let Some(first_newline) = content.iter().position(|byte| *byte == b'\n') else {
            return Ok(Vec::new());
        };
        content.drain(..=first_newline);
    }
    let content = String::from_utf8(content).map_err(|error| {
        format!(
            "failed to decode hook state {} as UTF-8: {error}",
            state_path.display()
        )
    })?;
    let lines = content.lines().collect::<Vec<_>>();
    let first_line = lines.len().saturating_sub(HOOK_EVENT_STATE_TAIL_LINE_CAP);
    Ok(lines[first_line..]
        .iter()
        .map(|line| (*line).to_string())
        .collect())
}

fn is_recent_event(event: &Value, now: u128) -> bool {
    is_recent_for_window(event, now, DENY_REPLAY_WINDOW_MS)
}

fn is_recent_for_window(event: &Value, now: u128, window_ms: u128) -> bool {
    let Some(recorded_at) = event.get("recordedAtUnixMs").and_then(Value::as_u64) else {
        return false;
    };
    now.saturating_sub(u128::from(recorded_at)) <= window_ms
}

fn event_matches_prompt_scope(
    event: &Value,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> bool {
    let fields = event.get("fields").unwrap_or(event);
    let session_matches = session_id
        .is_some_and(|expected| fields.get("sessionId").and_then(Value::as_str) == Some(expected));
    let transcript_matches = transcript_path.is_some_and(|expected| {
        fields.get("transcriptPath").and_then(Value::as_str) == Some(expected)
    });
    session_matches || transcript_matches
}

fn is_prompt_scope_boundary(event: &Value) -> bool {
    event.get("event").and_then(Value::as_str) == Some("user-prompt")
}
