//! Bootstrap migrations for agent session registry storage.

use crate::engine::turso_statement::execute_turso_operation;

pub(super) async fn dedupe_turso_agent_sessions_by_session_id(
    connection: &turso::Connection,
) -> Result<(), String> {
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_agent_sessions AS stale
                     WHERE EXISTS (
                         SELECT 1
                         FROM asp_agent_sessions AS keep
                         WHERE keep.session_id = stale.session_id
                           AND (
                               keep.updated_at > stale.updated_at
                               OR (
                                   keep.updated_at = stale.updated_at
                                   AND keep.created_at > stale.created_at
                               )
                               OR (
                                   keep.updated_at = stale.updated_at
                                   AND keep.created_at = stale.created_at
                                   AND keep.rowid > stale.rowid
                               )
                           )
                     )",
                    (),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to dedupe Turso sessions by session_id",
    )
    .await?;
    Ok(())
}
