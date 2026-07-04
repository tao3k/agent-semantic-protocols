//! Tool-event updates for registered agent sessions.

use agent_semantic_client_db::{AgentSessionToolEventRequest, agent_session_unix_timestamp};
use std::path::Path;

use super::agent_session_registry_state::open_existing_registry;

pub(crate) fn record_current_session_tool_event(
    project_root: &Path,
    tool_event: &str,
    command: Option<&str>,
    evidence_ref: Option<&str>,
) -> Result<bool, String> {
    let Some(session) = super::super::agent_session::current_agent_session() else {
        return Ok(false);
    };
    let Some(conn) = open_existing_registry(project_root)? else {
        return Ok(false);
    };
    conn.record_tool_event(AgentSessionToolEventRequest {
        session_id: &session.id,
        tool_event,
        command,
        evidence_ref,
        now: agent_session_unix_timestamp()?,
    })
}
