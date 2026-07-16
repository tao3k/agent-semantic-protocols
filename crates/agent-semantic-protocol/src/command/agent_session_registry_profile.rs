use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_rollout_adopt::{
    RolloutAdoptRequest, adopt_reusable_rollout_session,
};
use super::{
    normalize_session_permissions, normalize_session_roles, session_permissions_for_roles,
    session_role_defaults_for_session_name,
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
