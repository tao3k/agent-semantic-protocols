use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_rollout_adopt::{
    RolloutAdoptRequest, adopt_reusable_rollout_session,
};
use super::{
    normalize_session_permissions, normalize_session_roles, session_permissions_for_roles,
    session_role_defaults_for_session_name,
};
use agent_semantic_client_db::{AgentSessionRecord, AgentSessionRegistry};

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
    let record = adopt_reusable_rollout_session(
        registry,
        RolloutAdoptRequest {
            project_id,
            root_session_id,
            name,
            role: &profile.role,
            roles: &profile.roles,
            permissions: &profile.permissions,
            model: args.model.as_deref(),
            expires_at: args.expires_at,
            now,
            excluded_session_id,
        },
    )?;
    Ok(if let Some(record) = record {
        RolloutHistoryPreflight {
            status: "adopted-reusable-rollout",
            action: "resume-adopted-existing-child-then-validate-message-route",
            record: Some(record),
        }
    } else {
        RolloutHistoryPreflight {
            status: "checked-no-reusable-rollout",
            action: "create-resident-child-after-rollout-history-miss",
            record: None,
        }
    })
}
