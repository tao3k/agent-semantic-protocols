use super::choice_plane::publish_child_session_registration;

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
    publish_child_session_registration(project_root, platform, topology, metadata)
        .await
        .map(Some)
}

pub(crate) async fn register_child_session(
    project_root: &std::path::Path,
    platform: &str,
    child_session_id: String,
) -> Result<String, String> {
    let metadata = agent_semantic_runtime::codex_rollout_session_metadata(
        &child_session_id.clone().into(),
    )?
    .ok_or_else(|| {
        format!(
            "child-self-registration-rollout-metadata-required: no Codex rollout metadata for current child {child_session_id}"
        )
    })?;
    let root_session_id = metadata
        .root_session_id()
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            "child-self-registration-root-session-required: child rollout has no root identity"
                .to_owned()
        })?;
    let parent_session_id = metadata
        .parent_thread_id()
        .map(|value| value.as_str().to_owned())
        .ok_or_else(|| {
            "child-self-registration-parent-session-required: child rollout has no parent identity"
                .to_owned()
        })?;
    let topology = verified_host_child_session_topology(
        &metadata,
        &child_session_id,
        &parent_session_id,
        &root_session_id,
    )?;
    publish_child_session_registration(project_root, platform, topology, metadata).await
}

pub(crate) async fn register_current_child_session(
    project_root: &std::path::Path,
    platform: &str,
) -> Result<String, String> {
    if platform != "codex" {
        return Err(format!(
            "child-self-registration-host-unsupported: platform={platform}"
        ));
    }
    let child_session_id = std::env::var("CODEX_THREAD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("CODEX_SESSION_ID")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| {
            "child-self-registration-current-session-required: call the Host-created child before registration"
                .to_owned()
        })?;
    register_child_session(project_root, platform, child_session_id).await
}
