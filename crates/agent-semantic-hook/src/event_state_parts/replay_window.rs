// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Bounded replay-window reads and event freshness predicates.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_runtime::ensure_project_hook_state_dir;
use serde_json::Value;

use super::{
    DENY_REPLAY_WINDOW_MS, HOOK_EVENT_SCHEMA_ID, HOOK_EVENT_STATE_FILE,
    HOOK_EVENT_STATE_TAIL_BYTES, HOOK_EVENT_STATE_TAIL_LINE_CAP,
};
use crate::protocol::HOOK_PROTOCOL_ID;

pub(super) fn is_current_hook_event_state_line(line: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    value.get("schemaId").and_then(serde_json::Value::as_str) == Some(HOOK_EVENT_SCHEMA_ID)
        && value.get("protocolId").and_then(serde_json::Value::as_str) == Some(HOOK_PROTOCOL_ID)
}

pub(super) fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub(super) fn has_recent_matching_deny(
    project_root: &Path,
    replay_key: &str,
) -> Result<bool, String> {
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
        if !is_recent_for_window(&event, now, DENY_REPLAY_WINDOW_MS) {
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

pub(super) fn is_recent_for_window(event: &Value, now: u128, window_ms: u128) -> bool {
    let Some(recorded_at) = event.get("recordedAtUnixMs").and_then(Value::as_u64) else {
        return false;
    };
    now.saturating_sub(u128::from(recorded_at)) <= window_ms
}

pub(super) fn event_matches_prompt_scope(
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

pub(super) fn is_prompt_scope_boundary(event: &Value) -> bool {
    event.get("event").and_then(Value::as_str) == Some("user-prompt")
}
