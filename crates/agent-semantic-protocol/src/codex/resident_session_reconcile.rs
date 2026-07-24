use agent_semantic_client_db::{AgentSessionRecord, AgentSessionRegistry};

use super::rollout::{CodexRolloutSessionLiveness, rollout_session_liveness_for_session_id};

#[derive(Clone, Debug)]
pub(crate) struct CodexResidentSessionReconciliation {
    pub(crate) current: Option<CodexResidentSessionCandidate>,
    pub(crate) historical_resident_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct CodexResidentSessionCandidate {
    pub(crate) session: AgentSessionRecord,
    pub(crate) liveness: CodexRolloutSessionLiveness,
}

/// Resolve the one resident child permitted for a root before bootstrap or a
/// native start claims a new child. A completed turn stays resumable until an
/// explicit lifecycle event archives it.
pub(crate) fn reconcile_resident_session(
    registry: &AgentSessionRegistry,
    project_id: &str,
    root_session_id: &str,
    name: &str,
    role: &str,
) -> Result<CodexResidentSessionReconciliation, String> {
    let active_sessions: Vec<_> = registry
        .query_sessions(
            project_id,
            None,
            Some(agent_semantic_client_db::AgentSessionResidentName::from(
                name,
            )),
        )?
        .into_iter()
        .filter(|session| session.name == name || session.role == role)
        .filter(|session| !matches!(session.status.as_str(), "archived" | "closed"))
        .collect();
    let historical_resident_count = active_sessions
        .iter()
        .filter(|session| session.root_session_id != root_session_id)
        .count();
    let current = active_sessions
        .into_iter()
        .filter(|session| session.root_session_id == root_session_id)
        .max_by_key(|session| session.updated_at)
        .map(|session| CodexResidentSessionCandidate {
            liveness: rollout_session_liveness_for_session_id(&session.session_id),
            session,
        });

    Ok(CodexResidentSessionReconciliation {
        current,
        historical_resident_count,
    })
}
