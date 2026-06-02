use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::protocol::HookDecision;

pub(crate) const HOOK_EVENT_STATE_FILE: &str = "events.jsonl";

pub(crate) fn append_hook_event_state(
    profiles_path: &Path,
    decision: &HookDecision,
) -> Result<PathBuf, String> {
    let state_dir = profiles_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(state_dir).map_err(|error| {
        format!(
            "failed to create hook state dir {}: {error}",
            state_dir.display()
        )
    })?;
    let state_path = state_dir.join(HOOK_EVENT_STATE_FILE);
    let event = json!({
        "schemaId": "agent.semantic-protocols.semantic-agent-hook-event",
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
    writeln!(file, "{event}").map_err(|error| {
        format!(
            "failed to write hook state {}: {error}",
            state_path.display()
        )
    })?;
    Ok(state_path)
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
