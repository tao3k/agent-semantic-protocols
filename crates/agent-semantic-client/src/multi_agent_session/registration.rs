use super::choice_plane::publish_child_session_registration;
use agent_semantic_config::agent_route_registry::load_agent_route_registry_for_platform;

async fn read_or_materialize_registration_handoff<T, E, Read, Matches, Publish, PublishFuture>(
    mut read: Read,
    mut matches_current_child: Matches,
    publish: Publish,
) -> Result<T, E>
where
    Read: FnMut() -> Result<Option<T>, E>,
    Matches: FnMut(&T) -> bool,
    Publish: FnOnce() -> PublishFuture,
    PublishFuture: std::future::Future<Output = Result<T, E>>,
{
    if let Some(receipt) = read()? {
        if matches_current_child(&receipt) {
            return Ok(receipt);
        }
    }
    publish().await
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

pub(crate) fn canonical_agent_route_registry_path(
    global_state_home: &std::path::Path,
) -> std::path::PathBuf {
    global_state_home.join("agents/config.toml")
}

fn digest_domain_parts<I, S>(domain: &str, parts: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.as_bytes());
    for part in parts {
        let bytes = part.as_ref().as_bytes();
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

pub(crate) fn route_binding_digest(
    platform: &str,
    canonical_agent_name: &str,
    route_key: &str,
    profile_digest: &str,
) -> String {
    digest_domain_parts(
        "agent-semantic-protocols/child-registration-route/v1",
        [platform, canonical_agent_name, route_key, profile_digest],
    )
}

pub(crate) fn route_policy_digest(
    route_key: &str,
    denied_actions: &[String],
    allowed_rule_intents: &[String],
) -> String {
    let mut parts = Vec::with_capacity(1 + denied_actions.len() + allowed_rule_intents.len());
    parts.push(format!("route:{route_key}"));
    parts.extend(denied_actions.iter().map(|action| format!("deny:{action}")));
    parts.extend(
        allowed_rule_intents
            .iter()
            .map(|intent| format!("allow:{intent}")),
    );
    digest_domain_parts(
        "agent-semantic-protocols/child-registration-policy/v1",
        parts,
    )
}

fn child_registration_receipt_schema_is_current(
    receipt: &super::choice_plane::ChildSessionRegistrationReceipt,
) -> bool {
    receipt.schema_id == super::CHILD_REGISTRATION_SCHEMA_ID
        && receipt.schema_version == 1
        && receipt.generation > 0
        && !receipt.root_session_id.trim().is_empty()
        && !receipt.parent_session_id.trim().is_empty()
        && !receipt.child_session_id.trim().is_empty()
        && !receipt.host_role.trim().is_empty()
        && !receipt.canonical_agent_name.trim().is_empty()
        && !receipt.platform.trim().is_empty()
        && !receipt.route_key.trim().is_empty()
        && !receipt.route_digest.trim().is_empty()
        && !receipt.profile_digest.trim().is_empty()
        && !receipt.policy_digest.trim().is_empty()
}

pub(crate) async fn read_child_session_registration_from_host_payload(
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
    let transcript_path = std::path::PathBuf::from(transcript_path);
    let metadata = tokio::task::spawn_blocking(move || {
        agent_semantic_runtime::codex_rollout_session_metadata_at_path(&transcript_path)
    })
    .await
    .map_err(|error| format!("child-registration-receipt-rollout-task-failed: {error}"))??
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
    let agent_state_home = agent_semantic_runtime::resolve_state_home()?;
    let agent_registry_path = canonical_agent_route_registry_path(&agent_state_home);
    let loaded = load_agent_route_registry_for_platform(&agent_registry_path, platform)?;
    let route = loaded
        .compile_route_for_platform_host_identity(platform, agent_role)?
        .ok_or_else(|| {
            format!(
                "child-registration-receipt-host-role-unregistered: platform={platform} role={agent_role}"
            )
        })?;
    let payload_agent_type = payload
        .get("agent_type")
        .or_else(|| payload.get("agentType"))
        .and_then(serde_json::Value::as_str)
        .filter(|identity| !identity.trim().is_empty())
        .ok_or_else(|| "child-registration-receipt-payload-agent-required".to_owned())?;
    let payload_route = loaded
        .compile_route_for_platform_host_identity(platform, payload_agent_type)?
        .ok_or_else(|| {
            format!(
                "child-registration-receipt-payload-agent-unregistered: platform={platform} identity={payload_agent_type}"
            )
        })?;
    if payload_route.route_key != route.route_key {
        return Err("child-registration-receipt-payload-agent-mismatch".to_owned());
    }
    let project_id = agent_semantic_client_db::AgentSessionRegistry::workspace_id(project_root)?;
    // The child's first PreToolUse owns one atomic read-or-materialize
    // transition. There is no preceding lifecycle event and no polling window.
    let receipt = read_or_materialize_registration_handoff(
        || {
            super::choice_plane::read_replaceable_child_registration(
                project_root,
                &project_id,
                root_session_id,
                parent_session_id,
                child_session_id,
                route.route_key.as_str(),
            )
        },
    |receipt| {
        child_registration_receipt_schema_is_current(receipt)
            && receipt.child_session_id == topology.child_session_id
            && receipt.parent_session_id == topology.parent_session_id
    },
        || async {
            let receipt_json = register_child_session_from_host_payload(
                project_root,
                platform,
                payload,
                None,
            )
            .await?
            .ok_or_else(|| {
                "child-registration-receipt-host-delegation-required: typed Host child payload did not materialize a registration receipt"
                    .to_owned()
            })?;
            serde_json::from_str(&receipt_json).map_err(|error| {
                format!("failed to decode newly materialized child registration receipt: {error}")
            })
        },
    )
    .await?;
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
    if receipt.host_role.trim().is_empty()
        || receipt.canonical_agent_name.trim().is_empty()
        || receipt.platform.trim().is_empty()
        || receipt.route_digest.trim().is_empty()
        || receipt.policy_digest.trim().is_empty()
    {
        return Err("child-registration-receipt-schema-stale-reregister-required".to_owned());
    }
    let receipt_host_route = loaded
        .compile_route_for_platform_host_identity(platform, &receipt.host_role)?
        .ok_or_else(|| "child-registration-receipt-host-role-stale".to_owned())?;
    let receipt_canonical_route = loaded
        .compile_route_for_platform_host_identity(platform, &receipt.canonical_agent_name)?
        .ok_or_else(|| "child-registration-receipt-canonical-agent-stale".to_owned())?;
    let route_digest = route_binding_digest(
        platform,
        route.platform_host_agent_name.as_str(),
        route.route_key.as_str(),
        &profile_digest,
    );
    let policy_digest = route_policy_digest(
        route.route_key.as_str(),
        &denied_actions,
        &allowed_rule_intents,
    );
    if receipt.child_session_id != topology.child_session_id
        || receipt.parent_session_id != topology.parent_session_id
        || receipt.root_session_id != root_session_id
        || receipt.platform != platform
        || receipt_host_route.route_key != route.route_key
        || receipt_canonical_route.route_key != route.route_key
        || receipt.resident_id != route.platform_host_agent_name.as_str()
        || receipt.canonical_agent_name != route.platform_host_agent_name.as_str()
        || receipt.route_key != route.route_key.as_str()
        || receipt.route_digest != route_digest
        || receipt.profile_digest != profile_digest
        || receipt.policy_digest != policy_digest
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
