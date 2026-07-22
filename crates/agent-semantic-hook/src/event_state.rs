//! Append-only hook event state persisted by `asp hook`.

use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_runtime::ensure_project_hook_state_dir;
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

fn should_preserve_agent_session_route_message(decision: &HookDecision) -> bool {
    decision.has_configured_resident_dispatch()
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
    let preserve_agent_session_route_message =
        should_preserve_agent_session_route_message(decision);
    if source_access_replay && !preserve_agent_session_route_message {
        insert_asp_explore_recovery_action_fields(decision);
    }
    let compact_first_source_access_replay =
        source_access_replay && should_compact_source_access_deny_message(decision);

    if !has_recent_matching_deny(project_root, &replay_key)? {
        decision.fields.insert(
            "denyReplay".to_string(),
            Value::String("record".to_string()),
        );
        if preserve_agent_session_route_message {
            decision.fields.insert(
                "denyReplayMessagePolicy".to_string(),
                Value::String("preserve-agent-session-route".to_string()),
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
    if preserve_agent_session_route_message {
        decision.fields.insert(
            "denyReplayMessagePolicy".to_string(),
            Value::String("preserve-agent-session-route".to_string()),
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

fn insert_asp_explore_recovery_action_fields(decision: &mut HookDecision) {
    decision
        .fields
        .entry("requiredAction".to_string())
        .or_insert_with(|| Value::String("enter-asp-explore-choice-pane".to_string()));
    decision
        .fields
        .entry("nextAction".to_string())
        .or_insert_with(|| Value::String("choose-one-bootstrap-pane-option".to_string()));
    decision
        .fields
        .entry("targetAgentName".to_string())
        .or_insert_with(|| Value::String("asp_explorer".to_string()));
    decision
        .fields
        .entry("targetAgentRole".to_string())
        .or_insert_with(|| Value::String("asp_explorer".to_string()));
    decision
        .fields
        .entry("targetAgentSelectionSource".to_string())
        .or_insert_with(|| Value::String("hook-deny-intent".to_string()));
    decision
        .fields
        .entry("targetAgentRegistrySource".to_string())
        .or_insert_with(|| {
            Value::String("~/.agent-semantic-protocols/agents/config.toml".to_string())
        });
    decision
        .fields
        .entry("forbiddenUntilResolved".to_string())
        .or_insert_with(|| Value::String("raw-source-fallback".to_string()));
    decision
        .fields
        .entry("completionReceipt".to_string())
        .or_insert_with(|| Value::String("asp-explore-choice-pane-receipt".to_string()));
}

/// Append one compact hook decision record to `events.jsonl`.
pub fn append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    let state_dir = ensure_project_hook_state_dir(project_root)?;
    let state_path = state_dir.join(HOOK_EVENT_STATE_FILE);
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
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(&state_path)
        .map_err(|error| {
            format!(
                "failed to open hook state {}: {error}",
                state_path.display()
            )
        })?;
    file.lock_exclusive().map_err(|error| {
        format!(
            "failed to lock hook state {}: {error}",
            state_path.display()
        )
    })?;
    if file
        .metadata()
        .map_err(|error| {
            format!(
                "failed to stat hook state {}: {error}",
                state_path.display()
            )
        })?
        .len()
        > HOOK_EVENT_STATE_MAX_BYTES
    {
        file.set_len(0).map_err(|error| {
            format!(
                "failed to truncate hook state {}: {error}",
                state_path.display()
            )
        })?;
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            format!(
                "failed to seek hook state {}: {error}",
                state_path.display()
            )
        })?;
    }
    let mut line = event.to_string();
    line.push('\n');
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
    file.unlock().map_err(|error| {
        format!(
            "failed to unlock hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(state_path)
}

/// Return whether the current prompt/session already recorded subagent context.
pub fn has_recorded_subagent_context(
    project_root: &Path,
    session_id: Option<&str>,
    transcript_path: Option<&str>,
) -> Result<bool, String> {
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
        if !event_matches_prompt_scope(&event, session_id, transcript_path) {
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

    let mut content = String::new();
    file.read_to_string(&mut content).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;

    let mut lines = content.lines().collect::<Vec<_>>();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
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

pub(crate) fn asp_command_tokens(tokens: &[String]) -> bool {
    asp_token_index(tokens).is_some()
}

fn asp_token_index(tokens: &[String]) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token == "asp" || token.ends_with("/asp") || token.ends_with(".bin/asp"))
}
