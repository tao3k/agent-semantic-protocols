use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_config::{
    COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID, CollaborationLiveAgentSnapshot,
    CollaborationLiveAgents,
};
use serde_json::Value;

pub(crate) fn observe_post_tool_payload(payload: &Value) -> Result<bool, String> {
    let tool_name = payload
        .get("tool_name")
        .or_else(|| payload.get("toolName"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if tool_name != "list_agents" && tool_name != "collaboration.list_agents" {
        return Ok(false);
    }
    let response = payload
        .get("tool_response")
        .or_else(|| payload.get("toolResponse"))
        .ok_or("collaboration.list_agents PostToolUse payload requires tool_response")?;
    let agents = decode_live_agents(response)?;
    agents.validate()?;
    let workspace_root = payload
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|value| Path::new(value).is_absolute())
        .ok_or("collaboration.list_agents PostToolUse payload requires an absolute cwd")?;
    let root_session_id = payload
        .get("session_id")
        .or_else(|| payload.get("sessionId"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("collaboration.list_agents PostToolUse payload requires session_id")?;
    let observed_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("resolve collaboration snapshot clock: {error}"))?
        .as_millis()
        .try_into()
        .map_err(|_| "collaboration snapshot clock exceeds u64".to_owned())?;
    let snapshot = CollaborationLiveAgentSnapshot {
        schema_id: COLLABORATION_LIVE_AGENT_SNAPSHOT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        workspace_root: workspace_root.to_owned(),
        root_session_id: root_session_id.to_owned(),
        observed_at_unix_ms,
        agents,
    };
    snapshot.validate()?;
    publish_snapshot(&state_home()?, &snapshot)?;
    Ok(true)
}

fn decode_live_agents(value: &Value) -> Result<CollaborationLiveAgents, String> {
    if value.get("agents").is_some() {
        return serde_json::from_value(value.clone())
            .map_err(|error| format!("decode collaboration.list_agents response: {error}"));
    }
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        for item in content {
            let Some(text) = item.get("text").and_then(Value::as_str) else {
                continue;
            };
            if let Ok(decoded) = serde_json::from_str::<CollaborationLiveAgents>(text) {
                return Ok(decoded);
            }
        }
    }
    Err("collaboration.list_agents response does not contain an agents array".to_owned())
}

fn publish_snapshot(
    state_home: &Path,
    snapshot: &CollaborationLiveAgentSnapshot,
) -> Result<(), String> {
    let directory = state_home.join("agents/live");
    std::fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "create collaboration live-agent inbox {}: {error}",
            directory.display()
        )
    })?;
    let key = blake3::hash(
        format!("{}\0{}", snapshot.workspace_root, snapshot.root_session_id).as_bytes(),
    );
    let target = directory.join(format!("{}.json", key.to_hex()));
    let candidate = directory.join(format!(
        ".candidate-{}-{}.json",
        std::process::id(),
        snapshot.observed_at_unix_ms
    ));
    let bytes = serde_json::to_vec(snapshot)
        .map_err(|error| format!("encode collaboration live-agent snapshot: {error}"))?;
    std::fs::write(&candidate, bytes).map_err(|error| {
        format!(
            "write collaboration live-agent candidate {}: {error}",
            candidate.display()
        )
    })?;
    std::fs::rename(&candidate, &target).map_err(|error| {
        format!(
            "publish collaboration live-agent snapshot {}: {error}",
            target.display()
        )
    })?;
    notify_runtime_snapshot_owner(&directory);
    Ok(())
}

#[cfg(unix)]
fn notify_runtime_snapshot_owner(directory: &Path) {
    use std::os::unix::net::UnixDatagram;

    let Ok(socket) = UnixDatagram::unbound() else {
        return;
    };
    let _ = socket.send_to(&[1], directory.join("notify.sock"));
}

#[cfg(not(unix))]
fn notify_runtime_snapshot_owner(_directory: &Path) {}

fn state_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("ASP_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is unavailable")?;
    Ok(PathBuf::from(home).join(".agent-semantic-protocols"))
}

#[cfg(test)]
#[path = "../tests/unit/collaboration_snapshot_inbox.rs"]
mod tests;
