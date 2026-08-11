use std::path::Path;

use agent_semantic_client_db::workspace_db_ipc::{
    AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind,
};
use agent_semantic_config::agent_route_registry::load_agent_route_registry;
use serde_json::Value;

pub(super) enum HostLifecycleDisposition {
    NotLifecycle,
    Recorded,
}

pub(super) async fn record_host_lifecycle_event(
    client: &str,
    event: &str,
    payload: &Value,
    project_root: &Path,
) -> Result<HostLifecycleDisposition, String> {
    let Some(kind) = host_lifecycle_kind(client, event) else {
        return Ok(HostLifecycleDisposition::NotLifecycle);
    };
    let (root_session_id, child_session_id) = codex_payload_identity(payload)?;
    let parent_session_id = codex_rollout_parent_thread_id(child_session_id, root_session_id)?;
    let agent_type = required_payload_field(payload, "agent_type").map_err(|_| {
        "host-binding-evidence-unavailable: Codex lifecycle payload has no stable host task identity"
            .to_owned()
    })?;
    let state = agent_semantic_runtime::state_core::ResolvedState::resolve(project_root)?;
    let registry = load_agent_route_registry(&state.state_home.join("agents/config.toml"))?;
    let Some(route) = registry.compile_route_for_platform_host_agent_name("codex", agent_type)?
    else {
        // Ordinary Host workers are deliberately not coerced into ASP resident
        // identity. Their namespace decision is `none` and no ASP registration
        // or replacement is attempted.
        return Ok(HostLifecycleDisposition::Recorded);
    };
    let profile = tokio::fs::read(&route.profile_path)
        .await
        .map_err(|error| {
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
    let model = required_payload_field(payload, "model").map_err(|_| {
        "host-binding-evidence-unavailable: Codex lifecycle payload has no model identity"
            .to_owned()
    })?;
    let configured_model = route.model.as_deref().ok_or_else(|| {
        "host-binding-profile-model-required: matched Codex route has no configured model"
            .to_owned()
    })?;
    if model != configured_model {
        return Err(format!(
            "host-binding-model-drift: route={} configured={} observed={model}",
            route.route_key.as_str(),
            configured_model,
        ));
    }
    let sandbox_mode = route.sandbox_mode.as_deref().ok_or_else(|| {
        "host-binding-sandbox-required: matched Codex route has no configured sandbox".to_owned()
    })?;
    let observed_duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("failed to resolve Host lifecycle timestamp: {error}"))?;
    let observed_at = observed_duration.as_secs() as i64;
    let project_id = agent_semantic_client_db::AgentSessionRegistry::workspace_id(project_root)?;
    let payload_digest = format!(
        "blake3-256:{}",
        blake3::hash(payload.to_string().as_bytes()).to_hex()
    );
    let profile_digest = format!("blake3-256:{}", blake3::hash(&profile).to_hex());
    let event_identity = format!(
        "{client}\0{event}\0{project_id}\0{root_session_id}\0{parent_session_id}\0{child_session_id}\0{agent_type}\0{payload_digest}"
    );
    let namespace_identity = format!(
        "codex\0{project_id}\0{root_session_id}\0{}",
        route.route_key.as_str()
    );
    let lifecycle_event = AgentHostLifecycleEventIpc {
        host_event_id: format!(
            "blake3-256:{}",
            blake3::hash(event_identity.as_bytes()).to_hex()
        ),
        host_event_sequence: 0,
        namespace_id: format!(
            "blake3-256:{}",
            blake3::hash(namespace_identity.as_bytes()).to_hex()
        ),
        kind,
        platform: "codex".to_owned(),
        project_id,
        root_session_id: root_session_id.to_owned(),
        parent_session_id,
        child_session_id: child_session_id.to_owned(),
        host_task_name: agent_type.to_owned(),
        platform_host_agent_name: route.platform_host_agent_name.as_str().to_owned(),
        route_key: route.route_key.as_str().to_owned(),
        profile_id: route.profile_path.clone(),
        role,
        model: model.to_owned(),
        model_digest: format!("blake3-256:{}", blake3::hash(model.as_bytes()).to_hex()),
        profile_digest,
        sandbox_mode: sandbox_mode.to_owned(),
        session_lifetime: route.session_lifetime.as_str().to_owned(),
        payload_digest,
        transcript_path: string_field(payload, "transcript_path").map(str::to_owned),
        observed_at,
    };
    crate::server::runtime_server_hook_mutation::ensure(
        project_root,
        lifecycle_event.host_event_id.clone(),
    )
    .await?
    .validate()?;
    let runtime_registry =
        agent_semantic_client_db::AgentSessionRegistry::open_runtime_project_proxy(project_root)?
            .ok_or_else(|| {
                "host-lifecycle-runtime-registry-required: generation admission completed without a Runtime registry proxy"
                    .to_owned()
            })?;
    if lifecycle_event.kind
        == agent_semantic_client_db::workspace_db_ipc::AgentHostLifecycleEventKind::Started
    {
        if lifecycle_event.parent_session_id == lifecycle_event.root_session_id {
            runtime_registry
                .register_control_plane_agent(
                    agent_semantic_client_db::SessionControlPlaneAgentRegistration {
                        project_id: lifecycle_event.project_id.clone(),
                        root_session_id: lifecycle_event.root_session_id.clone(),
                        session_id: lifecycle_event.root_session_id.clone(),
                        parent_session_id: None,
                        resident_name: "codex-root".to_owned(),
                        capability: serde_json::from_value(serde_json::json!("standard")).map_err(
                            |error| {
                                format!("failed to decode standard delegation capability: {error}")
                            },
                        )?,
                    },
                )
                .await?;
        }
        let snapshot = runtime_registry
            .read_control_plane_snapshot(
                lifecycle_event.project_id.clone(),
                lifecycle_event.root_session_id.clone(),
            )
            .await?;
        let proposed_child_capability = match route.focus_mode {
            agent_semantic_config::agent_route_registry::AgentFocusMode::Standard => {
                serde_json::from_value(serde_json::json!("standard")).map_err(|error| {
                    format!("failed to decode standard delegation capability: {error}")
                })?
            }
            agent_semantic_config::agent_route_registry::AgentFocusMode::Leaf => {
                serde_json::from_value(serde_json::json!("focused-leaf")).map_err(|error| {
                    format!("failed to decode focused-leaf delegation capability: {error}")
                })?
            }
        };
        let transaction = runtime_registry
            .admit_control_plane_delegation(
                agent_semantic_client_db::SessionControlPlaneDelegationProposal {
                    event_id: lifecycle_event.host_event_id.clone(),
                    project_id: lifecycle_event.project_id.clone(),
                    root_session_id: lifecycle_event.root_session_id.clone(),
                    current_session_id: lifecycle_event.parent_session_id.clone(),
                    proposed_child_session_id: lifecycle_event.child_session_id.clone(),
                    proposed_child_resident_name: lifecycle_event.route_key.clone(),
                    proposed_child_capability,
                    expected_generation: snapshot.generation,
                    evidence_refs: vec![
                        lifecycle_event.payload_digest.clone(),
                        lifecycle_event.profile_digest.clone(),
                        format!("route:{}", lifecycle_event.route_key),
                    ],
                    observed_at_ms: lifecycle_event.observed_at.saturating_mul(1_000),
                },
            )
            .await?;
        if serde_json::to_value(&transaction.admission.decision)
            .map_err(|error| format!("failed to encode delegation decision: {error}"))?
            == serde_json::json!("denied")
        {
            return Err(transaction
                .admission
                .reason_kind
                .unwrap_or_else(|| "focused-agent-delegation-denied".to_owned()));
        }
    }
    if let Ok(Some(runtime_registry)) =
        agent_semantic_client_db::AgentSessionRegistry::open_runtime_project_proxy(project_root)
        && tokio::time::timeout(
            // Host lifecycle delivery is a local Runtime IPC round trip, not
            // part of the synchronous policy-classification budget.  A
            // sub-millisecond deadline made healthy Runtime registrations
            // spuriously fall back to the workspace mmap inbox, which a
            // read-only Agent cannot acknowledge.  Keep the hand-off bounded
            // while allowing the Runtime owner to durably register the child.
            std::time::Duration::from_millis(100),
            runtime_registry.record_host_lifecycle_event_async(lifecycle_event.clone()),
        )
        .await
        .is_ok_and(|result| result.is_ok())
    {
        return Ok(HostLifecycleDisposition::Recorded);
    }
    Err("host-lifecycle-runtime-delivery-required".to_owned())
}

fn host_lifecycle_kind(client: &str, event: &str) -> Option<AgentHostLifecycleEventKind> {
    if client != "codex" {
        return None;
    }
    match event {
        "subagent-start" => Some(AgentHostLifecycleEventKind::Started),
        "subagent-resume" => Some(AgentHostLifecycleEventKind::Resumed),
        "subagent-stop" => Some(AgentHostLifecycleEventKind::Stopped),
        "subagent-achieved" => Some(AgentHostLifecycleEventKind::Achieved),
        _ => None,
    }
}

fn required_payload_field<'a>(payload: &'a Value, name: &str) -> Result<&'a str, String> {
    string_field(payload, name)
        .ok_or_else(|| format!("Codex Host lifecycle payload requires `{name}`"))
}

fn codex_payload_identity(payload: &Value) -> Result<(&str, &str), String> {
    let root_session_id = required_payload_field(payload, "session_id").map_err(|_| {
        "host-binding-evidence-unavailable: Codex lifecycle payload has no root session identity"
            .to_owned()
    })?;
    let child_session_id = required_payload_field(payload, "agent_id").map_err(|_| {
        "host-binding-evidence-unavailable: Codex lifecycle payload has no stable child identity"
            .to_owned()
    })?;
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
mod hook_runtime_host_lifecycle_tests;
