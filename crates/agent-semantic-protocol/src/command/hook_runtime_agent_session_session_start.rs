//! Session-start bootstrap and route-context decisions for resident ASP children.

use crate::command::{
    current_agent_session_id, current_registered_session, current_root_session_id,
    has_current_agent_session, registered_resident_session_for_root,
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
            && self.root_session_id.is_none()
            && self.current_rollout_topology.is_none()
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
    let current_session =
        registry_lookup_for_route_child(current_registered_session(project_root), false)?;
    let root_session_id = current_root_session_id()
        .or_else(|| {
            current_session
                .as_ref()
                .map(|session| session.root_session_id.clone())
        })
        .or_else(current_agent_session_id);
    let now = unix_timestamp()?;
    let active_explore_session = root_session_id
        .as_deref()
        .map(|root_session_id| {
            registered_resident_session_for_root(
                project_root,
                root_session_id,
                asp_session_policy.resident_child_name(),
            )
        })
        .transpose()?
        .flatten()
        .filter(|session| {
            session_matches_resident_agent(
                session,
                asp_session_policy.resident_child_name(),
                asp_session_policy.resident_agent_role(),
            ) && session.is_routable_at(now)
        });
    let active_testing_session = root_session_id
        .as_deref()
        .map(|root_session_id| {
            registered_resident_session_for_root(
                project_root,
                root_session_id,
                asp_session_policy.testing_resident_child_name(),
            )
        })
        .transpose()?
        .flatten()
        .filter(|session| {
            session_matches_resident_agent(
                session,
                asp_session_policy.testing_resident_child_name(),
                asp_session_policy.testing_resident_agent_role(),
            ) && session.is_routable_at(now)
        });
    Ok(MainSessionRouteContext {
        has_agent_session: has_current_agent_session(),
        current_session,
        active_explore_session,
        active_testing_session,
        root_session_id,
        current_rollout_topology: None,
    })
}

fn payload_matches_resident_managed_agent(
    payload: &serde_json::Value,
    asp_session_policy: &AspSessionPolicy,
) -> bool {
    string_field(payload, &["agent_role", "agentRole"])
        .as_deref()
        .is_some_and(|role| role == asp_session_policy.resident_agent_role())
        || string_field(payload, &["agent_path", "agentPath"])
            .as_deref()
            .and_then(|path| path.rsplit('/').next())
            .is_some_and(|name| name == asp_session_policy.resident_agent_role())
}

pub(super) fn classify_session_start_bootstrap(
    project_root: &Path,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    asp_session_policy: &AspSessionPolicy,
) -> Result<Option<HookDecision>, String> {
    let codex_native_event = if platform == "codex" && event == "subagent-start" {
        crate::codex::native_agent_transport::parse_subagent_event(payload)?
    } else {
        None
    };
    let codex_native_managed_subagent_start = codex_native_event.as_ref().is_some_and(|native| {
        native.kind == crate::codex::native_agent_transport::CodexNativeSubagentEventKind::Start
            && native.agent_type == asp_session_policy.resident_agent_role()
    });
    if codex_native_event.is_some() && !codex_native_managed_subagent_start {
        return Ok(None);
    }
    let native_managed_subagent_start = event == "subagent-start"
        && (codex_native_managed_subagent_start
            || (platform != "codex"
                && payload_matches_resident_managed_agent(payload, asp_session_policy)));
    if !has_current_agent_session() && !native_managed_subagent_start {
        return Ok(None);
    }
    let now = unix_timestamp()?;
    let current_rollout_topology = if matches!(event, "session-start" | "subagent-start") {
        current_rollout_topology()?
    } else {
        None
    };
    if event == "subagent-start" {
        let direct_resident_topology = current_rollout_topology
            .as_ref()
            .filter(|topology| topology.is_direct_resident_subagent(asp_session_policy));
        let payload_matches_resident = codex_native_managed_subagent_start
            || (platform != "codex"
                && payload_matches_resident_managed_agent(payload, asp_session_policy));
        if direct_resident_topology.is_some() || payload_matches_resident {
            if let Some(native) = codex_native_event.as_ref() {
                let expected_model =
                    crate::command::agent_session_registry::expected_model_for_session_profile(
                        asp_session_policy.resident_child_name(),
                        asp_session_policy.resident_agent_role(),
                    )?;
                if expected_model.as_deref() != Some(native.model.as_str()) {
                    let mut decision = session_start_bootstrap_decision(
                        platform,
                        event,
                        payload,
                        Some(native.root_session_id.clone()),
                        asp_session_policy,
                    );
                    decision.message = format!(
                        "Rejecting configured ASP resident child {} because the host observed model {} but the managed profile requires {}. Stop this new native child and re-enter the bootstrap pane; do not register or route it.",
                        native.agent_id,
                        native.model,
                        expected_model
                            .as_deref()
                            .unwrap_or("<missing configured model>")
                    );
                    decision.fields.insert(
                        "agentSessionRejectedChildId".to_string(),
                        serde_json::Value::String(native.agent_id.clone()),
                    );
                    decision.fields.insert(
                        "agentSessionRejectedChildAction".to_string(),
                        serde_json::Value::String("stop-native-subagent".to_string()),
                    );
                    decision.fields.insert(
                        "observedModel".to_string(),
                        serde_json::Value::String(native.model.clone()),
                    );
                    if let Some(expected_model) = expected_model {
                        decision.fields.insert(
                            "expectedModel".to_string(),
                            serde_json::Value::String(expected_model),
                        );
                    }
                    return Ok(Some(decision));
                }
            }
            let child_session_id = codex_native_event
                .as_ref()
                .map(|native| native.agent_id.clone())
                .or_else(|| direct_resident_topology.map(|topology| topology.session_id.clone()))
                .or_else(|| {
                    string_field(
                        payload,
                        &[
                            "child_session_id",
                            "childSessionId",
                            "session_id",
                            "sessionId",
                            "agent_id",
                            "agentId",
                            "agent_thread_id",
                            "agentThreadId",
                        ],
                    )
                });
            let root_session_id = codex_native_event
                .as_ref()
                .map(|native| native.root_session_id.clone())
                .or_else(|| {
                    direct_resident_topology
                        .and_then(|topology| topology.root_session_id().map(str::to_string))
                })
                .or_else(|| {
                    string_field(
                        payload,
                        &[
                            "root_session_id",
                            "rootSessionId",
                            "parent_thread_id",
                            "parentThreadId",
                            "source_session_id",
                            "sourceSessionId",
                            "parent_session_id",
                            "parentSessionId",
                        ],
                    )
                })
                .or_else(current_root_session_id);
            if let (Some(child_session_id), Some(root_session_id)) =
                (child_session_id, root_session_id)
            {
                let registry =
                    agent_semantic_client_db::AgentSessionRegistry::open_or_create_project(
                        project_root,
                    )?;
                let project_id =
                    agent_semantic_client_db::AgentSessionRegistry::project_scope_id(project_root);
                let reconciliation =
                    crate::codex::resident_session_reconcile::reconcile_resident_session(
                        &registry,
                        &project_id,
                        &root_session_id,
                        asp_session_policy.resident_child_name(),
                        asp_session_policy.resident_agent_role(),
                    )?;
                if let Some(existing) = reconciliation.current.as_ref()
                    && existing.session.session_id != child_session_id
                {
                    let mut decision = session_start_decision_for_reconciled_resident(
                        now,
                        platform,
                        event,
                        payload,
                        existing,
                        asp_session_policy,
                    );
                    append_resident_reconciliation_fields(&mut decision, &reconciliation);
                    decision.fields.insert(
                        "agentSessionDuplicateChildId".to_string(),
                        serde_json::Value::String(child_session_id),
                    );
                    decision.fields.insert(
                        "agentSessionDuplicateChildAction".to_string(),
                        serde_json::Value::String("close-native-subagent".to_string()),
                    );
                    return Ok(Some(decision));
                }
                let metadata_json = serde_json::json!({
                    "event": "subagent-start",
                    "native": true,
                    "rootSessionId": root_session_id,
                    "childSessionId": child_session_id,
                    "agentRole": asp_session_policy.resident_agent_role(),
                    "agentType": codex_native_event.as_ref().map(|native| native.agent_type.as_str()),
                    "permissionMode": codex_native_event.as_ref().map(|native| native.permission_mode.as_str()),
                })
                .to_string();
                let message_target_id = codex_native_event
                    .as_ref()
                    .map(|native| native.message_target_id().to_string());
                let claimed = registry.claim_resident_session(
                    agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                        project_id: &project_id,
                        root_session_id: &root_session_id,
                        session_id: &child_session_id,
                        message_target_id: message_target_id.as_deref(),
                        parent_session_id: Some(&root_session_id),
                        name: asp_session_policy.resident_child_name(),
                        role: asp_session_policy.resident_agent_role(),
                        model_observation: string_field(payload, &["model", "modelId"])
                            .as_deref()
                            .map(|model| {
                                agent_semantic_client_db::AgentSessionModelObservationRef {
                                    model,
                                source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                                    observed_at: now,
                                    evidence_ref: None,
                                }
                            }),
                        status: if message_target_id.is_some() {
                            "active"
                        } else {
                            "pending-target"
                        },
                        expires_at: None,
                        metadata_json: &metadata_json,
                        now,
                    },
                )?;
                if claimed.session_id == child_session_id {
                    if message_target_id.is_some() && !claimed.is_routable_at(now) {
                        registry.register_session(
                            agent_semantic_client_db::agent_session_registry::AgentSessionRegisterRequest {
                                project_id: &project_id,
                                root_session_id: &root_session_id,
                                session_id: &child_session_id,
                                message_target_id: message_target_id.as_deref(),
                                parent_session_id: Some(&root_session_id),
                                name: asp_session_policy.resident_child_name(),
                                role: asp_session_policy.resident_agent_role(),
                                model_observation: string_field(payload, &["model", "modelId"])
                                    .as_deref()
                                    .map(|model| agent_semantic_client_db::AgentSessionModelObservationRef {
                                        model,
                                            source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                                        observed_at: now,
                                        evidence_ref: None,
                                    }),
                                status: "active",
                                expires_at: None,
                                metadata_json: &metadata_json,
                                now,
                            },
                        )?;
                    }
                    return Ok(None);
                }
                let mut decision = if claimed.is_routable_at(now) {
                    session_start_reuse_decision(
                        platform,
                        event,
                        payload,
                        &claimed,
                        asp_session_policy,
                    )
                } else {
                    session_start_resume_existing_decision(
                        platform,
                        event,
                        payload,
                        &claimed,
                        asp_session_policy,
                    )
                };
                decision.fields.insert(
                    "agentSessionDuplicateChildId".to_string(),
                    serde_json::Value::String(child_session_id),
                );
                decision.fields.insert(
                    "agentSessionDuplicateChildAction".to_string(),
                    serde_json::Value::String("close-native-subagent".to_string()),
                );
                return Ok(Some(decision));
            }
        }
    }
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
    if let Some(root_session_id) = current_root_session_id() {
        let registry =
            agent_semantic_client_db::AgentSessionRegistry::open_or_create_project(project_root)?;
        let project_id =
            agent_semantic_client_db::AgentSessionRegistry::project_scope_id(project_root);
        let reconciliation = crate::codex::resident_session_reconcile::reconcile_resident_session(
            &registry,
            &project_id,
            &root_session_id,
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        )?;
        if let Some(existing) = reconciliation.current.as_ref() {
            let mut decision = session_start_decision_for_reconciled_resident(
                now,
                platform,
                event,
                payload,
                existing,
                asp_session_policy,
            );
            append_resident_reconciliation_fields(&mut decision, &reconciliation);
            return Ok(Some(decision));
        }
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
    Ok(Some(session_start_bootstrap_decision(
        platform,
        event,
        payload,
        current_root_session_id(),
        asp_session_policy,
    )))
}

pub(in super::super) fn session_matches_resident_agent(
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

fn session_start_decision_for_reconciled_resident(
    now: i64,
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    existing: &crate::codex::resident_session_reconcile::CodexResidentSessionCandidate,
    asp_session_policy: &AspSessionPolicy,
) -> HookDecision {
    if !existing.session.is_routable_at(now) {
        return session_start_resume_existing_decision(
            platform,
            event,
            payload,
            &existing.session,
            asp_session_policy,
        );
    }
    match &existing.liveness {
        crate::codex::rollout::CodexRolloutSessionLiveness::Resumable(_) => {
            session_start_resume_existing_decision(
                platform,
                event,
                payload,
                &existing.session,
                asp_session_policy,
            )
        }
        crate::codex::rollout::CodexRolloutSessionLiveness::Active(_)
        | crate::codex::rollout::CodexRolloutSessionLiveness::Unknown(_) => {
            session_start_reuse_decision(
                platform,
                event,
                payload,
                &existing.session,
                asp_session_policy,
            )
        }
        crate::codex::rollout::CodexRolloutSessionLiveness::Missing
        | crate::codex::rollout::CodexRolloutSessionLiveness::Unavailable(_) => {
            session_start_resume_existing_decision(
                platform,
                event,
                payload,
                &existing.session,
                asp_session_policy,
            )
        }
    }
}

fn append_resident_reconciliation_fields(
    decision: &mut HookDecision,
    reconciliation: &crate::codex::resident_session_reconcile::CodexResidentSessionReconciliation,
) {
    let Some(current) = reconciliation.current.as_ref() else {
        return;
    };
    let (state, last_event_kind, scanned_bytes, error) = match &current.liveness {
        crate::codex::rollout::CodexRolloutSessionLiveness::Resumable(activity) => (
            "rollout-resumable",
            activity.last_event_kind.as_deref(),
            Some(activity.scanned_bytes),
            None,
        ),
        crate::codex::rollout::CodexRolloutSessionLiveness::Active(activity) => (
            "rollout-active",
            activity.last_event_kind.as_deref(),
            Some(activity.scanned_bytes),
            None,
        ),
        crate::codex::rollout::CodexRolloutSessionLiveness::Unknown(activity) => (
            "rollout-unknown",
            activity.last_event_kind.as_deref(),
            Some(activity.scanned_bytes),
            None,
        ),
        crate::codex::rollout::CodexRolloutSessionLiveness::Missing => {
            ("rollout-missing", None, None, None)
        }
        crate::codex::rollout::CodexRolloutSessionLiveness::Unavailable(error) => {
            ("rollout-unavailable", None, None, Some(error.as_str()))
        }
    };
    decision.fields.insert(
        "agentSessionReconciliation".to_string(),
        serde_json::Value::String(state.to_string()),
    );
    decision.fields.insert(
        "agentSessionRolloutLookup".to_string(),
        serde_json::Value::String("session-id-fast-path".to_string()),
    );
    decision.fields.insert(
        "agentSessionHistoricalResidentCount".to_string(),
        serde_json::json!(reconciliation.historical_resident_count),
    );
    if let Some(last_event_kind) = last_event_kind {
        decision.fields.insert(
            "agentSessionRolloutLastEventKind".to_string(),
            serde_json::Value::String(last_event_kind.to_string()),
        );
    }
    if let Some(scanned_bytes) = scanned_bytes {
        decision.fields.insert(
            "agentSessionRolloutScannedBytes".to_string(),
            serde_json::json!(scanned_bytes),
        );
    }
    if let Some(error) = error {
        decision.fields.insert(
            "agentSessionRolloutError".to_string(),
            serde_json::Value::String(error.to_string()),
        );
    }
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
        "agentSessionExistingChildId".to_string(),
        serde_json::Value::String(session.session_id.clone()),
    );
    fields.insert(
        "childSessionName".to_string(),
        serde_json::Value::String(session.name.clone()),
    );
    fields.insert(
        "nextAction".to_string(),
        serde_json::Value::String("enter-bootstrap-pane-for-existing-child".to_string()),
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
        serde_json::Value::String("enter-bootstrap-pane-for-existing-child".to_string()),
    );
    let message = format!(
        "Existing resident {resident_child_name} child session {} is registered with status {}. Enter the resident-child choice pane and let it recover or replace that child; do not create a generic replacement outside the pane.",
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
    let mut fields =
        agent_session_route_fields("enter-resident-child-bootstrap-pane", resident_child_name);
    append_resident_agent_fields(&mut fields, asp_session_policy);
    fields.insert(
        "agentSessionBootstrap".to_string(),
        serde_json::Value::String("session-start-reminder".to_string()),
    );
    fields.insert(
        "agentSessionBootstrapGuideCommand".to_string(),
        serde_json::Value::String(format!(
            "asp agent session bootstrap --name {resident_child_name}"
        )),
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
