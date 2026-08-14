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

async fn publish_host_lifecycle_event_locally(
    state_home: &Path,
    lifecycle_event: &mut AgentHostLifecycleEventIpc,
) -> Result<(), String> {
    const SCHEMA: &str = "agent.host-lifecycle-local-authority.v1";

    let namespace_key = lifecycle_event
        .namespace_id
        .strip_prefix("blake3-256:")
        .unwrap_or(&lifecycle_event.namespace_id);
    let authority_dir = state_home
        .join("hooks")
        .join("host-sessions")
        .join(namespace_key);
    tokio::fs::create_dir_all(&authority_dir)
        .await
        .map_err(|error| {
            format!(
                "failed to create Host lifecycle authority {}: {error}",
                authority_dir.display()
            )
        })?;

    let lock_path = authority_dir.join(".publication-lock");
    let mut lock_acquired = false;
    for _ in 0..100 {
        match tokio::fs::create_dir(&lock_path).await {
            Ok(()) => {
                lock_acquired = true;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Err(error) => {
                return Err(format!(
                    "failed to acquire Host lifecycle authority lock {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
    if !lock_acquired {
        return Err(format!(
            "timed out acquiring Host lifecycle authority lock {}",
            lock_path.display()
        ));
    }

    let authority_path = authority_dir.join("authority.v1.json");
    let publication = async {
        let mut authority = match tokio::fs::read(&authority_path).await {
            Ok(bytes) => serde_json::from_slice::<Value>(&bytes).map_err(|error| {
                format!(
                    "failed to decode Host lifecycle authority {}: {error}",
                    authority_path.display()
                )
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({
                "schema": SCHEMA,
                "projectId": lifecycle_event.project_id,
                "namespaceId": lifecycle_event.namespace_id,
                "lastSequence": 0,
                "events": [],
            }),
            Err(error) => {
                return Err(format!(
                    "failed to read Host lifecycle authority {}: {error}",
                    authority_path.display()
                ));
            }
        };
        if authority.get("schema").and_then(Value::as_str) != Some(SCHEMA)
            || authority.get("projectId").and_then(Value::as_str)
                != Some(lifecycle_event.project_id.as_str())
            || authority.get("namespaceId").and_then(Value::as_str)
                != Some(lifecycle_event.namespace_id.as_str())
        {
            return Err(format!(
                "Host lifecycle authority identity mismatch at {}",
                authority_path.display()
            ));
        }
        let last_sequence = authority
            .get("lastSequence")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let events = authority
            .get_mut("events")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                format!(
                    "Host lifecycle authority has no events array at {}",
                    authority_path.display()
                )
            })?;
        if let Some(sequence) = events.iter().find_map(|event| {
            (event.get("hostEventId").and_then(Value::as_str)
                == Some(lifecycle_event.host_event_id.as_str()))
            .then(|| event.get("sequence").and_then(Value::as_u64))
            .flatten()
        }) {
            lifecycle_event.host_event_sequence = sequence as _;
            return Ok(());
        }

        let sequence = last_sequence
            .checked_add(1)
            .ok_or_else(|| "Host lifecycle authority sequence exhausted".to_owned())?;
        lifecycle_event.host_event_sequence = sequence as _;
        events.push(serde_json::json!({
            "hostEventId": lifecycle_event.host_event_id,
            "sequence": sequence,
            "event": serde_json::to_value(&*lifecycle_event)
                .map_err(|error| format!("failed to encode Host lifecycle event: {error}"))?,
        }));
        authority["lastSequence"] = serde_json::json!(sequence);

        let encoded = serde_json::to_vec_pretty(&authority)
            .map_err(|error| format!("failed to encode Host lifecycle authority: {error}"))?;
        let temporary_path = authority_dir.join(format!(
            ".authority.v1.{}.{}.tmp",
            std::process::id(),
            lifecycle_event
                .host_event_id
                .trim_start_matches("blake3-256:")
        ));
        tokio::fs::write(&temporary_path, encoded)
            .await
            .map_err(|error| {
                format!(
                    "failed to write Host lifecycle authority temporary file {}: {error}",
                    temporary_path.display()
                )
            })?;
        if let Err(error) = tokio::fs::rename(&temporary_path, &authority_path).await {
            let _ = tokio::fs::remove_file(&temporary_path).await;
            return Err(format!(
                "failed to publish Host lifecycle authority {}: {error}",
                authority_path.display()
            ));
        }
        Ok(())
    }
    .await;
    let unlock_result = tokio::fs::remove_dir(&lock_path).await;
    match (publication, unlock_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(format!(
            "failed to release Host lifecycle authority lock {}: {error}",
            lock_path.display()
        )),
        (Ok(()), Ok(())) => Ok(()),
    }
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
    let mut lifecycle_event = AgentHostLifecycleEventIpc {
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
    publish_host_lifecycle_event_locally(&state.state_home, &mut lifecycle_event).await?;

    // The Host session authority above is the deterministic publication point.
    // Runtime admission and registry delivery are secondary reconciliation: a
    // missing endpoint or source generation must not erase an observed Host
    // lifecycle event or force the Host to recreate its native child.
    let _ = async {
        let Some(runtime_registry) =
            agent_semantic_client_db::AgentSessionRegistry::open_runtime_project_proxy(
                project_root,
            )?
        else {
            return Ok::<(), String>(());
        };
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
                            capability: serde_json::from_value(serde_json::json!("standard"))
                                .map_err(|error| {
                                    format!(
                                        "failed to decode standard delegation capability: {error}"
                                    )
                                })?,
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
        tokio::time::timeout(
            std::time::Duration::from_millis(100),
            runtime_registry.record_host_lifecycle_event_async(lifecycle_event.clone()),
        )
        .await
        .map_err(|_| "Host lifecycle Runtime reconciliation timed out".to_owned())??;
        Ok::<(), String>(())
    }
    .await;

    Ok(HostLifecycleDisposition::Recorded)
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
