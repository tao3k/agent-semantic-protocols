//! Host-envelope and registry identity helpers for configured residents.

use std::path::Path;

pub(super) fn top_level_string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    })
}

pub(super) fn registered_resident_session_by_id(
    project_root: &Path,
    session_id: &str,
) -> Result<Option<agent_semantic_client_db::AgentSessionRecord>, String> {
    let Some(registry) =
        agent_semantic_client_db::AgentSessionRegistry::open_existing_project(project_root)?
    else {
        return Ok(None);
    };
    let project_id = agent_semantic_client_db::AgentSessionRegistry::project_scope_id(project_root);
    registry.lookup_session(agent_semantic_client_db::AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: Some(session_id),
        root_session_id: None,
        name: None,
    })
}
