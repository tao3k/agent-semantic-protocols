//! Rollout-derived topology decisions for agent-session hooks.

use super::{
    AspSessionPolicy, DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, agent_session_route_fields, append_resident_agent_fields, string_field,
};
use agent_semantic_runtime::{codex_rollout_session_metadata, current_agent_runtime_session};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct CurrentRolloutTopology {
    pub(super) session_id: String,
    root_session_id: Option<String>,
    parent_thread_id: Option<String>,
    thread_source: Option<String>,
    agent_role: Option<String>,
    spawn_depth: Option<i64>,
}

impl CurrentRolloutTopology {
    pub(super) fn is_resident_subagent(&self, asp_session_policy: &AspSessionPolicy) -> bool {
        self.thread_source.as_deref() == Some("subagent")
            && self.agent_role.as_deref().is_some_and(|role| {
                configured_resident_identity_matches(
                    normalize_agent_session_role(role).as_str(),
                    asp_session_policy,
                )
            })
    }

    pub(super) fn is_nested_resident_subagent(
        &self,
        asp_session_policy: &AspSessionPolicy,
    ) -> bool {
        self.is_resident_subagent(asp_session_policy)
            && (self.spawn_depth != Some(1)
                || self.parent_thread_id.as_deref() != self.root_session_id.as_deref())
    }
}

pub(super) fn current_rollout_topology() -> Result<Option<CurrentRolloutTopology>, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(None);
    };
    let Some(metadata) = codex_rollout_session_metadata(session.recall_session_id())? else {
        return Ok(None);
    };
    Ok(Some(CurrentRolloutTopology {
        session_id: metadata.session_id,
        root_session_id: metadata.root_session_id,
        parent_thread_id: metadata.parent_thread_id,
        thread_source: metadata.thread_source,
        agent_role: metadata.agent_role,
        spawn_depth: metadata.spawn_depth,
    }))
}

pub(super) fn register_required_resident_child_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    topology: &CurrentRolloutTopology,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields = agent_session_route_fields("register-existing-child", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    append_rollout_topology_fields(&mut fields, topology);
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("register-required".to_string()),
    );
    fields.insert(
        "agentSessionBootstrapGuideCommand".to_string(),
        serde_json::Value::String(register_existing_child_command(
            asp_session_policy,
            topology,
        )),
    );
    let root_session_id = topology.root_session_id.as_deref().unwrap_or("");
    let parent_thread_id = topology.parent_thread_id.as_deref().unwrap_or("");
    let message = format!(
        "ASP resident child session is not registered. Register this exact child before running restricted ASP work.\nchildSessionId={}\nrootSessionId={root_session_id}\nparentThreadId={parent_thread_id}\nregisterCommand={}",
        topology.session_id,
        register_existing_child_command(asp_session_policy, topology)
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}

pub(super) fn nested_resident_child_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    topology: &CurrentRolloutTopology,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields =
        agent_session_route_fields("nested-resident-child-denied", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    append_rollout_topology_fields(&mut fields, topology);
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("nested-resident-child-denied".to_string()),
    );
    let message = format!(
        "ASP denied nested resident child lifecycle. This session is already a configured resident child and must not create or route through another resident child.\nchildSessionId={}\nrootSessionId={}\nparentThreadId={}\nspawnDepth={}",
        topology.session_id,
        topology.root_session_id.as_deref().unwrap_or(""),
        topology.parent_thread_id.as_deref().unwrap_or(""),
        topology
            .spawn_depth
            .map(|depth| depth.to_string())
            .unwrap_or_else(|| "<missing>".to_string())
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::RawBroadSearch,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: string_field(payload, &["tool_name", "toolName"]),
            command: None,
            paths: Vec::new(),
        },
        routes: Vec::new(),
        message,
        fields,
    }
}

fn normalize_agent_session_role(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn configured_resident_identity_matches(
    normalized_role: &str,
    asp_session_policy: &AspSessionPolicy,
) -> bool {
    [
        asp_session_policy.resident_child_name(),
        asp_session_policy.resident_agent_role(),
        asp_session_policy.resident_codex_agent_name(),
    ]
    .iter()
    .any(|candidate| normalize_agent_session_role(candidate) == normalized_role)
}

fn register_existing_child_command(
    asp_session_policy: &AspSessionPolicy,
    topology: &CurrentRolloutTopology,
) -> String {
    let resident_child_name = asp_session_policy.resident_child_name();
    let resident_agent_role = asp_session_policy.resident_agent_role();
    format!(
        "asp agent session register --name {resident_child_name} --child-session-id {} --root-session-id {} --role {resident_agent_role}",
        topology.session_id,
        topology.root_session_id.as_deref().unwrap_or("")
    )
}

fn append_rollout_topology_fields(
    fields: &mut BTreeMap<String, serde_json::Value>,
    topology: &CurrentRolloutTopology,
) {
    fields.insert(
        "childSessionId".to_string(),
        serde_json::Value::String(topology.session_id.clone()),
    );
    if let Some(root_session_id) = topology.root_session_id.as_ref() {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id.clone()),
        );
    }
    if let Some(parent_thread_id) = topology.parent_thread_id.as_ref() {
        fields.insert(
            "parentThreadId".to_string(),
            serde_json::Value::String(parent_thread_id.clone()),
        );
    }
    if let Some(thread_source) = topology.thread_source.as_ref() {
        fields.insert(
            "threadSource".to_string(),
            serde_json::Value::String(thread_source.clone()),
        );
    }
    if let Some(agent_role) = topology.agent_role.as_ref() {
        fields.insert(
            "agentRole".to_string(),
            serde_json::Value::String(agent_role.clone()),
        );
    }
    if let Some(spawn_depth) = topology.spawn_depth {
        fields.insert(
            "spawnDepth".to_string(),
            serde_json::Value::Number(serde_json::Number::from(spawn_depth)),
        );
    }
}
