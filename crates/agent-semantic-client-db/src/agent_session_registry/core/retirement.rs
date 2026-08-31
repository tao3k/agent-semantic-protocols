use std::path::Path;

use super::storage::connect_turso_agent_session_registry;

pub(super) async fn turso_retire_and_delete_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    connection
        .execute("BEGIN IMMEDIATE", ())
        .await
        .map_err(|error| format!("failed to begin Turso session retirement: {error}"))?;

    let result = async {
        connection
            .execute(
                "INSERT OR IGNORE INTO asp_agent_session_retirements (
                    project_id, root_session_id, name, session_id, physical_generation, retired_at
                )
                SELECT project_id, root_session_id, name, session_id, physical_generation, unixepoch()
                FROM asp_agent_sessions
                WHERE project_id = ?1 AND session_id = ?2",
                (project_id, session_id),
            )
            .await
            .map_err(|error| format!("failed to persist Turso session retirement: {error}"))?;
        connection
            .execute(
                "DELETE FROM asp_agent_sessions WHERE project_id = ?1 AND session_id = ?2",
                (project_id, session_id),
            )
            .await
            .map_err(|error| format!("failed to delete Turso session row: {error}"))
    }
    .await;

    match result {
        Ok(changes) => {
            connection
                .execute("COMMIT", ())
                .await
                .map_err(|error| format!("failed to commit Turso session retirement: {error}"))?;
            Ok(changes > 0)
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK", ()).await;
            Err(error)
        }
    }
}

pub(super) async fn turso_session_is_retired(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut rows = connection
        .query(
            "SELECT 1 FROM asp_agent_session_retirements
             WHERE project_id = ?1 AND session_id = ?2 LIMIT 1",
            (project_id, session_id),
        )
        .await
        .map_err(|error| format!("failed to read Turso session retirement: {error}"))?;
    rows.next()
        .await
        .map(|row| row.is_some())
        .map_err(|error| format!("failed to read Turso session retirement row: {error}"))
}
