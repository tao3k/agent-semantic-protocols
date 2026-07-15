use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_rollout_adopt::{
    RolloutAdoptRequest, adopt_reusable_rollout_session,
};
use super::{
    normalize_session_permissions, normalize_session_roles, session_permissions_for_roles,
    session_role_defaults_for_session_name,
};
use agent_semantic_client_db::agent_session_registry::{
    SameChildRuntimeOverrideState, classify_same_child_runtime_override_state,
};
use agent_semantic_client_db::{
    AgentSessionLookupRequest, AgentSessionModelObservationRef, AgentSessionModelObservationSource,
    AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionRegistry,
};

pub(super) struct ResolvedSessionProfile {
    pub(super) roles: Vec<String>,
    pub(super) role: String,
    pub(super) permissions: Vec<String>,
}

pub(super) struct RolloutHistoryPreflight {
    pub(super) status: &'static str,
    pub(super) action: &'static str,
    pub(super) record: Option<AgentSessionRecord>,
}

pub(super) fn resolve_session_profile_from_args(
    args: &SessionArgs,
    name: &str,
) -> Result<ResolvedSessionProfile, String> {
    let role_defaults = session_role_defaults_for_session_name(name)?;
    let has_explicit_roles = args.role.is_some();
    let requested_roles = args
        .role
        .as_deref()
        .map(|roles| {
            roles
                .split(',')
                .map(str::trim)
                .filter(|role| !role.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| role_defaults.roles.clone());
    let roles = normalize_session_roles(&requested_roles)?;
    let role = roles.join(",");
    let permissions = if has_explicit_roles || role_defaults.permissions.is_empty() {
        session_permissions_for_roles(&roles)
    } else {
        normalize_session_permissions(&role_defaults.permissions)?
    };
    Ok(ResolvedSessionProfile {
        roles,
        role,
        permissions,
    })
}

pub(super) fn adopt_reusable_rollout_session_before_create(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: Option<&str>,
    args: &SessionArgs,
    name: Option<&str>,
    excluded_session_id: Option<&str>,
    now: i64,
) -> Result<RolloutHistoryPreflight, String> {
    let Some(root_session_id) = root_session_id.filter(|session| !session.trim().is_empty()) else {
        return Ok(RolloutHistoryPreflight {
            status: "root-session-required",
            action: "run-agent-session-status-with-root-and-name-before-create",
            record: None,
        });
    };
    let Some(name) = name.filter(|name| !name.trim().is_empty()) else {
        return Ok(RolloutHistoryPreflight {
            status: "name-required",
            action: "run-agent-session-status-with-name-before-create",
            record: None,
        });
    };
    let profile = resolve_session_profile_from_args(args, name)?;
    let record = match adopt_reusable_rollout_session(
        registry,
        RolloutAdoptRequest {
            project_id,
            root_session_id,
            name,
            role: &profile.role,
            roles: &profile.roles,
            permissions: &profile.permissions,
            expires_at: args.expires_at,
            now,
            excluded_session_id,
        },
    )? {
        Some(record) => Some(record),
        None => adopt_unique_root_attributed_rollout_session(
            registry,
            project_id,
            root_session_id,
            name,
            &profile.role,
            args.expires_at,
            excluded_session_id,
            now,
        )?,
    };
    Ok(if let Some(record) = record {
        RolloutHistoryPreflight {
            status: "adopted-reusable-rollout",
            action: "resume-adopted-existing-child-then-validate-message-route",
            record: Some(record),
        }
    } else {
        RolloutHistoryPreflight {
            status: "checked-no-reusable-rollout",
            action: "audit-host-agent-tree-after-rollout-history-miss",
            record: None,
        }
    })
}

fn adopt_unique_root_attributed_rollout_session(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    name: &str,
    role: &str,
    expires_at: Option<i64>,
    excluded_session_id: Option<&str>,
    now: i64,
) -> Result<Option<AgentSessionRecord>, String> {
    let host_records =
        agent_semantic_runtime::codex_app_server_child_session_metadata(root_session_id)?;
    let host_candidates = host_records
        .iter()
        .filter(|metadata| excluded_session_id != Some(metadata.session_id.as_str()))
        .filter(|metadata| {
            super::agent_session_registry_validation::rollout_metadata_matches_managed_agent_profile(
                name, role, metadata,
            )
        })
        .collect::<Vec<_>>();
    match host_candidates.as_slice() {
        [candidate] => {
            return register_recovered_rollout_session(
                registry,
                project_id,
                root_session_id,
                name,
                role,
                expires_at,
                candidate,
                "codex-app-server-native-host-tree",
                now,
            );
        }
        [] => {}
        _ => return Ok(None),
    }
    let Some(index) = agent_semantic_runtime::codex_rollout_session_index(root_session_id)? else {
        return Ok(None);
    };
    let candidates = index
        .records
        .iter()
        .filter(|metadata| excluded_session_id != Some(metadata.session_id.as_str()))
        .filter(|metadata| {
            super::agent_session_registry_validation::rollout_metadata_matches_managed_agent_profile(
                name, role, metadata,
            )
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return Ok(None);
    };
    register_recovered_rollout_session(
        registry,
        project_id,
        root_session_id,
        name,
        role,
        expires_at,
        candidate,
        "unique-root-attributed-managed-rollout",
        now,
    )
}

#[allow(clippy::too_many_arguments)]
fn register_recovered_rollout_session(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    name: &str,
    role: &str,
    expires_at: Option<i64>,
    candidate: &agent_semantic_runtime::CodexRolloutSessionMetadata,
    recovery_source: &str,
    now: i64,
) -> Result<Option<AgentSessionRecord>, String> {
    let observed_model = candidate
        .model
        .as_deref()
        .or(candidate.collaboration_model.as_deref());
    let metadata_json = serde_json::json!({
        "native": true,
        "rootSessionId": root_session_id,
        "childSessionId": candidate.session_id,
        "agentRole": candidate.agent_role,
        "agentPath": candidate.agent_path,
        "model": observed_model,
        "reasoningEffort": candidate.reasoning_effort,
        "registryRecovery": recovery_source,
        "existingChildDiscovery": recovery_source,
        "messageTargetStatus": "unbound",
    })
    .to_string();
    registry.register_session(AgentSessionRegisterRequest {
        project_id,
        root_session_id,
        session_id: &candidate.session_id,
        message_target_id: None,
        parent_session_id: candidate
            .parent_thread_id
            .as_deref()
            .or(Some(root_session_id)),
        name,
        role,
        model_observation: observed_model.map(|model| AgentSessionModelObservationRef {
            model,
            source: AgentSessionModelObservationSource::CodexRollout,
            observed_at: now,
            evidence_ref: None,
        }),
        status: "existing-child-discovered",
        expires_at,
        metadata_json: &metadata_json,
        now,
    })?;
    registry.lookup_session(AgentSessionLookupRequest {
        project_id,
        session_id: Some(&candidate.session_id),
        root_session_id: Some(root_session_id),
        name: Some(name),
    })
}

pub(super) struct CodexHostRuntimeRefresh {
    pub(super) record: Option<AgentSessionRecord>,
    pub(super) fresh_after_previous_observation: bool,
    pub(super) runtime_override_blocked: bool,
    pub(super) observed_model: Option<String>,
    pub(super) observed_reasoning_effort: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_existing_codex_host_runtime(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    record: &AgentSessionRecord,
    expected_model: Option<&str>,
    expected_reasoning_effort: Option<&str>,
    now: i64,
) -> Result<CodexHostRuntimeRefresh, String> {
    let records = agent_semantic_runtime::codex_app_server_child_session_metadata(root_session_id)?;
    let candidates = records
        .iter()
        .filter(|metadata| metadata.session_id == record.session_id)
        .filter(|metadata| {
            super::agent_session_registry_validation::rollout_metadata_matches_managed_agent_profile(
                &record.name,
                &record.role,
                metadata,
            )
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return Ok(CodexHostRuntimeRefresh {
            record: None,
            fresh_after_previous_observation: false,
            runtime_override_blocked: false,
            observed_model: None,
            observed_reasoning_effort: None,
        });
    };
    let observed_model = candidate
        .model
        .as_deref()
        .or(candidate.collaboration_model.as_deref())
        .map(str::to_string);
    let observed_reasoning_effort = candidate.reasoning_effort.clone();
    let rollout_observed_at = std::fs::metadata(&candidate.rollout_path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(now);
    let fresh_after_previous_observation = record
        .model_observed_at
        .is_some_and(|previous| rollout_observed_at > previous);
    let runtime_matches = expected_model
        .is_none_or(|expected| observed_model.as_deref() == Some(expected))
        && expected_reasoning_effort
            .is_none_or(|expected| observed_reasoning_effort.as_deref() == Some(expected));
    let runtime_override_state = classify_same_child_runtime_override_state(
        &record.status,
        runtime_matches,
        fresh_after_previous_observation,
    );
    let runtime_override_blocked =
        runtime_override_state == SameChildRuntimeOverrideState::ReplacementRequired;
    let metadata_json = metadata_with_updates(
        record,
        serde_json::json!({
            "native": true,
            "rootSessionId": root_session_id,
            "childSessionId": candidate.session_id,
            "agentRole": candidate.agent_role,
            "agentPath": candidate.agent_path,
            "model": observed_model,
            "reasoningEffort": observed_reasoning_effort,
            "registryRecovery": "codex-app-server-native-host-tree-refresh",
            "freshAfterPreviousObservation": fresh_after_previous_observation,
            "expectedModel": expected_model,
            "expectedReasoningEffort": expected_reasoning_effort,
        }),
    );
    let evidence_ref = candidate.rollout_path.to_str();
    registry.register_session(AgentSessionRegisterRequest {
        project_id,
        root_session_id,
        session_id: &record.session_id,
        message_target_id: record.message_target_id(),
        parent_session_id: record
            .parent_session_id
            .as_deref()
            .or(Some(root_session_id)),
        name: &record.name,
        role: &record.role,
        model_observation: observed_model
            .as_deref()
            .map(|model| AgentSessionModelObservationRef {
                model,
                source: AgentSessionModelObservationSource::CodexRollout,
                observed_at: rollout_observed_at,
                evidence_ref,
            }),
        status: runtime_override_state.registry_status(),
        expires_at: record.expires_at,
        metadata_json: &metadata_json,
        now,
    })?;
    let refreshed = registry.lookup_session(AgentSessionLookupRequest {
        project_id,
        session_id: Some(&record.session_id),
        root_session_id: Some(root_session_id),
        name: Some(&record.name),
    })?;
    Ok(CodexHostRuntimeRefresh {
        record: refreshed,
        fresh_after_previous_observation,
        runtime_override_blocked,
        observed_model,
        observed_reasoning_effort,
    })
}

fn metadata_with_updates(record: &AgentSessionRecord, updates: serde_json::Value) -> String {
    let mut metadata = serde_json::from_str::<serde_json::Value>(record.metadata_json())
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if let Some(updates) = updates.as_object() {
        metadata.extend(updates.clone());
    }
    serde_json::Value::Object(metadata).to_string()
}
