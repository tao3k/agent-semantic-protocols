use std::path::Path;

use turso::Connection;

use super::storage::connect_turso_agent_session_registry;

pub(super) async fn bootstrap_turso_agent_session_retirement_schema(
    connection: &Connection,
) -> Result<(), String> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS asp_agent_session_retirements (
                project_id TEXT NOT NULL,
                root_session_id TEXT NOT NULL,
                name TEXT NOT NULL,
                session_id TEXT NOT NULL UNIQUE,
                physical_generation INTEGER NOT NULL,
                retired_at INTEGER NOT NULL,
                PRIMARY KEY(project_id, root_session_id, name, physical_generation)
            );
            CREATE INDEX IF NOT EXISTS idx_asp_agent_session_retirements_route
                ON asp_agent_session_retirements(project_id, root_session_id, name);
            CREATE TRIGGER IF NOT EXISTS asp_agent_sessions_reject_retired_session
            BEFORE INSERT ON asp_agent_sessions
            WHEN EXISTS (
                SELECT 1 FROM asp_agent_session_retirements AS retired
                WHERE retired.project_id = NEW.project_id
                  AND retired.session_id = NEW.session_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'retired physical session generation cannot be registered again');
            END;
            CREATE TRIGGER IF NOT EXISTS asp_agent_sessions_advance_retired_generation
            AFTER INSERT ON asp_agent_sessions
            BEGIN
                UPDATE asp_agent_sessions
                SET physical_generation = MAX(
                    NEW.physical_generation,
                    COALESCE(
                        (
                            SELECT MAX(retired.physical_generation) + 1
                            FROM asp_agent_session_retirements AS retired
                            WHERE retired.project_id = NEW.project_id
                              AND retired.root_session_id = NEW.root_session_id
                              AND retired.name = NEW.name
                        ),
                        NEW.physical_generation
                    )
                )
                WHERE project_id = NEW.project_id
                  AND root_session_id = NEW.root_session_id
                  AND name = NEW.name;
            END;",
        )
        .await
        .map_err(|error| format!("failed to initialize Turso session retirement schema: {error}"))?;
    Ok(())
}

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
