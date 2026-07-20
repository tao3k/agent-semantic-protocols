//! Session-start bootstrap and route-context decisions for resident ASP children.

#[path = "hook_runtime_agent_session_session_start_route.rs"]
mod hook_runtime_agent_session_session_start_route;

use crate::command::{
    ResidentChildIdentityProof, codex_transcript_resident_child_identity, current_agent_session_id,
    current_registered_session, current_registered_session_identity,
    current_resident_child_identity_proof, current_root_session_id, has_current_agent_session,
    registered_resident_session_for_root,
};
use agent_semantic_client_db::AgentSessionRecord;
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
use agent_semantic_runtime::codex_rollout_session_metadata;
use std::path::Path;

use super::hook_runtime_agent_session_rollout_topology::{
    CurrentRolloutTopology, current_rollout_topology, nested_resident_child_decision,
    register_required_resident_child_decision,
};
use super::{
    AspSessionPolicy, agent_session_route_fields, append_resident_agent_fields, string_field,
    unix_timestamp,
};
use hook_runtime_agent_session_session_start_route::{
    registry_lookup_for_route_child, session_start_bootstrap_decision,
};
pub(super) use hook_runtime_agent_session_session_start_route::{
    session_start_resume_existing_decision, session_start_reuse_decision,
};

pub(super) struct MainSessionRouteContext {
    pub(super) has_agent_session: bool,
    pub(super) current_session: Option<AgentSessionRecord>,
    pub(super) active_explore_session: Option<AgentSessionRecord>,
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

    pub(super) fn outside_agent_session(&self) -> bool {
        !self.has_agent_session
            && self.current_session.is_none()
            && self.active_explore_session.is_none()
            && self.root_session_id.is_none()
            && self.current_rollout_topology.is_none()
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
    payload: &serde_json::Value,
) -> Result<MainSessionRouteContext, String> {
    let current_session =
        registry_lookup_for_route_child(current_registered_session_identity(project_root), false)?;
    let root_session_id = current_root_session_id()
        .or_else(|| {
            current_session
                .as_ref()
                .map(|session| session.root_session_id.clone())
        })
        .or_else(|| {
            string_field(
                payload,
                &[
                    "root_session_id",
                    "rootSessionId",
                    "session_id",
                    "sessionId",
                ],
            )
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
    let current_rollout_topology = current_rollout_topology()?;
    Ok(MainSessionRouteContext {
        has_agent_session: has_current_agent_session(),
        current_session,
        active_explore_session,
        root_session_id,
        current_rollout_topology,
    })
}

pub(in crate::command) fn current_session_resident_child_identity_proof(
    project_root: &Path,
    asp_session_policy: &AspSessionPolicy,
    payload: &serde_json::Value,
) -> Result<Option<ResidentChildIdentityProof>, String> {
    current_session_configured_resident_identity_proof(
        project_root,
        payload,
        asp_session_policy.resident_child_name(),
        asp_session_policy.resident_agent_role(),
        asp_session_policy.resident_codex_agent_name(),
    )
}

pub(super) fn current_session_configured_resident_identity_proof(
    project_root: &Path,
    payload: &serde_json::Value,
    resident_child_name: &str,
    resident_agent_role: &str,
    resident_codex_agent_name: &str,
) -> Result<Option<ResidentChildIdentityProof>, String> {
    // Codex keeps CODEX_THREAD_ID pinned to the root task for collaboration
    // children.  The host-owned pre-tool envelope identifies the executing
    // child separately through top-level agent_id/agent_type fields, so
    // requiring agent_id == CODEX_THREAD_ID misclassifies resumed residents as
    // the main Agent.  Never recurse into tool_input here: only host envelope
    // fields may prove identity.
    let payload_agent_id = super::hook_runtime_agent_session_identity::top_level_string_field(
        payload,
        &["agent_id", "agentId"],
    );
    let payload_agent_type = super::hook_runtime_agent_session_identity::top_level_string_field(
        payload,
        &["agent_type", "agentType"],
    );
    if let Some(payload_agent_id) = payload_agent_id
        && payload_agent_type.as_deref() == Some(resident_codex_agent_name)
    {
        if let Some(session) =
            super::hook_runtime_agent_session_identity::registered_resident_session_by_id(
                project_root,
                &payload_agent_id,
            )?
            && session_matches_resident_agent(&session, resident_child_name, resident_agent_role)
        {
            super::hook_runtime_agent_session_presence::record_trusted_resident_hook_presence(
                project_root,
                &session,
                resident_child_name,
                resident_codex_agent_name,
            )?;
            return Ok(Some(ResidentChildIdentityProof::CodexHookPayload));
        }
        if let Some(root_session_id) =
            super::hook_runtime_agent_session_identity::top_level_string_field(
                payload,
                &["session_id", "sessionId"],
            )
            && let Some(session) =
                super::hook_runtime_agent_session_presence::rehydrate_trusted_resident_hook_session(
                    project_root,
                    &root_session_id,
                    &payload_agent_id,
                    resident_child_name,
                    resident_agent_role,
                    resident_codex_agent_name,
                )?
        {
            super::hook_runtime_agent_session_presence::record_trusted_resident_hook_presence(
                project_root,
                &session,
                resident_child_name,
                resident_codex_agent_name,
            )?;
            return Ok(Some(ResidentChildIdentityProof::CodexHookPayload));
        }
        // The host-owned top-level agent_id/agent_type pair proves which
        // executor is running, so that child may terminate its own configured
        // execution lane without routing back to itself. Registry mutation is
        // deliberately stricter: only the exact registered identity or the
        // rollout/profile compare-and-swap paths above may refresh Ready.
        return Ok(Some(ResidentChildIdentityProof::CodexHookPayload));
    }
    if let Some(session_id) = super::hook_runtime_agent_session_identity::top_level_string_field(
        payload,
        &["session_id", "sessionId"],
    ) && let Some(session) =
        super::hook_runtime_agent_session_identity::registered_resident_session_by_id(
            project_root,
            &session_id,
        )?
        && session_matches_resident_agent(&session, resident_child_name, resident_agent_role)
    {
        super::hook_runtime_agent_session_presence::record_trusted_resident_hook_presence(
            project_root,
            &session,
            resident_child_name,
            resident_codex_agent_name,
        )?;
        return Ok(Some(ResidentChildIdentityProof::RegistryExact));
    }
    if let Some((proof, transcript_session_id)) = codex_transcript_resident_child_identity(
        project_root,
        payload,
        resident_child_name,
        resident_agent_role,
    )? {
        if let Some(session) =
            super::hook_runtime_agent_session_identity::registered_resident_session_by_id(
                project_root,
                &transcript_session_id,
            )?
            && session_matches_resident_agent(&session, resident_child_name, resident_agent_role)
        {
            super::hook_runtime_agent_session_presence::record_trusted_resident_hook_presence(
                project_root,
                &session,
                resident_child_name,
                resident_codex_agent_name,
            )?;
        }
        return Ok(Some(proof));
    }
    let proof = current_resident_child_identity_proof(
        project_root,
        resident_child_name,
        resident_agent_role,
    )?;
    Ok(proof)
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
    use crate::command::agent_session_registry::record_subagent_start_target_present as record_start;

    let resident_name = asp_session_policy.resident_child_name();
    let codex_native_event = if platform == "codex" && event == "subagent-start" {
        crate::codex::native_agent_transport::parse_subagent_event(payload)?
    } else {
        None
    };
    let codex_native_rollout_metadata = if let Some(native) = codex_native_event.as_ref() {
        match codex_rollout_session_metadata(&native.agent_id) {
            Ok(metadata) => metadata,
            Err(error)
                if error.starts_with(
                    "Codex rollout invariant broken: no rollout JSONL found for session",
                ) =>
            {
                None
            }
            Err(error) => return Err(error),
        }
    } else {
        None
    };
    let expected_model_for_native_start = if codex_native_event.is_some() {
        crate::command::agent_session_registry::expected_model_for_session_profile(
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        )?
    } else {
        None
    };
    let expected_reasoning_for_native_start = if codex_native_event.is_some() {
        crate::command::agent_session_registry::expected_reasoning_effort_for_session_profile(
            asp_session_policy.resident_child_name(),
            asp_session_policy.resident_agent_role(),
        )?
    } else {
        None
    };
    let observed_reasoning_for_native_start = codex_native_event
        .as_ref()
        .and_then(|native| native.reasoning_effort.clone())
        .or_else(|| {
            codex_native_rollout_metadata
                .as_ref()
                .and_then(|metadata| metadata.reasoning_effort.clone())
        });
    let native_root_session_id = codex_native_event.as_ref().map(|native| {
        codex_native_rollout_metadata
            .as_ref()
            .and_then(|metadata| {
                metadata
                    .root_session_id
                    .clone()
                    .or_else(|| metadata.parent_thread_id.clone())
            })
            .unwrap_or_else(|| native.root_session_id.clone())
    });
    let codex_native_managed_subagent_start = codex_native_event.as_ref().is_some_and(|native| {
        native.kind == crate::codex::native_agent_transport::CodexNativeSubagentEventKind::Start
            && native.agent_type == asp_session_policy.resident_agent_role()
    });
    if codex_native_managed_subagent_start && let Some(native) = codex_native_event.as_ref() {
        let root_session_id = native_root_session_id
            .as_deref()
            .unwrap_or(&native.root_session_id);
        super::hook_runtime_agent_session_typed_replacement::release_terminal_owner_before_typed_start(
            project_root,
            native,
            root_session_id,
            asp_session_policy,
        )?;
    }
    if let Some(native) = codex_native_event.as_ref()
        && !codex_native_managed_subagent_start
    {
        return Ok(Some(unmanaged_codex_subagent_start_decision(
            platform,
            event,
            payload,
            native,
            asp_session_policy,
            expected_model_for_native_start.as_deref(),
            expected_reasoning_for_native_start.as_deref(),
            observed_reasoning_for_native_start.as_deref(),
            false,
        )));
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
    let mut native_runtime_drift_decision = None;
    if event == "subagent-start" {
        let direct_resident_topology = current_rollout_topology
            .as_ref()
            .filter(|topology| topology.is_direct_resident_subagent(asp_session_policy));
        let payload_matches_resident = codex_native_managed_subagent_start
            || (platform != "codex"
                && payload_matches_resident_managed_agent(payload, asp_session_policy));
        if direct_resident_topology.is_some() || payload_matches_resident {
            if let Some(native) = codex_native_event.as_ref() {
                let expected_model = expected_model_for_native_start.clone();
                let expected_reasoning_effort = expected_reasoning_for_native_start.clone();
                let model_mismatch = expected_model
                    .as_deref()
                    .is_some_and(|expected_model| expected_model != native.model);
                let reasoning_mismatch = super::hook_runtime_agent_session_profile::reasoning_observation_mismatches_profile(
                    observed_reasoning_for_native_start.as_deref(),
                    expected_reasoning_effort.as_deref(),
                );
                if model_mismatch || reasoning_mismatch {
                    let observed_reasoning_effort = observed_reasoning_for_native_start
                        .as_deref()
                        .unwrap_or("<missing>");
                    let mut decision = session_start_bootstrap_decision(
                        platform,
                        event,
                        payload,
                        native_root_session_id.clone(),
                        asp_session_policy,
                    );
                    decision.decision = DecisionKind::Allow;
                    decision.reason_kind = ReasonKind::None;
                    decision.message = format!(
                        "Codex started or resumed ASP child {} with runtime model {} and reasoning {}, but its registered role requires model {} and reasoning {}. Codex exposes no same-child runtime override on resume/follow-up. Allow lifecycle completion, mark this child replacement-required, retire/archive it through the host, then create exactly one agent_type={} replacement from the registered TOML. This drift must not block unrelated Codex tools.",
                        native.agent_id,
                        native.model,
                        observed_reasoning_effort,
                        expected_model
                            .as_deref()
                            .unwrap_or("<missing configured model>"),
                        expected_reasoning_effort
                            .as_deref()
                            .unwrap_or("<not configured>"),
                        asp_session_policy.resident_agent_role(),
                    );
                    decision.fields.insert(
                        "agentSessionAction".to_string(),
                        serde_json::Value::String("replace-drifted-native-subagent".to_string()),
                    );
                    if let Some(root_session_id) = native_root_session_id.as_ref() {
                        decision.fields.insert(
                            "rootSessionId".to_string(),
                            serde_json::Value::String(root_session_id.clone()),
                        );
                    }
                    decision.fields.insert(
                        "childSessionId".to_string(),
                        serde_json::Value::String(native.agent_id.clone()),
                    );
                    decision.fields.insert(
                        "agentSessionObservedChildId".to_string(),
                        serde_json::Value::String(native.agent_id.clone()),
                    );
                    decision.fields.insert(
                        "agentSessionObservedAgentType".to_string(),
                        serde_json::Value::String(native.agent_type.clone()),
                    );
                    decision.fields.insert(
                        "agentSessionExpectedAgentType".to_string(),
                        serde_json::Value::String(
                            asp_session_policy.resident_agent_role().to_string(),
                        ),
                    );
                    decision.fields.insert(
                        "agentSessionObservedModel".to_string(),
                        serde_json::Value::String(native.model.clone()),
                    );
                    if let Some(observed_reasoning_effort) =
                        observed_reasoning_for_native_start.as_ref()
                    {
                        decision.fields.insert(
                            "agentSessionObservedReasoningEffort".to_string(),
                            serde_json::Value::String(observed_reasoning_effort.clone()),
                        );
                    }
                    decision.fields.insert(
                        "nextAction".to_string(),
                        serde_json::Value::String(
                            "retire-drifted-child-and-create-configured-replacement".to_string(),
                        ),
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
                    if let Some(expected_reasoning_effort) = expected_reasoning_effort {
                        decision.fields.insert(
                            "expectedReasoningEffort".to_string(),
                            serde_json::Value::String(expected_reasoning_effort),
                        );
                    }
                    decision.fields.insert(
                        "runtimeDriftDetected".to_string(),
                        serde_json::Value::Bool(true),
                    );
                    native_runtime_drift_decision = Some(decision);
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
            let root_session_id = native_root_session_id
                .clone()
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
                    && !matches!(existing.session.status.as_str(), "archived" | "closed")
                {
                    let mut decision = super::hook_runtime_agent_session_typed_replacement::session_start_decision_for_reconciled_resident(
                        now,
                        platform,
                        event,
                        payload,
                        existing,
                        asp_session_policy,
                    );
                    super::hook_runtime_agent_session_typed_replacement::append_resident_reconciliation_fields(&mut decision, &reconciliation);
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
                let message_target_id = codex_native_event
                    .as_ref()
                    .map(|native| native.message_target_id().to_string())
                    .or_else(|| (platform == "codex").then(|| child_session_id.clone()));
                let metadata_json = serde_json::json!({
                    "event": "subagent-start",
                    "native": true,
                    "rootSessionId": root_session_id,
                    "childSessionId": child_session_id,
                    "agentRole": asp_session_policy.resident_agent_role(),
                    "agentType": codex_native_event.as_ref().map(|native| native.agent_type.as_str()),
                    "model": codex_native_event.as_ref().map(|native| native.model.as_str()),
                    "reasoningVerification": super::hook_runtime_agent_session_profile::typed_spawn_reasoning_verification(
                        observed_reasoning_for_native_start.as_deref(),
                        expected_reasoning_for_native_start.as_deref(),
                    ),
                    "permissionMode": codex_native_event.as_ref().map(|native| native.permission_mode.as_str()),
                    "messageTargetBinding": message_target_id.as_ref().map(|target| serde_json::json!({
                        "source": "codex.subagent-start",
                        "boundRootSessionId": root_session_id,
                        "childSessionId": child_session_id,
                        "messageTargetId": target,
                        "observedAt": now,
                    })),
                })
                .to_string();
                let terminal_existing_owner = registered_resident_session_for_root(
                    project_root,
                    &root_session_id,
                    asp_session_policy.resident_child_name(),
                )?
                .filter(|existing| {
                    existing.session_id != child_session_id
                        && matches!(
                            existing.status.as_str(),
                            "archived" | "closed" | "invalid" | "replacement-required"
                        )
                })
                .map(|existing| existing.session_id);
                if let Some(terminal_existing_owner) = terminal_existing_owner {
                    registry.delete_session(&project_id, &terminal_existing_owner)?;
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
                                .map(|model| {
                                    agent_semantic_client_db::AgentSessionModelObservationRef {
                                        model,
                                        source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexSubagentStart,
                                        observed_at: now,
                                        evidence_ref: None,
                                    }
                                }),
                            status: if native_runtime_drift_decision.is_some() {
                                "replacement-required"
                            } else if message_target_id.is_some() {
                                "active"
                            } else {
                                "pending-target"
                            },
                            expires_at: None,
                            metadata_json: &metadata_json,
                            now,
                        },
                    )?;
                    if message_target_id.is_some() {
                        record_start(&registry, &root_session_id, resident_name, now)?;
                    }
                    return Ok(native_runtime_drift_decision);
                }
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
                        status: if native_runtime_drift_decision.is_some() {
                            "replacement-required"
                        } else if message_target_id.is_some() {
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
                    if message_target_id.is_some() {
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
                                status: if native_runtime_drift_decision.is_some() {
                                    "replacement-required"
                                } else {
                                    "active"
                                },
                                expires_at: None,
                                metadata_json: &metadata_json,
                                now,
                            },
                        )?;
                    }
                    if message_target_id.is_some() {
                        record_start(&registry, &root_session_id, resident_name, now)?;
                    }
                    return Ok(native_runtime_drift_decision);
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
            let mut decision = super::hook_runtime_agent_session_typed_replacement::session_start_decision_for_reconciled_resident(
                now,
                platform,
                event,
                payload,
                existing,
                asp_session_policy,
            );
            super::hook_runtime_agent_session_typed_replacement::append_resident_reconciliation_fields(&mut decision, &reconciliation);
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

fn unmanaged_codex_subagent_start_decision(
    platform: &str,
    event: &str,
    payload: &serde_json::Value,
    native: &crate::codex::native_agent_transport::CodexNativeSubagentEvent,
    asp_session_policy: &AspSessionPolicy,
    expected_model: Option<&str>,
    expected_reasoning_effort: Option<&str>,
    observed_reasoning_effort: Option<&str>,
    repair_candidate: bool,
) -> HookDecision {
    let resident_child_name = asp_session_policy.resident_child_name();
    let expected_agent_type = asp_session_policy.resident_agent_role();
    let action = if repair_candidate {
        "replace-drifted-native-subagent"
    } else {
        "ignore-unmanaged-native-subagent"
    };
    let mut fields = agent_session_route_fields(action, resident_child_name);
    append_resident_agent_fields(&mut fields, platform, asp_session_policy);
    fields.insert(
        "rootSessionId".to_string(),
        serde_json::Value::String(native.root_session_id.clone()),
    );
    fields.insert(
        "agentSessionObservedChildId".to_string(),
        serde_json::Value::String(native.agent_id.clone()),
    );
    fields.insert(
        "agentSessionObservedAgentType".to_string(),
        serde_json::Value::String(native.agent_type.clone()),
    );
    fields.insert(
        "agentSessionExpectedAgentType".to_string(),
        serde_json::Value::String(expected_agent_type.to_string()),
    );
    fields.insert(
        "agentSessionObservedModel".to_string(),
        serde_json::Value::String(native.model.clone()),
    );
    if let Some(expected_model) = expected_model {
        fields.insert(
            "agentSessionExpectedModel".to_string(),
            serde_json::Value::String(expected_model.to_string()),
        );
    }
    if let Some(observed_reasoning_effort) = observed_reasoning_effort {
        fields.insert(
            "agentSessionObservedReasoningEffort".to_string(),
            serde_json::Value::String(observed_reasoning_effort.to_string()),
        );
    }
    if let Some(expected_reasoning_effort) = expected_reasoning_effort {
        fields.insert(
            "agentSessionExpectedReasoningEffort".to_string(),
            serde_json::Value::String(expected_reasoning_effort.to_string()),
        );
    }
    if repair_candidate {
        fields.insert(
            "runtimeDriftDetected".to_string(),
            serde_json::Value::Bool(true),
        );
        fields.insert(
            "profileDriftDetected".to_string(),
            serde_json::Value::Bool(native.agent_type != expected_agent_type),
        );
        fields.insert(
            "nextAction".to_string(),
            serde_json::Value::String(
                "retire-drifted-child-and-create-configured-replacement".to_string(),
            ),
        );
    }
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
        message: if repair_candidate {
            format!(
                "Codex delivered SubagentStart for child {} with agent_type `{}` and runtime model `{}` / reasoning `{}`, while the registered `{expected_agent_type}` role requires model `{}` / reasoning `{}`. This is a typed-child replacement case, not a same-child model-switch case. Allow native lifecycle completion, retire/archive this drifted child, then create exactly one replacement with agent_type={expected_agent_type} and fork_turns=none so Codex loads the registered TOML. If the host cannot retire or expose typed creation, keep ASP routing degraded without blocking unrelated Codex tools.",
                native.agent_id,
                native.agent_type,
                native.model,
                observed_reasoning_effort.unwrap_or("<missing>"),
                expected_model.unwrap_or("<missing configured model>"),
                expected_reasoning_effort.unwrap_or("<not configured>"),
            )
        } else {
            format!(
                "Codex delivered SubagentStart for unrelated native agent type `{}` with model `{}` while an ASP resident child is already active. This child remains outside ASP lifecycle management.",
                native.agent_type, native.model
            )
        },
        fields,
    }
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
