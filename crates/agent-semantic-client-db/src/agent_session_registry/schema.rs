//! Runtime, connection, and schema bootstrap for the agent-session registry.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::engine::{
    turso_lock_policy::{
        TURSO_CLIENT_DB_BUSY_TIMEOUT_MS, TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS, is_turso_lock_error,
        turso_lock_retry_delay,
    },
    turso_statement::execute_turso_statement,
};

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

pub(in crate::agent_session_registry) async fn bootstrap_turso_agent_session_schema(
    db_path: &Path,
) -> Result<(), String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    create_turso_agent_sessions_table(&connection, "asp_agent_sessions").await?;
    validate_turso_agent_sessions_instance_identity(&connection).await?;
    bootstrap_turso_agent_session_retirement_schema(&connection).await?;
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
    execute_turso_statement(
        &connection,
        "DROP INDEX IF EXISTS idx_asp_agent_sessions_project_root_name",
        "failed to retire singleton resident-route session index",
    )
    .await?;
    for statement in [
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_project_root_name
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

async fn bootstrap_turso_agent_session_retirement_schema(
    connection: &turso::Connection,
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

const AGENT_SESSION_COLUMNS: [&str; 25] = [
    "project_id",
    "root_session_id",
    "session_id",
    "physical_generation",
    "configured_agent_type",
    "profile_evidence_json",
    "message_target_id",
    "parent_session_id",
    "name",
    "role",
    "model",
    "model_observation_source",
    "model_observed_at",
    "model_evidence_ref",
    "status",
    "created_at",
    "updated_at",
    "last_seen_at",
    "last_heartbeat_at",
    "expires_at",
    "archived_at",
    "last_tool_event",
    "last_command",
    "last_evidence_ref",
    "metadata_json",
];

async fn create_turso_agent_sessions_table(
    connection: &turso::Connection,
    table_name: &str,
) -> Result<(), String> {
    let statement = format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (
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
            metadata_json TEXT NOT NULL DEFAULT '{{}}',
            PRIMARY KEY(project_id, session_id)
        )"
    );
    execute_turso_statement(
        connection,
        &statement,
        "failed to initialize Turso session registry schema",
    )
    .await
}

async fn validate_turso_agent_sessions_instance_identity(
    connection: &turso::Connection,
) -> Result<(), String> {
    let mut rows = connection
        .query("PRAGMA table_info(asp_agent_sessions)", ())
        .await
        .map_err(|error| format!("failed to inspect Turso session identity schema: {error}"))?;
    let mut columns = BTreeSet::new();
    let mut primary_key = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to inspect Turso session identity column: {error}"))?
    {
        let column = row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso session identity column: {error}"))?;
        let position = row.get::<i64>(5).map_err(|error| {
            format!("failed to read Turso session identity primary key: {error}")
        })?;
        columns.insert(column.clone());
        if position > 0 {
            primary_key.push((position, column));
        }
    }
    primary_key.sort_by_key(|(position, _)| *position);
    let primary_key = primary_key
        .into_iter()
        .map(|(_, column)| column)
        .collect::<Vec<_>>();
    let expected_columns = AGENT_SESSION_COLUMNS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if columns != expected_columns || primary_key != ["project_id", "session_id"] {
        return Err(format!(
            "agent-session-v1-instance-schema-not-current: expectedColumns={expected_columns:?} observedColumns={columns:?} expectedPrimaryKey=[project_id, session_id] observedPrimaryKey={primary_key:?}"
        ));
    }
    Ok(())
}
