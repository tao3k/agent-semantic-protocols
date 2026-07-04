//! Lifecycle mutations for DB-owned agent session registry rows.

use std::path::Path;

use super::core::{
    AgentSessionRegistry, block_on_agent_session_registry_async,
    connect_turso_agent_session_registry,
};
use super::{AGENT_SESSION_STATUS_ARCHIVED, AGENT_SESSION_STATUS_IDLE};

impl AgentSessionRegistry {
    /// Mark a session as archived in the DB registry.
    pub fn archive_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_archive_session(
            self.db_path(),
            project_id,
            session_id,
            now,
        ))
    }

    /// Restore an archived session to an idle, routable registry state.
    pub fn unarchive_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_unarchive_session(
            self.db_path(),
            project_id,
            session_id,
            now,
        ))
    }

    /// Remove a session row from the DB registry.
    pub fn delete_session(&self, project_id: &str, session_id: &str) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_delete_session(
            self.db_path(),
            project_id,
            session_id,
        ))
    }
}

async fn turso_archive_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    now: i64,
) -> Result<bool, String> {
    let rows = connect_turso_agent_session_registry(db_path)
        .await?
        .execute(
            "UPDATE asp_agent_sessions
             SET status = ?3,
                 archived_at = ?4,
                 updated_at = ?4
             WHERE project_id = ?1
               AND session_id = ?2",
            (project_id, session_id, AGENT_SESSION_STATUS_ARCHIVED, now),
        )
        .await
        .map_err(|error| format!("failed to archive Turso session: {error}"))?;
    Ok(rows > 0)
}

async fn turso_unarchive_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    now: i64,
) -> Result<bool, String> {
    let rows = connect_turso_agent_session_registry(db_path)
        .await?
        .execute(
            "UPDATE asp_agent_sessions
             SET status = ?3,
                 archived_at = NULL,
                 updated_at = ?4
             WHERE project_id = ?1
               AND session_id = ?2",
            (project_id, session_id, AGENT_SESSION_STATUS_IDLE, now),
        )
        .await
        .map_err(|error| format!("failed to unarchive Turso session: {error}"))?;
    Ok(rows > 0)
}

async fn turso_delete_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    let rows = connect_turso_agent_session_registry(db_path)
        .await?
        .execute(
            "DELETE FROM asp_agent_sessions
             WHERE project_id = ?1
               AND session_id = ?2",
            (project_id, session_id),
        )
        .await
        .map_err(|error| format!("failed to delete Turso session: {error}"))?;
    Ok(rows > 0)
}
