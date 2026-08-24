use super::choice_plane::{publish_child_session_registration, read_current_child_registration};
use agent_semantic_config::agent_route_registry::load_agent_route_registry_for_platform;

const CHILD_REGISTRATION_HANDOFF_ATTEMPTS: usize = 25;
const CHILD_REGISTRATION_HANDOFF_RETRY: std::time::Duration = std::time::Duration::from_millis(1);

fn wait_for_registration_handoff<T, E>(
    mut read: impl FnMut() -> Result<Option<T>, E>,
    mut matches_current_child: impl FnMut(&T) -> bool,
    attempts: usize,
    retry: std::time::Duration,
) -> Result<Option<T>, E> {
    for attempt in 0..attempts {
        if let Some(receipt) = read()? {
            if matches_current_child(&receipt) {
                return Ok(Some(receipt));
            }
        }
        if attempt + 1 < attempts && !retry.is_zero() {
            std::thread::sleep(retry);
        }
    }
    Ok(None)
}

pub(crate) struct HostChildSessionTopology {
    pub(super) child_session_id: String,
    pub(super) parent_session_id: String,
    pub(super) root_session_id: String,
}

fn verified_host_child_session_topology(
    metadata: &agent_semantic_runtime::CodexRolloutSessionMetadata,
    child_session_id: &str,
    parent_session_id: &str,
    root_session_id: &str,
) -> Result<HostChildSessionTopology, String> {
    let metadata_root = metadata
        .root_session_id()
        .map(|value| value.as_str())
        .ok_or_else(|| {
            "child-self-registration-root-session-required: Host rollout metadata has no root identity"
                .to_owned()
        })?;
    let metadata_parent = metadata
        .parent_thread_id()
        .map(|value| value.as_str())
        .ok_or_else(|| {
            "child-self-registration-parent-session-required: Host rollout metadata has no parent identity"
                .to_owned()
        })?;
    if metadata.session_id().as_str() != child_session_id
        || metadata_root != root_session_id
        || metadata_parent != parent_session_id
        || child_session_id == root_session_id
        || child_session_id == parent_session_id
    {
        return Err(
            "child-self-registration-host-topology-mismatch: child, parent, or root differs from verified Host rollout metadata"
                .to_owned(),
        );
    }
    Ok(HostChildSessionTopology {
        child_session_id: child_session_id.to_owned(),
        parent_session_id: parent_session_id.to_owned(),
        root_session_id: root_session_id.to_owned(),
    })
}

pub(crate) async fn register_child_session_from_host_payload(
    project_root: &std::path::Path,
    platform: &str,
    payload: &serde_json::Value,
    registered_session: Option<agent_semantic_client_db::AgentSessionRecord>,
) -> Result<Option<String>, String> {
    let Some(child_session_id) = payload
        .get("agent_id")
        .or_else(|| payload.get("agentId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let transcript_path = payload
        .get("transcript_path")
        .or_else(|| payload.get("transcriptPath"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "child-self-registration-transcript-required: typed Host child payload has no transcript path"
                .to_owned()
        })?;
    let metadata = agent_semantic_runtime::codex_rollout_session_metadata_at_path(
        std::path::Path::new(transcript_path),
    )?
    .ok_or_else(|| {
        "child-self-registration-rollout-metadata-required: transcript has no session metadata"
            .to_owned()
    })?;
    let payload_root_session_id = payload
        .get("root_session_id")
        .or_else(|| payload.get("rootSessionId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let payload_parent_session_id = payload
        .get("parent_session_id")
        .or_else(|| payload.get("parentSessionId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let root_session_id = metadata
        .root_session_id()
        .map(|value| value.as_str())
        .ok_or_else(|| {
            "child-self-registration-root-session-required: Host rollout metadata has no root identity"
                .to_owned()
        })?;
    let parent_session_id = metadata
        .parent_thread_id()
        .map(|value| value.as_str())
        .ok_or_else(|| {
            "child-self-registration-parent-session-required: Host rollout metadata has no parent identity"
                .to_owned()
        })?;
    if payload_root_session_id.is_some_and(|value| value != root_session_id)
        || payload_parent_session_id.is_some_and(|value| value != parent_session_id)
    {
        return Err(
            "child-self-registration-host-topology-mismatch: typed Host payload differs from verified rollout metadata"
                .to_owned(),
        );
    }
    let topology = verified_host_child_session_topology(
        &metadata,
        child_session_id,
        parent_session_id,
        root_session_id,
    )?;
    publish_child_session_registration(
        project_root,
        platform,
        topology,
        metadata,
        registered_session,
    )
    .await
    .map(Some)
}

pub(crate) fn read_child_session_registration_from_host_payload(
    project_root: &std::path::Path,
    platform: &str,
    payload: &serde_json::Value,
) -> Result<Option<String>, String> {
    let Some(child_session_id) = payload
        .get("agent_id")
        .or_else(|| payload.get("agentId"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let transcript_path = payload
        .get("transcript_path")
        .or_else(|| payload.get("transcriptPath"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "child-registration-receipt-transcript-required: typed Host child payload has no transcript path"
                .to_owned()
        })?;
    let metadata = agent_semantic_runtime::codex_rollout_session_metadata_at_path(
        std::path::Path::new(transcript_path),
    )?
    .ok_or_else(|| {
        "child-registration-receipt-rollout-metadata-required: transcript has no session metadata"
            .to_owned()
    })?;
    let root_session_id = metadata
        .root_session_id()
        .map(|value| value.as_str())
        .ok_or_else(|| "child-registration-receipt-root-session-required".to_owned())?;
    let parent_session_id = metadata
        .parent_thread_id()
        .map(|value| value.as_str())
        .ok_or_else(|| "child-registration-receipt-parent-session-required".to_owned())?;
    let topology = verified_host_child_session_topology(
        &metadata,
        child_session_id,
        parent_session_id,
        root_session_id,
    )?;
    let agent_role = metadata
        .agent_role()
        .ok_or_else(|| "child-registration-receipt-host-role-required".to_owned())?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)?;
    let loaded = load_agent_route_registry_for_platform(
        &state.state_home.join("agents/config.toml"),
        platform,
    )?;
    let route = loaded
        .compile_route_for_platform_host_agent_name(platform, agent_role)?
        .ok_or_else(|| {
            format!(
                "child-registration-receipt-host-role-unregistered: platform={platform} role={agent_role}"
            )
        })?;
    let payload_agent_type = payload
        .get("agent_type")
        .or_else(|| payload.get("agentType"))
        .and_then(serde_json::Value::as_str);
    if payload_agent_type != Some(route.platform_host_agent_name.as_str()) {
        return Err("child-registration-receipt-payload-agent-mismatch".to_owned());
    }
    let project_id = agent_semantic_client_db::AgentSessionRegistry::workspace_id(project_root)?;
    // SubagentStart and the child's first PreToolUse are independent Host Hook
    // deliveries. Give the already verified route a small, process-local handoff
    // window, but never accept the previous child receipt for the same route.
    let Some(receipt) = wait_for_registration_handoff(
        || {
            read_current_child_registration(
                project_root,
                &project_id,
                root_session_id,
                route.route_key.as_str(),
            )
        },
        |receipt| {
            receipt.child_session_id == topology.child_session_id
                && receipt.parent_session_id == topology.parent_session_id
        },
        CHILD_REGISTRATION_HANDOFF_ATTEMPTS,
        CHILD_REGISTRATION_HANDOFF_RETRY,
    )?
    else {
        return Ok(None);
    };
    let profile = std::fs::read(&route.profile_path).map_err(|error| {
        format!(
            "child-registration-receipt-profile-unavailable: {}: {error}",
            route.profile_path
        )
    })?;
    let profile_digest = format!("blake3-256:{}", blake3::hash(&profile).to_hex());
    let denied_actions = route
        .effective_permissions
        .denied_actions()
        .map(|action| action.as_str().to_owned())
        .collect::<Vec<_>>();
    let allowed_rule_intents = route.allowed_rule_intents.clone();
    if receipt.child_session_id != topology.child_session_id
        || receipt.parent_session_id != topology.parent_session_id
        || receipt.resident_id != route.platform_host_agent_name.as_str()
        || receipt.profile_digest != profile_digest
        || receipt.definition_schema_id != route.definition_schema_id
        || receipt.denied_actions != denied_actions
        || receipt.allowed_rule_intents != allowed_rule_intents
    {
        return Err("child-registration-receipt-current-profile-mismatch".to_owned());
    }
    serde_json::to_string(&receipt)
        .map(Some)
        .map_err(|error| format!("failed to encode current child registration receipt: {error}"))
}

#[cfg(test)]
#[path = "../../tests/unit/multi_agent_session_registration.rs"]
mod tests;
