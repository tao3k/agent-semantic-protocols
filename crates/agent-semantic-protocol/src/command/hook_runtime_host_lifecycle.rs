use std::path::Path;

use agent_semantic_client_db::workspace_db_ipc::{
    AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind, AgentSessionRegistryIpcResult,
};
use agent_semantic_config::agent_route_registry::load_agent_route_registry;
use serde_json::Value;

pub(super) fn record_host_lifecycle_event(
    client: &str,
    event: &str,
    payload: &Value,
    project_root: &Path,
) -> Result<bool, String> {
    let Some(kind) = host_lifecycle_kind(client, event, payload) else {
        return Ok(false);
    };
    let (root_session_id, child_session_id) = codex_payload_identity(payload)?;
    let parent_session_id = codex_rollout_parent_thread_id(child_session_id, root_session_id)?;
    let agent_type = required_payload_field(payload, "agent_type")?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)?;
    let registry = load_agent_route_registry(&state.state_home.join("agents/config.toml"))?;
    let route = registry
        .compile_route_for_platform_host_agent_name("codex", agent_type)?
        .ok_or_else(|| {
            format!("Codex Host lifecycle agent_type `{agent_type}` is not registered")
        })?;
    let profile = std::fs::read(&route.profile_path).map_err(|error| {
        format!(
            "failed to read registered Codex profile {}: {error}",
            route.profile_path
        )
    })?;
    let role = route
        .roles
        .iter()
        .find(|role| role.as_str() != "subagent")
        .cloned()
        .unwrap_or_else(|| route.route_key.as_str().to_owned());
    let observed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("failed to resolve Host lifecycle timestamp: {error}"))?
        .as_secs() as i64;
    let runtime_registry =
        agent_semantic_client_db::AgentSessionRegistry::open_runtime_project_proxy(project_root)?
            .ok_or_else(|| "Runtime Server agent-session registry proxy is unavailable".to_owned())?;
    let result = runtime_registry.record_host_lifecycle_event(AgentHostLifecycleEventIpc {
        kind,
        platform: "codex".to_owned(),
        project_id: agent_semantic_client_db::AgentSessionRegistry::workspace_id(project_root)?,
        root_session_id: root_session_id.to_owned(),
        parent_session_id,
        child_session_id: child_session_id.to_owned(),
        platform_host_agent_name: route.platform_host_agent_name.as_str().to_owned(),
        role,
        model: string_field(payload, "model").map(str::to_owned),
        profile_digest: format!("blake3-256:{}", blake3::hash(&profile).to_hex()),
        transcript_path: string_field(payload, "transcript_path").map(str::to_owned),
        observed_at,
    })?;
    match (kind, result) {
        (
            AgentHostLifecycleEventKind::Started,
            AgentSessionRegistryIpcResult::Registered { .. },
        )
        | (AgentHostLifecycleEventKind::Stopped, AgentSessionRegistryIpcResult::Changed { .. }) => {
            Ok(true)
        }
        _ => Err("Runtime Server returned an unexpected Host lifecycle result".to_owned()),
    }
}

fn host_lifecycle_kind(
    client: &str,
    event: &str,
    payload: &Value,
) -> Option<AgentHostLifecycleEventKind> {
    if client != "codex"
        || string_field(payload, "agent_id").is_none()
        || string_field(payload, "agent_type").is_none()
    {
        return None;
    }
    match event {
        "subagent-start" | "pre-tool" => Some(AgentHostLifecycleEventKind::Started),
        "subagent-stop" | "stop" => Some(AgentHostLifecycleEventKind::Stopped),
        _ => None,
    }
}

fn required_payload_field<'a>(payload: &'a Value, name: &str) -> Result<&'a str, String> {
    string_field(payload, name)
        .ok_or_else(|| format!("Codex Host lifecycle payload requires `{name}`"))
}

fn codex_payload_identity(payload: &Value) -> Result<(&str, &str), String> {
    let root_session_id = required_payload_field(payload, "session_id")?;
    let child_session_id = required_payload_field(payload, "agent_id")?;
    if root_session_id == child_session_id {
        return Err(format!(
            "Codex Host lifecycle root and child must be distinct: root={root_session_id} child={child_session_id}"
        ));
    }
    Ok((root_session_id, child_session_id))
}

fn codex_rollout_parent_thread_id(
    child_session_id: &str,
    payload_root_session_id: &str,
) -> Result<String, String> {
    let metadata =
        agent_semantic_runtime::codex_rollout_session_metadata(&child_session_id.into())?
            .ok_or_else(|| {
                format!(
                    "Codex Host lifecycle rollout metadata is missing for child {child_session_id}"
                )
            })?;
    let (parent, root_verified) = verified_rollout_topology(
        child_session_id,
        payload_root_session_id,
        metadata.root_session_id().map(|value| value.as_str()),
        metadata.parent_thread_id().map(|value| value.as_str()),
    )?;
    if !root_verified {
        verify_rollout_parent_chain(&parent, payload_root_session_id)?;
    }
    Ok(parent)
}

fn verified_rollout_topology(
    child_session_id: &str,
    payload_root_session_id: &str,
    rollout_root_session_id: Option<&str>,
    parent_thread_id: Option<&str>,
) -> Result<(String, bool), String> {
    if let Some(rollout_root) = rollout_root_session_id
        && rollout_root != payload_root_session_id
    {
        return Err(format!(
            "Codex Host lifecycle root mismatch for child {child_session_id}: payload={} rollout={}",
            payload_root_session_id, rollout_root
        ));
    }
    let parent = parent_thread_id.ok_or_else(|| {
        format!("Codex Host lifecycle parent_thread_id is missing for child {child_session_id}")
    })?;
    if parent == child_session_id {
        return Err(format!(
            "Codex Host lifecycle child cannot be its own parent: child={child_session_id}"
        ));
    }
    Ok((
        parent.to_owned(),
        rollout_root_session_id == Some(payload_root_session_id)
            || parent == payload_root_session_id,
    ))
}

fn verify_rollout_parent_chain(
    immediate_parent_session_id: &str,
    payload_root_session_id: &str,
) -> Result<(), String> {
    let mut cursor = immediate_parent_session_id.to_owned();
    let mut visited = std::collections::HashSet::new();
    for _ in 0..64 {
        if cursor == payload_root_session_id {
            return Ok(());
        }
        if !visited.insert(cursor.clone()) {
            return Err(format!(
                "Codex Host lifecycle parent chain contains a cycle at {cursor}"
            ));
        }
        let metadata =
            agent_semantic_runtime::codex_rollout_session_metadata(&cursor.as_str().into())?
                .ok_or_else(|| {
                    format!(
                        "Codex Host lifecycle rollout metadata is missing for ancestor {cursor}"
                    )
                })?;
        if let Some(root) = metadata.root_session_id() {
            if root.as_str() != payload_root_session_id {
                return Err(format!(
                    "Codex Host lifecycle ancestor root mismatch: payload={} rollout={}",
                    payload_root_session_id,
                    root.as_str()
                ));
            }
            return Ok(());
        }
        let parent = metadata.parent_thread_id().ok_or_else(|| {
            format!("Codex Host lifecycle parent_thread_id is missing for ancestor {cursor}")
        })?;
        if parent.as_str() == cursor {
            return Err(format!(
                "Codex Host lifecycle ancestor cannot be its own parent: ancestor={cursor}"
            ));
        }
        cursor = parent.as_str().to_owned();
    }
    Err("Codex Host lifecycle parent chain exceeded 64 typed edges".to_owned())
}

fn string_field<'a>(payload: &'a Value, name: &str) -> Option<&'a str> {
    payload
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_host_lifecycle.rs"]
mod tests;
