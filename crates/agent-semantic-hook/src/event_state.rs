//! Append-only hook event state written by `asp hook`.

use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_runtime::ensure_project_hook_state_dir;
use serde_json::{Value, json};

use crate::protocol::{DecisionKind, HOOK_PROTOCOL_ID, HookDecision};

pub(crate) const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";
const HOOK_EVENT_SCHEMA_ID: &str = "agent.semantic-protocols.hook.event";
const DENY_REPLAY_WINDOW_MS: u128 = 5 * 60 * 1000;

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

    if !has_recent_matching_deny(project_root, &replay_key)? {
        decision.fields.insert(
            "denyReplay".to_string(),
            Value::String("record".to_string()),
        );
        return Ok(false);
    }

    decision.fields.insert(
        "denyReplay".to_string(),
        Value::String("repeated".to_string()),
    );
    decision.message = repeated_deny_message(decision);
    Ok(true)
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

/// Remove cached hook event state when it belongs to an older hook protocol.
pub fn remove_incompatible_hook_event_state(
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let state_path = ensure_project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
    if !state_path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(&state_path).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    if content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .all(is_current_hook_event_state_line)
    {
        return Ok(None);
    }
    fs::remove_file(&state_path).map_err(|error| {
        format!(
            "failed to remove hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(Some(state_path))
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
    let content = fs::read_to_string(&state_path).map_err(|error| {
        format!(
            "failed to read hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(content.lines().rev().any(|line| {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        is_recent_event(&event, now)
            && event.get("decision").and_then(Value::as_str) == Some("deny")
            && event.get("denyReplayKey").and_then(Value::as_str) == Some(replay_key)
    }))
}

fn is_recent_event(event: &Value, now: u128) -> bool {
    let Some(recorded_at) = event.get("recordedAtUnixMs").and_then(Value::as_u64) else {
        return false;
    };
    now.saturating_sub(u128::from(recorded_at)) <= DENY_REPLAY_WINDOW_MS
}

fn deny_replay_key(decision: &HookDecision) -> Option<String> {
    if decision.decision != DecisionKind::Deny {
        return None;
    }
    let reason = serde_json::to_value(decision.reason_kind).ok()?;
    let mut language_ids = decision.language_ids.clone();
    language_ids.sort();
    language_ids.dedup();
    let routes = decision
        .routes
        .iter()
        .map(|route| {
            json!({
                "languageId": route.language_id,
                "providerId": route.provider_id,
                "kind": route.kind,
                "argv": route.argv,
            })
        })
        .collect::<Vec<_>>();
    let subject = if routes.is_empty() {
        serde_json::to_value(&decision.subject).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let key = json!({
        "platform": decision.platform,
        "reasonKind": reason,
        "languageIds": language_ids,
        "operationIntent": decision.fields.get("operationIntent").cloned().unwrap_or(Value::Null),
        "toolSurface": decision.fields.get("toolSurface").cloned().unwrap_or(Value::Null),
        "sessionId": decision.fields.get("sessionId").cloned().unwrap_or(Value::Null),
        "transcriptPath": decision.fields.get("transcriptPath").cloned().unwrap_or(Value::Null),
        "routes": routes,
        "subject": subject,
    });
    serde_json::to_string(&key).ok()
}

fn repeated_deny_message(decision: &HookDecision) -> String {
    let reason = serde_json::to_value(decision.reason_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "source-access".to_string());
    [
        format!("ASP hook already denied `{reason}` on this source-access lane."),
        "See @.agents/skills/agent-semantic-protocols/SKILL.md for the active ASP agent workflow."
            .to_string(),
        String::new(),
        "## ASP Hook Recovery".to_string(),
        "Follow the previous recovery route instead of retrying raw source tools.".to_string(),
        String::new(),
        "## Stop".to_string(),
        "Do not retry `Read`, `cat`, `sed`, `rg`, or source-dump commands on the matched source. The hook has already denied this lane."
            .to_string(),
    ]
    .join("\n")
}
