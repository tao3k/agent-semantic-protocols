use super::types::AgentSessionRecord;

pub(super) fn from_turso_row(row: &turso::Row) -> Result<AgentSessionRecord, String> {
    macro_rules! read {
        ($index:expr, $field:literal) => {
            row.get($index)
                .map_err(|error| format!("failed to read Turso {}: {error}", $field))?
        };
    }
    Ok(AgentSessionRecord {
        project_id: read!(0, "project id"),
        root_session_id: read!(1, "root session id"),
        session_id: read!(2, "session id"),
        physical_generation: read!(3, "physical generation"),
        configured_agent_type: read!(4, "configured agent type"),
        profile_evidence_json: read!(5, "profile evidence json"),
        message_target_id: read!(6, "message target id"),
        parent_session_id: read!(7, "parent session id"),
        name: read!(8, "session name"),
        role: read!(9, "session role"),
        model: read!(10, "session model"),
        model_observation_source: read!(11, "model observation source"),
        model_observed_at: read!(12, "model observed_at"),
        model_evidence_ref: read!(13, "model evidence ref"),
        status: read!(14, "session status"),
        created_at: read!(15, "session created_at"),
        updated_at: read!(16, "session updated_at"),
        last_seen_at: read!(17, "session last_seen_at"),
        last_heartbeat_at: read!(18, "session last_heartbeat_at"),
        expires_at: read!(19, "session expires_at"),
        archived_at: read!(20, "session archived_at"),
        last_tool_event: read!(21, "session last_tool_event"),
        last_command: read!(22, "session last_command"),
        last_evidence_ref: read!(23, "session last_evidence_ref"),
        metadata_json: read!(24, "session metadata_json"),
    })
}

pub(super) fn select_sql(predicate: &str) -> String {
    format!(
        "SELECT project_id, root_session_id, session_id, physical_generation, configured_agent_type, profile_evidence_json, message_target_id, parent_session_id, name, role, model, model_observation_source, model_observed_at, model_evidence_ref, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json FROM asp_agent_sessions {predicate}"
    )
}
