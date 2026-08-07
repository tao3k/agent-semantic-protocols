//! Runtime, connection, and schema bootstrap for the agent-session registry.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use crate::engine::{
    turso_lock_policy::{
        TURSO_CLIENT_DB_BUSY_TIMEOUT_MS, TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS, is_turso_lock_error,
        turso_lock_retry_delay,
    },
    turso_statement::{execute_turso_statement, run_turso_operation},
};

use super::bootstrap::dedupe_turso_agent_sessions_by_session_id;

pub(in crate::agent_session_registry) fn block_on_agent_session_registry_async<T: Send>(
    future: impl std::future::Future<Output = Result<T, String>> + Send,
) -> Result<T, String> {
    crate::engine::facade::block_on_db_engine_borrowed(future)
}

pub(in crate::agent_session_registry) async fn connect_turso_agent_session_registry(
    db_path: &Path,
) -> Result<turso::Connection, String> {
    let mut last_lock_error = None;
    for attempt in 0..TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS {
        match connect_turso_agent_session_registry_once(db_path).await {
            Ok(connection) => return Ok(connection),
            Err(error)
                if is_turso_lock_error(&error)
                    && attempt + 1 < TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS =>
            {
                last_lock_error = Some(error);
                tokio::time::sleep(turso_lock_retry_delay(attempt)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(format!(
        "failed to open Turso agent session registry after lock retries: {}",
        last_lock_error.unwrap_or_else(|| "unknown lock error".to_string())
    ))
}

async fn connect_turso_agent_session_registry_once(
    db_path: &Path,
) -> Result<turso::Connection, String> {
    let db_path = prepare_turso_agent_session_registry_path(db_path)?;
    let database = crate::engine::shared_turso_database(&db_path)
        .await
        .map_err(|error| format!("failed to open Turso agent session registry: {error}"))?;
    let connection = database
        .connect()
        .map_err(|error| format!("failed to connect Turso agent session registry: {error}"))?;
    connection
        .busy_timeout(Duration::from_millis(TURSO_CLIENT_DB_BUSY_TIMEOUT_MS))
        .map_err(|error| {
            format!("failed to configure Turso agent session registry busy timeout: {error}")
        })?;
    Ok(connection)
}

fn prepare_turso_agent_session_registry_path(db_path: &Path) -> Result<PathBuf, String> {
    if db_path.is_file() {
        return Ok(db_path.to_path_buf());
    }
    super::permissions::prepare_private_registry_path(db_path)
}

pub(super) async fn bootstrap_turso_agent_session_schema(db_path: &Path) -> Result<(), String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_statement(
        &connection,
        "CREATE TABLE IF NOT EXISTS asp_agent_sessions (
            project_id TEXT NOT NULL DEFAULT 'default',
            root_session_id TEXT NOT NULL,
            session_id TEXT NOT NULL UNIQUE,
            physical_generation INTEGER NOT NULL DEFAULT 1,
            configured_agent_type TEXT,
            profile_evidence_json TEXT,
            message_target_id TEXT,
            parent_session_id TEXT,
            name TEXT NOT NULL,
            role TEXT NOT NULL,
            model TEXT,
            model_observation_source TEXT,
            model_observed_at INTEGER,
            model_evidence_ref TEXT,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_seen_at INTEGER,
            last_heartbeat_at INTEGER,
            expires_at INTEGER,
            archived_at INTEGER,
            last_tool_event TEXT,
            last_command TEXT,
            last_evidence_ref TEXT,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            PRIMARY KEY(project_id, root_session_id, name)
    )",
        "failed to initialize Turso session registry schema",
    )
    .await?;
    super::retirement::bootstrap_turso_agent_session_retirement_schema(&connection).await?;
    super::dispatch::bootstrap_turso_agent_dispatch_schema(&connection).await?;
    execute_turso_statement(
        &connection,
        "CREATE TABLE IF NOT EXISTS asp_host_child_match_decisions (
            project_id TEXT NOT NULL,
            root_session_id TEXT NOT NULL,
            child_session_id TEXT NOT NULL,
            host_task_name TEXT NOT NULL,
            match_decision TEXT NOT NULL CHECK(match_decision = 'none'),
            lifecycle_state TEXT NOT NULL,
            payload_digest TEXT NOT NULL,
            observed_at INTEGER NOT NULL,
            PRIMARY KEY(project_id, root_session_id, child_session_id)
        )",
        "failed to initialize Host child match-decision schema",
    )
    .await?;
    ensure_turso_agent_sessions_project_id_column(&connection).await?;
    ensure_turso_agent_sessions_message_target_id_column(&connection).await?;
    ensure_turso_agent_sessions_model_observation_columns(&connection).await?;
    dedupe_turso_agent_sessions_by_session_id(&connection).await?;
    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_asp_agent_sessions_project_root_name
            ON asp_agent_sessions(project_id, root_session_id, name)",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_asp_agent_sessions_session_id_unique
            ON asp_agent_sessions(session_id)",
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_root
            ON asp_agent_sessions(project_id, root_session_id)",
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_parent
            ON asp_agent_sessions(parent_session_id)",
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_message_target
            ON asp_agent_sessions(message_target_id)",
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_session
            ON asp_agent_sessions(project_id, session_id)",
    ] {
        execute_turso_statement(
            &connection,
            statement,
            "failed to initialize Turso session registry schema",
        )
        .await?;
    }
    Ok(())
}

async fn ensure_turso_agent_sessions_project_id_column(
    connection: &turso::Connection,
) -> Result<(), String> {
    if turso_agent_sessions_column_exists(connection, "project_id").await? {
        return Ok(());
    }
    execute_turso_statement(
        connection,
        "ALTER TABLE asp_agent_sessions ADD COLUMN project_id TEXT NOT NULL DEFAULT 'default'",
        "failed to migrate Turso session registry project_id",
    )
    .await?;
    Ok(())
}

async fn ensure_turso_agent_sessions_message_target_id_column(
    connection: &turso::Connection,
) -> Result<(), String> {
    if turso_agent_sessions_column_exists(connection, "message_target_id").await? {
        return Ok(());
    }
    execute_turso_statement(
        connection,
        "ALTER TABLE asp_agent_sessions ADD COLUMN message_target_id TEXT",
        "failed to migrate Turso session registry message_target_id",
    )
    .await?;
    Ok(())
}

async fn ensure_turso_agent_sessions_model_observation_columns(
    connection: &turso::Connection,
) -> Result<(), String> {
    const COLUMNS: [(&str, &str); 6] = [
        ("physical_generation", "INTEGER NOT NULL DEFAULT 1"),
        ("configured_agent_type", "TEXT"),
        ("profile_evidence_json", "TEXT"),
        ("model_observation_source", "TEXT"),
        ("model_observed_at", "INTEGER"),
        ("model_evidence_ref", "TEXT"),
    ];
    for (column, definition) in COLUMNS {
        if turso_agent_sessions_column_exists(connection, column).await? {
            continue;
        }
        let statement = format!("ALTER TABLE asp_agent_sessions ADD COLUMN {column} {definition}");
        execute_turso_statement(
            connection,
            &statement,
            "failed to migrate Turso session registry columns",
        )
        .await?;
    }
    Ok(())
}

async fn turso_agent_sessions_column_exists(
    connection: &turso::Connection,
    expected_column: &str,
) -> Result<bool, String> {
    let mut rows = run_turso_operation(
        || async {
            connection
                .query("PRAGMA table_info(asp_agent_sessions)", ())
                .await
                .map_err(|error| error.to_string())
        },
        "failed to inspect Turso session registry schema",
    )
    .await?;
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to inspect Turso session registry column: {error}"))?
    {
        let column_name = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso session registry column: {error}"))?;
        if column_name == expected_column {
            return Ok(true);
        }
    }
    Ok(false)
}
