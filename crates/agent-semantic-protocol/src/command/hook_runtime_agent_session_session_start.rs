//! Session-start bootstrap and route-context decisions for resident ASP children.

use crate::command::{
    asp_explore_session_for_current_root, asp_explore_session_record_for_current_root,
    current_registered_session, current_root_session_id, has_current_agent_session,
};
use agent_semantic_client_db::AgentSessionRecord;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use std::path::Path;

use super::hook_runtime_agent_session_command::command_requires_resident_child;
use super::hook_runtime_agent_session_rollout_topology::{
    CurrentRolloutTopology, current_rollout_topology, nested_resident_child_decision,
    register_required_resident_child_decision,
};
use super::{
    AspSessionPolicy, agent_session_route_fields, append_resident_agent_fields,
    render_agent_session_template, resident_child_create_action, string_field, template_value,
    unix_timestamp,
};

pub(super) struct MainSessionRouteContext {
    pub(super) has_agent_session: bool,
    pub(super) current_session: Option<AgentSessionRecord>,
    pub(super) active_explore_session: Option<AgentSessionRecord>,
    pub(super) active_testing_session: Option<AgentSessionRecord>,
    pub(super) root_session_id: Option<String>,
    current_rollout_topology: Option<CurrentRolloutTopology>,
}

impl MainSessionRouteContext {
    pub(super) fn current_is_active_resident_child(
        &self,
        now: i64,
        asp_session_policy: &AspSessionPolicy,
    ) -> bool {
        if self
            .current_rollout_topology
            .as_ref()
            .is_some_and(|topology| topology.is_direct_resident_subagent(asp_session_policy))
        {
            return true;
        }
        self.current_session.as_ref().is_some_and(|session| {
            session_matches_resident_agent(
                session,
                asp_session_policy.resident_child_name(),
                asp_session_policy.resident_agent_role(),
            ) && session.is_routable_at(now)
        })
    }

    pub(super) fn current_is_active_testing_child(
        &self,
        now: i64,
        asp_session_policy: &AspSessionPolicy,
    ) -> bool {
        self.current_session.as_ref().is_some_and(|session| {
            session_matches_resident_agent(
                session,
                asp_session_policy.testing_resident_child_name(),
                asp_session_policy.testing_resident_agent_role(),
            ) && session.is_routable_at(now)
        })
    }

    pub(super) fn outside_agent_session(&self) -> bool {
        !self.has_agent_session
            && self.current_session.is_none()
            && self.active_explore_session.is_none()
            && self.active_testing_session.is_none()
    }

    pub(super) fn needs_bootstrap_for(
        &self,
        commands: &[String],
        asp_session_policy: &AspSessionPolicy,
    ) -> bool {
        self.active_explore_session.is_none()
            && commands.iter().any(|command| {
                command_requires_resident_child(command, |tokens, index| {
                    asp_session_policy.main_asp_command_allowed(tokens, index)
                })
            })
    }

    pub(super) fn current_register_required_resident_child(
        &self,
        asp_session_policy: &AspSessionPolicy,
    ) -> Option<&CurrentRolloutTopology> {
        self.current_rollout_topology.as_ref().filter(|topology| {
            topology.is_resident_subagent(asp_session_policy)
                && !topology.is_nested_resident_subagent(asp_session_policy)
                && self.current_session.is_none()
        })
    }

    pub(super) fn current_nested_resident_child(
        &self,
        asp_session_policy: &AspSessionPolicy,
    ) -> Option<&CurrentRolloutTopology> {
        self.current_rollout_topology
            .as_ref()
            .filter(|topology| topology.is_nested_resident_subagent(asp_session_policy))
    }
}

pub(super) fn main_session_route_context(
    project_root: &Path,
    asp_session_policy: &AspSessionPolicy,
) -> Result<MainSessionRouteContext, String> {
    let current_rollout_topology = current_rollout_topology()?;
    let rollout_direct_resident_child = current_rollout_topology
        .as_ref()
        .is_some_and(|topology| topology.is_direct_resident_subagent(asp_session_policy));
    let current_session = registry_lookup_for_route_child(
        current_registered_session(project_root),
        rollout_direct_resident_child,
    )?;
    let now = unix_timestamp()?;
    let active_explore_session = registry_lookup_for_route_child(
        asp_explore_session_for_current_root(
            project_root,
            asp_session_policy.resident_child_name(),
        ),
        rollout_direct_resident_child,
    )?
    .filter(|session| {
        session_matches_resident_agent(
            session,
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        ) && session.is_routable_at(now)
    });
    let active_testing_session = registry_lookup_for_route_child(
        asp_explore_session_for_current_root(
            project_root,
            asp_session_policy.testing_resident_child_name(),
        ),
        rollout_direct_resident_child,
    )?
    .filter(|session| {
        session_matches_resident_agent(
            session,
            asp_session_policy.testing_resident_child_name(),
            asp_session_policy.testing_resident_agent_role(),
        ) && session.is_routable_at(now)
    });
    let root_session_id = current_root_session_id().or_else(|| {
        current_rollout_topology
            .as_ref()
            .and_then(|topology| topology.root_session_id().map(str::to_string))
    });
    Ok(MainSessionRouteContext {
        has_agent_session: has_current_agent_session(),
        current_session,
        active_explore_session,
        active_testing_session,
        root_session_id,
        current_rollout_topology,
    })
}

pub(super) fn classify_session_start_bootstrap(
    project_root: &Path,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    asp_session_policy: &AspSessionPolicy,
) -> Result<Option<HookDecision>, String> {
    if !has_current_agent_session() {
        return Ok(None);
    }
    let now = unix_timestamp()?;
    let current_rollout_topology = current_rollout_topology()?;
    let rollout_direct_resident_child = current_rollout_topology
        .as_ref()
        .is_some_and(|topology| topology.is_direct_resident_subagent(asp_session_policy));
    if registry_lookup_for_route_child(
        current_registered_session(project_root),
        rollout_direct_resident_child,
    )?
    .as_ref()
    .is_some_and(|session| {
        session_matches_resident_agent(
            session,
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        ) && session.is_routable_at(now)
    }) {
        return Ok(None);
    }
    if let Some(topology) = current_rollout_topology {
        if topology.is_nested_resident_subagent(asp_session_policy) {
            return Ok(Some(nested_resident_child_decision(
                platform,
                event,
                payload,
                &topology,
                asp_session_policy,
            )));
        }
        if topology.is_resident_subagent(asp_session_policy) {
            return Ok(Some(register_required_resident_child_decision(
                platform,
                event,
                payload,
                &topology,
                asp_session_policy,
            )));
        }
    }
    let active_explore_session = asp_explore_session_for_current_root(
        project_root,
        asp_session_policy.resident_child_name(),
    )?
    .filter(|session| {
        session_matches_resident_agent(
            session,
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        ) && session.is_routable_at(now)
    });
    if let Some(session) = active_explore_session.as_ref() {
        return Ok(Some(session_start_reuse_decision(
            platform,
            event,
            payload,
            session,
            asp_session_policy,
        )));
    }
    let existing_explore_session = asp_explore_session_record_for_current_root(
        project_root,
        asp_session_policy.resident_child_name(),
    )?
    .filter(|session| {
        session_matches_resident_agent(
            session,
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        )
    });
    if let Some(session) = existing_explore_session.as_ref() {
        return Ok(Some(session_start_resume_existing_decision(
            platform,
            event,
            payload,
            session,
            asp_session_policy,
        )));
    }
    Ok(Some(session_start_bootstrap_decision(
        platform,
        event,
        payload,
        current_root_session_id(),
        asp_session_policy,
    )))
}

fn session_matches_resident_agent(
    session: &AgentSessionRecord,
    resident_child_name: &str,
    resident_agent_role: &str,
) -> bool {
    session.name == resident_child_name
        || legacy_resident_agent_role_matches(&session.role, resident_agent_role)
}

fn legacy_resident_agent_role_matches(session_role: &str, resident_agent_role: &str) -> bool {
    session_role == resident_agent_role
}

fn registry_lookup_for_route_child<T>(
    result: Result<Option<T>, String>,
    rollout_direct_resident_child: bool,
) -> Result<Option<T>, String> {
    match result {
        Ok(value) => Ok(value),
        Err(error)
            if rollout_direct_resident_child && registry_unavailable_for_route_child(&error) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn registry_unavailable_for_route_child(error: &str) -> bool {
    error.contains("failed to open Turso agent session registry")
        || error.contains("database is locked")
        || error.contains("locking error")
        || error.contains("failed locking file")
}

fn session_start_reuse_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    session: &AgentSessionRecord,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields = agent_session_route_fields("reuse-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    fields.insert(
        "rootSessionId".to_string(),
        serde_json::Value::String(session.root_session_id.clone()),
    );
    fields.insert(
        "childSessionId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "agentSessionResumeId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "childSessionName".to_string(),
        serde_json::Value::String(session.name.clone()),
    );
    let message = render_agent_session_template(
        asp_session_policy.messages.session_start_reuse.as_deref(),
        &[
            template_value("residentChildName", resident_child_name),
            template_value("childSessionId", &session.session_id),
            template_value("rootSessionId", &session.root_session_id),
        ],
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
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

fn session_start_resume_existing_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    session: &AgentSessionRecord,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields =
        agent_session_route_fields("resume-existing-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    fields.insert(
        "rootSessionId".to_string(),
        serde_json::Value::String(session.root_session_id.clone()),
    );
    fields.insert(
        "childSessionId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "agentSessionResumeId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "childSessionName".to_string(),
        serde_json::Value::String(session.name.clone()),
    );
    fields.insert(
        "childSessionStatus".to_string(),
        serde_json::Value::String(session.status.clone()),
    );
    fields.insert(
        "nextAction".to_string(),
        serde_json::Value::String("resume-existing-resident-child".to_string()),
    );
    fields.insert(
        "archiveOrDeleteRequiredForReplacement".to_string(),
        serde_json::Value::Bool(true),
    );
    let message = format!(
        "Existing resident {resident_child_name} child session {} is registered with status {}. Resume that child session instead of creating a replacement; archive or delete it only when you intentionally want to destroy the Codex session history.",
        session.session_id, session.status
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

fn session_start_bootstrap_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    root_session_id: Option<String>,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let mut fields = agent_session_route_fields("start-resident-child", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("session-start-reminder".to_string()),
    );
    fields.insert(
        "agentSessionBootstrapGuideCommand".to_string(),
        serde_json::Value::String(format!(
            "asp agent session bootstrap --name {resident_child_name} --json"
        )),
    );
    fields.insert(
        "agentSessionLifecycleAuditCommand".to_string(),
        serde_json::Value::String("asp agent session lifecycle audit --json".to_string()),
    );
    if let Some(root_session_id) = root_session_id.as_ref() {
        fields.insert(
            "rootSessionId".to_string(),
            serde_json::Value::String(root_session_id.clone()),
        );
    }
    let create_action = resident_child_create_action(platform, asp_session_policy);
    let message = render_agent_session_template(
        asp_session_policy
            .messages
            .session_start_bootstrap
            .as_deref(),
        &[
            template_value("residentChildName", resident_child_name),
            template_value(
                "residentCodexAgentName",
                asp_session_policy.resident_codex_agent_name(),
            ),
            template_value("createAction", &create_action),
            template_value("rootSessionId", root_session_id.as_deref().unwrap_or("")),
        ],
    );
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: platform.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
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
