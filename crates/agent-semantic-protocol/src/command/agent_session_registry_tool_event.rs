//! Tool-event updates for registered agent sessions.

use rusqlite::params;
use std::path::Path;

pub(crate) fn record_current_session_tool_event(
    project_root: &Path,
    tool_event: &str,
    command: Option<&str>,
    evidence_ref: Option<&str>,
) -> Result<bool, String> {
    let Some(session) = super::super::agent_session::current_agent_session() else {
        return Ok(false);
    };
    let Some(conn) = super::open_existing_registry(project_root)? else {
        return Ok(false);
    };
    let now = super::unix_timestamp()?;
    let rows = conn
        .execute(
            "UPDATE asp_agent_sessions
             SET updated_at = ?2,
                 last_seen_at = ?2,
                 last_heartbeat_at = ?2,
                 last_tool_event = ?3,
                 last_command = COALESCE(?4, last_command),
                 last_evidence_ref = COALESCE(?5, last_evidence_ref)
             WHERE session_id = ?1
               AND status IN ('active', 'idle')",
            params![session.id, now, tool_event, command, evidence_ref],
        )
        .map_err(|error| format!("failed to record session tool event: {error}"))?;
    Ok(rows > 0)
}
