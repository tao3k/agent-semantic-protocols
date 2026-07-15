//! Registry ownership transition for a host-created typed resident replacement.

use std::path::Path;

use super::AspSessionPolicy;

pub(super) fn release_terminal_owner_before_typed_start(
    project_root: &Path,
    native: &crate::codex::native_agent_transport::CodexNativeSubagentEvent,
    root_session_id: &str,
    asp_session_policy: &AspSessionPolicy,
) -> Result<(), String> {
    let registry =
        agent_semantic_client_db::AgentSessionRegistry::open_or_create_project(project_root)?;
    let project_id = agent_semantic_client_db::AgentSessionRegistry::project_scope_id(project_root);
    let Some(existing) = registry
        .query_sessions(
            &project_id,
            Some(root_session_id),
            Some(asp_session_policy.resident_child_name()),
        )?
        .into_iter()
        .find(|existing| {
            existing.session_id != native.agent_id
                && matches!(
                    existing.status.as_str(),
                    "invalid" | "replacement-required" | "orphan-risk" | "archived" | "closed"
                )
        })
    else {
        return Ok(());
    };
    registry
        .delete_session(&project_id, &existing.session_id)
        .map(|_| ())
}
