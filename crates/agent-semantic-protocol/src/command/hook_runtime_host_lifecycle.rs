use std::path::Path;

use agent_semantic_client_db::workspace_db_ipc::{
    AgentHostLifecycleEventIpc, AgentHostLifecycleEventKind, AgentSessionRegistryIpcResult,
};
use agent_semantic_client_db::{
    SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
};
use agent_semantic_config::agent_route_registry::load_agent_route_registry;
use agent_semantic_context_product::agent_session_delegation_admission::{
    AgentSessionDelegationCapability, AgentSessionDelegationDecision,
};
use serde_json::Value;

fn focused_nested_subagent_denial(
    client: &str,
    event: &str,
    parent_session_id: &str,
    child_session_id: &str,
    child_agent_type: &str,
) -> agent_semantic_hook::HookDecision {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "focusMode".to_owned(),
        serde_json::Value::String("leaf".to_owned()),
    );
    fields.insert(
        "parentSessionId".to_owned(),
        serde_json::Value::String(parent_session_id.to_owned()),
    );
    fields.insert(
        "parentCapability".to_owned(),
        serde_json::Value::String("focused-leaf".to_owned()),
    );
    fields.insert(
        "childSessionId".to_owned(),
        serde_json::Value::String(child_session_id.to_owned()),
    );
    fields.insert(
        "childAgentType".to_owned(),
        serde_json::Value::String(child_agent_type.to_owned()),
    );
    agent_semantic_hook::HookDecision {
        schema_id: agent_semantic_hook::HOOK_DECISION_SCHEMA_ID,
        schema_version: agent_semantic_hook::HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: agent_semantic_hook::HOOK_PROTOCOL_ID,
        protocol_version: agent_semantic_hook::HOOK_PROTOCOL_VERSION,
        platform: client.to_owned(),
        event: event.to_owned(),
        decision: agent_semantic_hook::DecisionKind::Deny,
        reason_kind: agent_semantic_hook::ReasonKind::FocusedSubagentNestedStart,
        language_ids: Vec::new(),
        subject: agent_semantic_hook::DecisionSubject::default(),
        routes: Vec::new(),
        message: format!(
            "registered ASP focused-leaf session `{parent_session_id}` cannot start nested subagent `{child_agent_type}`"
        ),
        fields,
    }
}

pub(super) enum HostLifecycleDisposition {
    NotLifecycle,
    Recorded,
    Denied(agent_semantic_hook::HookDecision),
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
    let observed_at_ms = i64::try_from(observed_duration.as_millis())
        .map_err(|_| "Host lifecycle timestamp exceeds i64 milliseconds".to_owned())?;
    let runtime_registry =
        agent_semantic_client_db::AgentSessionRegistry::open_runtime_project_proxy(project_root)?
            .ok_or_else(|| "Runtime Server agent-session registry proxy is unavailable".to_owned())?;
    let project_id = agent_semantic_client_db::AgentSessionRegistry::workspace_id(project_root)?;
    let payload_digest = format!(
        "blake3-256:{}",
        blake3::hash(payload.to_string().as_bytes()).to_hex()
    );
    let profile_digest = format!("blake3-256:{}", blake3::hash(&profile).to_hex());
    if matches!(kind, AgentHostLifecycleEventKind::Started) {
        if parent_session_id == root_session_id {
            runtime_registry
                .register_control_plane_agent(SessionControlPlaneAgentRegistration {
                    project_id: project_id.clone(),
                    root_session_id: root_session_id.to_owned(),
                    session_id: root_session_id.to_owned(),
                    parent_session_id: None,
                    resident_name: "codex-root".to_owned(),
                    capability: AgentSessionDelegationCapability::Standard,
                })
                .await?;
        }
        let snapshot = runtime_registry
            .read_control_plane_snapshot(project_id.clone(), root_session_id.to_owned())
            .await?;
        let proposed_child_capability = match route.focus_mode {
            agent_semantic_config::agent_route_registry::AgentFocusMode::Standard => {
                AgentSessionDelegationCapability::Standard
            }
            agent_semantic_config::agent_route_registry::AgentFocusMode::Leaf => {
                AgentSessionDelegationCapability::FocusedLeaf
            }
        };
        let event_identity = format!(
            "{client}\0{event}\0{project_id}\0{root_session_id}\0{parent_session_id}\0{child_session_id}"
        );
        let transaction_receipt = runtime_registry
            .admit_control_plane_delegation(SessionControlPlaneDelegationProposal {
                event_id: format!(
                    "blake3-256:{}",
                    blake3::hash(event_identity.as_bytes()).to_hex()
                ),
                project_id: project_id.clone(),
                root_session_id: root_session_id.to_owned(),
                current_session_id: parent_session_id.clone(),
                proposed_child_session_id: child_session_id.to_owned(),
                proposed_child_resident_name: route.route_key.as_str().to_owned(),
                proposed_child_capability,
                expected_generation: snapshot.generation,
                evidence_refs: vec![
                    payload_digest.clone(),
                    profile_digest.clone(),
                    format!("route:{}", route.route_key.as_str()),
                ],
                observed_at_ms,
            })
            .await?;
        match transaction_receipt.admission.decision {
            AgentSessionDelegationDecision::Accepted => {}
            AgentSessionDelegationDecision::Denied => {
                let reason_kind = transaction_receipt
                    .admission
                    .reason_kind
                    .as_deref()
                    .unwrap_or("focused-agent-delegation-denied");
                if reason_kind != "focused-agent-delegation-denied" {
                    return Err(format!(
                        "unexpected Session Control Plane delegation denial: {reason_kind}"
                    ));
                }
                return Ok(HostLifecycleDisposition::Denied(
                    focused_nested_subagent_denial(
                        client,
                        event,
                        &parent_session_id,
                        child_session_id,
                        agent_type,
                    ),
                ));
            }
        }
    }
    let result = runtime_registry
        .record_host_lifecycle_event_async(AgentHostLifecycleEventIpc {
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
        })
        .await?;
    match (kind, result) {
        (
            AgentHostLifecycleEventKind::Started,
            AgentSessionRegistryIpcResult::Registered { .. },
        )
        | (AgentHostLifecycleEventKind::Stopped, AgentSessionRegistryIpcResult::Changed { .. }) => {
            Ok(HostLifecycleDisposition::Recorded)
        }
        _ => Err("Runtime Server returned an unexpected Host lifecycle result".to_owned()),
    }
}

fn host_lifecycle_kind(client: &str, event: &str) -> Option<AgentHostLifecycleEventKind> {
    if client != "codex" {
        return None;
    }
    match event {
        "subagent-start" => Some(AgentHostLifecycleEventKind::Started),
        "subagent-stop" => Some(AgentHostLifecycleEventKind::Stopped),
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
mod tests;
#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_host_lifecycle.rs"]
mod hook_runtime_host_lifecycle_tests;
