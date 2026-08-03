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
            RootAttributedRolloutAdoption {
                project_id,
                root_session_id,
                name,
                role: &profile.role,
                expires_at: args.expires_at,
                excluded_session_id,
                now,
            },
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

struct RootAttributedRolloutAdoption<'a> {
    project_id: &'a str,
    root_session_id: &'a str,
    name: &'a str,
    role: &'a str,
    expires_at: Option<i64>,
    excluded_session_id: Option<&'a str>,
    now: i64,
}

fn adopt_unique_root_attributed_rollout_session(
    registry: &AgentSessionRegistry,
    adoption: RootAttributedRolloutAdoption<'_>,
) -> Result<Option<AgentSessionRecord>, String> {
    let RootAttributedRolloutAdoption {
        project_id,
        root_session_id,
        name,
        role,
        expires_at,
        excluded_session_id,
        now,
    } = adoption;
    let host_records =
        agent_semantic_runtime::codex_app_server_child_session_metadata(&root_session_id.into())?;
    let host_candidates = host_records
        .iter()
        .filter(|metadata| excluded_session_id != Some(metadata.session_id().as_str()))
        .filter(|metadata| {
            super::agent_session_registry_validation::rollout_metadata_matches_host_agent_identity(
                name, role, metadata,
            )
        })
        .collect::<Vec<_>>();
    match host_candidates.as_slice() {
        [candidate] => {
            return register_recovered_rollout_session(
                registry,
                RecoveredRolloutRegistration {
                    project_id,
                    root_session_id,
                    name,
                    role,
                    expires_at,
                    candidate,
                    recovery_source: "codex-app-server-native-host-tree",
                    now,
                },
            );
        }
        [] => {}
        _ => return Ok(None),
    }
    let Some(index) = agent_semantic_runtime::codex_rollout_session_index(&root_session_id.into())?
    else {
        return Ok(None);
    };
    let candidates = index
        .records
        .iter()
        .filter(|metadata| excluded_session_id != Some(metadata.session_id().as_str()))
        .filter(|metadata| {
            super::agent_session_registry_validation::rollout_metadata_matches_host_agent_identity(
                name, role, metadata,
            )
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return Ok(None);
    };
    register_recovered_rollout_session(
        registry,
        RecoveredRolloutRegistration {
            project_id,
            root_session_id,
            name,
            role,
            expires_at,
            candidate,
            recovery_source: "unique-root-attributed-managed-rollout",
            now,
        },
    )
}

struct RecoveredRolloutRegistration<'a> {
    project_id: &'a str,
    root_session_id: &'a str,
    name: &'a str,
    role: &'a str,
    expires_at: Option<i64>,
    candidate: &'a agent_semantic_runtime::CodexRolloutSessionMetadata,
    recovery_source: &'a str,
    now: i64,
}

fn register_recovered_rollout_session(
    registry: &AgentSessionRegistry,
    request: RecoveredRolloutRegistration<'_>,
) -> Result<Option<AgentSessionRecord>, String> {
    let observed_model = request
        .candidate
        .model()
        .or(request.candidate.collaboration_model());
    let metadata_json = serde_json::json!({
        "native": true,
        "rootSessionId": request.root_session_id,
        "childSessionId": request.candidate.session_id(),
        "agentRole": request.candidate.agent_role(),
        "agentPath": request.candidate.agent_path(),
        "model": observed_model,
        "reasoningEffort": request.candidate.reasoning_effort(),
        "registryRecovery": request.recovery_source,
        "existingChildDiscovery": request.recovery_source,
        "messageTargetStatus": "unbound",
    })
    .to_string();
    registry.register_session(AgentSessionRegisterRequest {
        project_id: request.project_id.into(),
        root_session_id: request.root_session_id.into(),
        session_id: request.candidate.session_id().as_str().into(),
        message_target_id: None,
        parent_session_id: request
            .candidate
            .parent_thread_id()
            .map(|session_id| session_id.as_str())
            .or(Some(request.root_session_id))
            .map(Into::into),
        name: request.name.into(),
        role: request.role.into(),
        model_observation: observed_model.map(|model| AgentSessionModelObservationRef {
            model,
            source: AgentSessionModelObservationSource::CodexRollout,
            observed_at: request.now,
            evidence_ref: None,
        }),
        status: "existing-child-discovered".into(),
        expires_at: request.expires_at,
        metadata_json: (&metadata_json).into(),
        now: request.now,
    })?;
    registry.lookup_session(AgentSessionLookupRequest {
        project_id: request.project_id.into(),
        session_id: Some(request.candidate.session_id().as_str().into()),
        root_session_id: Some(request.root_session_id.into()),
        name: Some(request.name.into()),
    })
}
