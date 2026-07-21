//! Lifecycle mutations for DB-owned agent session registry rows.

use crate::engine::turso_statement::execute_turso_operation;

use super::core::{
    AgentSessionRegistry, block_on_agent_session_registry_async,
    connect_turso_agent_session_registry,
};
use super::types::AgentSessionRecord;

impl AgentSessionRegistry {
    /// Atomically mark a resident child orphaned and revoke its native route.
    ///
    /// A host-tree `absent` observation invalidates the persisted message
    /// target attestation at the same instant that it changes routing status.
    /// Keeping either the target id or `messageTargetBinding` would allow
    /// status/bootstrap projections to resurrect a route the host can no
    /// longer resolve.
    pub fn invalidate_session_live_binding(
        &self,
        project_id: &str,
        session_id: &str,
        status: &str,
        now: i64,
    ) -> Result<Option<AgentSessionRecord>, String> {
        let changed =
            block_on_agent_session_registry_async(turso_invalidate_session_live_binding(
                self.db_path(),
                project_id,
                session_id,
                status,
                now,
            ))?;
        if !changed {
            return Ok(None);
        }
        self.session_by_id(project_id, session_id)
    }
}

async fn turso_invalidate_session_live_binding(
    db_path: &std::path::Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    crate::engine::turso_statement::execute_turso_statement(
        &connection,
        "BEGIN IMMEDIATE",
        "failed to begin Turso live-binding invalidation",
    )
    .await?;
    let invalidation = async {
        let changed =
            crate::engine::turso_statement::execute_turso_operation_with_statement_change_signal(
                &connection,
                || async {
                    connection
                        .execute(
                            "UPDATE asp_agent_sessions
                     SET status = ?1,
                         message_target_id = NULL,
                         metadata_json = CASE
                           WHEN json_type(metadata_json, '$.dispatchLease') IS NULL
                             THEN json_remove(metadata_json, '$.messageTargetBinding')
                           ELSE json_set(
                             json_remove(metadata_json, '$.messageTargetBinding'),
                             '$.dispatchLease.status', 'orphaned-awaiting-rebind',
                             '$.dispatchLease.deliveryTargetId', NULL,
                             '$.dispatchLease.revokedAt', ?2
                           )
                         END,
                         updated_at = ?2,
                         last_seen_at = ?2
                     WHERE project_id = ?3 AND session_id = ?4",
                            (status, now, project_id, session_id),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to invalidate Turso session live binding",
            )
            .await?;
        if changed {
            execute_turso_operation(
                || async {
                    connection
                        .execute(
                            "UPDATE asp_agent_dispatch_leases
                             SET status = 'orphaned-awaiting-rebind',
                                 delivery_target_id = NULL,
                                 updated_at = ?1
                             WHERE project_id = ?2
                               AND delivery_target_id = ?3
                               AND status = 'in-flight'",
                            (now, project_id, session_id),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to orphan Turso agent dispatch leases",
            )
            .await?;
        }
        Ok::<bool, String>(changed)
    }
    .await;
    let changed = match invalidation {
        Ok(changed) => changed,
        Err(error) => {
            let _ = crate::engine::turso_statement::execute_turso_statement(
                &connection,
                "ROLLBACK",
                "failed to roll back Turso live-binding invalidation",
            )
            .await;
            return Err(error);
        }
    };
    crate::engine::turso_statement::execute_turso_statement(
        &connection,
        "COMMIT",
        "failed to commit Turso live-binding invalidation",
    )
    .await?;
    Ok(changed)
}
