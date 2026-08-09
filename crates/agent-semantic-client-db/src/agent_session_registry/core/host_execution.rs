//! Turso refinement for a Host-observed execution of an existing child.

use std::path::Path;

use crate::engine::turso_statement::execute_turso_operation;

use super::storage::connect_turso_agent_session_registry;

pub(super) async fn turso_observe_host_execution(
    db_path: &Path,
    observation: &crate::workspace_db_ipc::AgentHostExecutionObservationIpc,
    metadata_json: &str,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET status = 'active',
                         message_target_id = ?1,
                         expires_at = NULL,
                         metadata_json = ?2,
                         updated_at = ?3,
                         last_seen_at = ?3
                     WHERE project_id = ?4
                       AND root_session_id = ?5
                       AND session_id = ?1
                       AND name = ?6
                       AND status NOT IN ('achieved', 'invalid')",
                    (
                        observation.child_session_id.as_str(),
                        metadata_json,
                        observation.observed_at,
                        observation.project_id.as_str(),
                        observation.root_session_id.as_str(),
                        observation.platform_host_agent_name.as_str(),
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to refine Turso session from Host execution observation",
    )
    .await?;
    Ok(changes > 0)
}
