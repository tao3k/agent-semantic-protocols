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
    pub(super) expires_at: Option<i64>,
    pub(super) now: i64,
    pub(super) excluded_session_id: Option<&'a str>,
}

pub(super) fn adopt_reusable_rollout_session(
    registry: &AgentSessionRegistry,
    request: RolloutAdoptRequest<'_>,
) -> Result<Option<AgentSessionRecord>, String> {
    let Some(index) = codex_rollout_session_index(&request.root_session_id.into())? else {
        return Ok(None);
    };
    let Some((candidate_session_id, candidate_model)) = index
        .records
        .iter()
        .filter(|metadata| {
            metadata
                .root_session_id()
                .map(|session_id| session_id.as_str())
                == Some(request.root_session_id)
                && request.excluded_session_id != Some(metadata.session_id().as_str())
                && metadata.session_id().as_str() != request.root_session_id
                && rollout_metadata_matches_managed_agent_profile(
                    request.name,
                    request.role,
                    metadata,
                )
                && rollout_index_session_is_reusable(&index, metadata.session_id().as_str())
        })
        .max_by_key(|metadata| rollout_index_session_score(&index, metadata.session_id().as_str()))
        .map(|metadata| {
            (
                metadata.session_id().clone(),
                metadata.model().map(str::to_owned),
            )
        })
    else {
        return Ok(None);
    };
    if registry.session_is_retired(request.project_id, candidate_session_id.as_str())? {
        return Ok(None);
    }
    let validation = validate_recent_session_profile(
        candidate_session_id.as_str(),
        request.root_session_id,
        request.name,
        request.role,
        request.now,
    )?;
    if validation.status().as_str() == "failed" {
        return Ok(None);
    }
    let metadata_json =
        normalized_metadata_with_roles(None, &validation, request.roles, request.permissions)?;
    registry
        .register_session(AgentSessionRegisterRequest {
            project_id: request.project_id.into(),
            root_session_id: request.root_session_id.into(),
            session_id: candidate_session_id.as_str().into(),
            message_target_id: None,
            parent_session_id: Some(request.root_session_id.into()),
            name: request.name.into(),
            role: request.role.into(),
            model_observation: candidate_model.as_deref().map(|model| {
                agent_semantic_client_db::AgentSessionModelObservationRef {
                    model,
                    source:
                        agent_semantic_client_db::AgentSessionModelObservationSource::CodexRollout,
                    observed_at: request.now,
                    evidence_ref: None,
                }
            }),
            status: "existing-child-discovered".into(),
            expires_at: request.expires_at,
            metadata_json: (&metadata_json).into(),
            now: request.now,
        })
        .map(Some)
}

fn rollout_index_session_is_reusable(index: &CodexRolloutSessionIndex, session_id: &str) -> bool {
    index
        .activity_by_session
        .iter()
        .find_map(|(candidate_id, activity)| {
            (candidate_id.as_str() == session_id).then_some(activity)
        })
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
        .iter()
        .find_map(|(candidate_id, activity)| {
            (candidate_id.as_str() == session_id).then_some(activity)
        })
        .and_then(|activity| activity.last_heartbeat_at.or(activity.last_event_at))
        .unwrap_or(0)
}
