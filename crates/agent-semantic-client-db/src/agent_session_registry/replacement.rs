//! Atomic compare-and-swap replacement for one resident route owner.

use crate::engine::turso_statement::{execute_turso_operation, execute_turso_statement};

use super::{
    core::{
        AgentSessionRegistry, block_on_agent_session_registry_async,
        connect_turso_agent_session_registry, turso_session_by_name,
    },
    types::{AgentSessionId, AgentSessionRecord, AgentSessionRegisterRequest},
};

impl AgentSessionRegistry {
    /// Atomically replace one exact resident route owner.
    ///
    /// The compare-and-swap guard prevents a late hook from overwriting a
    /// newer native child that already claimed the same root/name route.
    pub fn replace_resident_session(
        &self,
        expected_session_id: impl Into<AgentSessionId>,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        let expected_session_id = expected_session_id.into();
        block_on_agent_session_registry_async(turso_replace_resident_session(
            self.db_path(),
            expected_session_id.as_str(),
            request,
        ))
    }
}

async fn turso_replace_resident_session(
    db_path: &std::path::Path,
    expected_session_id: &str,
    request: AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_statement(
        &connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso resident session replacement",
    )
    .await?;
    let replacement =
        replace_resident_session_in_transaction(&connection, expected_session_id, &request).await;
    if let Err(error) = replacement {
        let _ = execute_turso_statement(
            &connection,
            "ROLLBACK",
            "failed to roll back Turso resident session replacement",
        )
        .await;
        return Err(error);
    }
    execute_turso_statement(
        &connection,
        "COMMIT",
        "failed to commit Turso resident session replacement",
    )
    .await?;
    turso_session_by_name(
        db_path,
        request.project_id,
        request.root_session_id,
        request.name,
    )
    .await?
    .ok_or_else(|| "replaced Turso resident session was not readable".to_string())
}

async fn replace_resident_session_in_transaction(
    connection: &turso::Connection,
    expected_session_id: &str,
    request: &AgentSessionRegisterRequest<'_>,
) -> Result<(), String> {
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_agent_sessions
                     WHERE project_id = ?1
                       AND session_id = ?2
                       AND NOT (root_session_id = ?3 AND name = ?4)",
                    (
                        request.project_id,
                        request.session_id,
                        request.root_session_id,
                        request.name,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to clear conflicting Turso resident session mapping",
    )
    .await?;
    let changed = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET session_id = ?1,
                         physical_generation = physical_generation + 1,
                         message_target_id = ?2,
                         parent_session_id = ?3,
                         role = ?4,
                         model = ?5,
                         model_observation_source = ?6,
                         model_observed_at = ?7,
                         model_evidence_ref = ?8,
                         status = ?9,
                         updated_at = ?10,
                         last_seen_at = ?10,
                         last_heartbeat_at = ?10,
                         expires_at = ?11,
                         archived_at = NULL,
                         metadata_json = ?12,
                         configured_agent_type = CASE WHEN json_valid(?12) AND json_extract(?12, '$.event') = 'subagent-start' AND json_extract(?12, '$.native') = 1 THEN json_extract(?12, '$.agentType') END,
                         profile_evidence_json = CASE WHEN json_valid(?12) AND json_extract(?12, '$.event') = 'subagent-start' AND json_extract(?12, '$.native') = 1 THEN ?12 END
                     WHERE project_id = ?13
                       AND root_session_id = ?14
                       AND name = ?15
                       AND session_id = ?16",
                    (
                        request.session_id,
                        request.message_target_id,
                        request.parent_session_id,
                        request.role,
                        request
                            .model_observation
                            .as_ref()
                            .map(|observation| observation.model),
                        request
                            .model_observation
                            .as_ref()
                            .map(|observation| observation.source.as_str()),
                        request
                            .model_observation
                            .as_ref()
                            .map(|observation| observation.observed_at),
                        request
                            .model_observation
                            .as_ref()
                            .and_then(|observation| observation.evidence_ref),
                        request.status,
                        request.now,
                        request.expires_at,
                        request.metadata_json,
                        request.project_id,
                        request.root_session_id,
                        request.name,
                        expected_session_id,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to replace Turso resident session",
    )
    .await?;
    if changed != 1 {
        return Err(format!(
            "resident session replacement compare-and-swap failed: expected={expected_session_id} route={}/{}",
            request.root_session_id, request.name
        ));
    }
    Ok(())
}
