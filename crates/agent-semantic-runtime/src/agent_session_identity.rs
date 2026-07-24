//! Agent-session identity helpers shared by CLI and hook runtime code.

use crate::agent_session_status::{codex_rollout_session_metadata, current_agent_runtime_session};

/// Runtime-resolved identity for registering one agent session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSessionRegistrationIdentity {
    /// Session id that should be stored as the child/worker session.
    pub session_id: String,
    /// Root session id used to group resident child sessions.
    pub root_session_id: String,
}

/// Request for resolving register-time child/root session identity.
pub struct AgentSessionRegistrationIdentityRequest<'a> {
    child_session_id: Option<&'a str>,
    root_session_id: Option<&'a str>,
}

impl<'a> From<(Option<&'a str>, Option<&'a str>)> for AgentSessionRegistrationIdentityRequest<'a> {
    fn from((child_session_id, root_session_id): (Option<&'a str>, Option<&'a str>)) -> Self {
        Self {
            child_session_id,
            root_session_id,
        }
    }
}

/// Return whether the host process exposes one supported agent session id.
#[must_use]
pub fn has_current_agent_runtime_session() -> bool {
    current_agent_runtime_session().is_some()
}

/// Resolve the root session id for the current runtime session.
#[must_use]
pub fn current_agent_runtime_root_session_id() -> Option<String> {
    let session = current_agent_runtime_session()?;
    let runtime_session_id =
        crate::agent_session_status::RuntimeSessionId::from(session.id.as_str());
    codex_rollout_root_session_id(&runtime_session_id)
        .or_else(|| Some(session.recall_session_id().to_string()))
}

/// Resolve the root id recorded in Codex rollout metadata for `session_id`.
#[must_use]
pub(crate) fn codex_rollout_root_session_id(
    session_id: &crate::agent_session_status::RuntimeSessionId,
) -> Option<String> {
    codex_rollout_session_metadata(session_id)
        .ok()
        .flatten()
        .and_then(|metadata| metadata.root_session_id.or(metadata.parent_thread_id))
}

/// Resolve register-time child/root identity from explicit args and host state.
pub fn agent_session_registration_identity(
    request: AgentSessionRegistrationIdentityRequest<'_>,
) -> Result<AgentSessionRegistrationIdentity, String> {
    let runtime_session = current_agent_runtime_session();
    let session_id = request
        .child_session_id
        .map(str::to_string)
        .or_else(|| runtime_session.as_ref().map(|session| session.id.clone()))
        .ok_or_else(|| {
            "asp agent session register requires --child-session-id or an agent session env"
                .to_string()
        })?;
    let session_id_for_rollout =
        crate::agent_session_status::RuntimeSessionId::from(session_id.as_str());
    let root_session_id = request
        .root_session_id
        .map(str::to_string)
        .or_else(|| codex_rollout_root_session_id(&session_id_for_rollout))
        .or_else(|| {
            runtime_session
                .as_ref()
                .map(|session| session.recall_session_id().to_string())
        })
        .unwrap_or_else(|| session_id.clone());
    Ok(AgentSessionRegistrationIdentity {
        session_id,
        root_session_id,
    })
}
