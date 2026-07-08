use agent_semantic_client_db::{
    AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionRegistry,
};
use agent_semantic_runtime::{CodexRolloutSessionIndex, codex_rollout_session_index};

use super::agent_session_registry_validation::{
    rollout_metadata_matches_managed_agent_profile, validate_recent_session_profile,
};
use super::normalized_metadata_with_roles;

pub(super) struct RolloutAdoptRequest<'a> {
    pub(super) project_id: &'a str,
    pub(super) root_session_id: &'a str,
    pub(super) name: &'a str,
    pub(super) role: &'a str,
    pub(super) roles: &'a [String],
    pub(super) permissions: &'a [String],
    pub(super) model: Option<&'a str>,
    pub(super) expires_at: Option<i64>,
    pub(super) now: i64,
    pub(super) excluded_session_id: Option<&'a str>,
}

pub(super) fn adopt_reusable_rollout_session(
    registry: &AgentSessionRegistry,
    request: RolloutAdoptRequest<'_>,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(index) = codex_rollout_session_index(request.root_session_id)? else {
        return Ok(None);
    };
    let Some((candidate_session_id, candidate_model)) = index
        .records
        .iter()
        .filter(|metadata| {
            metadata.root_session_id.as_deref() == Some(request.root_session_id)
                && request.excluded_session_id != Some(metadata.session_id.as_str())
                && metadata.session_id != request.root_session_id
                && rollout_metadata_matches_managed_agent_profile(
                    request.name,
                    request.role,
                    metadata,
                )
                && rollout_index_session_is_reusable(&index, &metadata.session_id)
        })
        .max_by_key(|metadata| rollout_index_session_score(&index, &metadata.session_id))
        .map(|metadata| (metadata.session_id.clone(), metadata.model.clone()))
    else {
        return Ok(None);
    };
    let validation = validate_recent_session_profile(
        &candidate_session_id,
        request.root_session_id,
        request.name,
        request.role,
        request.now,
    )?;
    if validation.status == "failed" {
        return Ok(None);
    }
    let metadata_json =
        normalized_metadata_with_roles(None, &validation, request.roles, request.permissions)?;
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: request.project_id,
            root_session_id: request.root_session_id,
            session_id: &candidate_session_id,
            message_target_id: None,
            parent_session_id: Some(request.root_session_id),
            name: request.name,
            role: request.role,
            model: candidate_model.as_deref().or(request.model),
            status: "active",
            expires_at: request.expires_at,
            metadata_json: &metadata_json,
            now: request.now,
        })
        .map(Some)
}

fn rollout_index_session_is_reusable(index: &CodexRolloutSessionIndex, session_id: &str) -> bool {
    index
        .activity_by_session
        .get(session_id)
        .map(|activity| {
            matches!(
                activity.status.as_str(),
                "tool-running" | "agent-active" | "idle-resumable"
            ) || !activity.running_session_closed
        })
        .unwrap_or(true)
}

fn rollout_index_session_score(index: &CodexRolloutSessionIndex, session_id: &str) -> i64 {
    index
        .activity_by_session
        .get(session_id)
        .and_then(|activity| activity.last_heartbeat_at.or(activity.last_event_at))
        .unwrap_or(0)
}
