//! Append-only hook event state written by `asp hook`.

use fs2::FileExt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cache_paths::project_hook_state_dir;
use serde_json::json;

use crate::protocol::{HOOK_PROTOCOL_ID, HookDecision};

pub(crate) const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";
const HOOK_EVENT_SCHEMA_ID: &str = "agent.semantic-protocols.hook.event";

/// Append one compact hook decision record to `events.jsonl`.
pub fn append_hook_event_state(
    project_root: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    let state_dir = project_hook_state_dir(project_root)?;
    fs::create_dir_all(&state_dir).map_err(|error| {
        format!(
            "failed to create hook state dir {}: {error}",
            state_dir.display()
        )
    })?;
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
    let state_path = project_hook_state_dir(project_root)?.join(HOOK_EVENT_STATE_FILE);
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
