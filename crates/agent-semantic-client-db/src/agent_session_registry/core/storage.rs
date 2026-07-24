//! DB-owned storage for agent session registry rows.

use agent_semantic_client_core::state_core::ResolvedState;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::engine::{
    turso_lock_policy::{
        TURSO_CLIENT_DB_BUSY_TIMEOUT_MS, TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS, is_turso_lock_error,
        turso_lock_retry_delay,
    },
    turso_statement::{execute_turso_operation, execute_turso_statement, run_turso_operation},
};

use super::bootstrap::dedupe_turso_agent_sessions_by_session_id;
use super::types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AgentSessionRecord, AgentSessionRegisterRequest,
    AgentSessionToolEventRequest,
};

const AGENT_SESSION_EXPIRED_REFRESH_LOCK_STALE_AFTER: Duration = Duration::from_secs(60);

struct ExpiredRefreshLock {
    path: PathBuf,
}

impl Drop for ExpiredRefreshLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn expired_refresh_lock_path(db_path: &Path) -> PathBuf {
    db_path.with_extension("expired-refresh.lock")
}

fn try_acquire_expired_refresh_lock(db_path: &Path) -> Option<ExpiredRefreshLock> {
    let path = expired_refresh_lock_path(db_path);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(_) => Some(ExpiredRefreshLock { path }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let is_stale = fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|elapsed| elapsed > AGENT_SESSION_EXPIRED_REFRESH_LOCK_STALE_AFTER);
            if !is_stale {
                return None;
            }
            let _ = fs::remove_file(&path);
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .ok()
                .map(|_| ExpiredRefreshLock { path })
        }
        Err(_) => None,
    }
}

/// Turso-backed registry for agent session routing state.
pub struct AgentSessionRegistry {
    pub(super) db_path: PathBuf,
}

impl AgentSessionRegistry {
    /// Return the canonical project-scope id used by legacy registry callers.
    pub fn project_scope_id(project_root: impl AsRef<Path>) -> String {
        fs::canonicalize(project_root.as_ref())
            .unwrap_or_else(|_| project_root.as_ref().to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    /// Return the current working directory as a canonical project-scope id.
    pub fn current_project_scope_id() -> Result<String, String> {
        let project_root = std::env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?;
        Ok(Self::project_scope_id(project_root))
    }

    /// Resolve a configured registry state root against the project root.
    pub fn resolve_state_root_override(
        project_root: impl AsRef<Path>,
        state_root: impl AsRef<Path>,
    ) -> PathBuf {
        if state_root.as_ref().is_absolute() {
            state_root.as_ref().to_path_buf()
        } else {
            project_root.as_ref().join(state_root.as_ref())
        }
    }

    #[must_use]
    pub fn state_root_for_resolved_state(state: &ResolvedState) -> PathBuf {
        state.state_home.clone()
    }

    pub fn state_root_for_project(project_root: impl AsRef<Path>) -> Result<PathBuf, String> {
        let state = ResolvedState::resolve(project_root.as_ref())?;
        Ok(Self::state_root_for_resolved_state(&state))
    }

    #[must_use]
    pub fn db_path_for_state_root(state_root: impl AsRef<Path>) -> PathBuf {
        state_root.as_ref().join(AGENT_SESSION_REGISTRY_DB_NAME)
    }

    pub fn open_or_create_project(project_root: impl AsRef<Path>) -> Result<Self, String> {
        let state = ResolvedState::resolve(project_root.as_ref())?;
        state.ensure_minimal_layout()?;
        Self::open_or_create_state_root(Self::state_root_for_resolved_state(&state))
    }

    pub fn open_existing_project_read_only(
        project_root: impl AsRef<Path>,
    ) -> Result<Option<Self>, String> {
        let state = ResolvedState::resolve(project_root.as_ref())?;
        Self::open_existing_state_root_read_only(Self::state_root_for_resolved_state(&state))
    }

    pub fn open_existing_project(project_root: impl AsRef<Path>) -> Result<Option<Self>, String> {
        let state = ResolvedState::resolve(project_root.as_ref())?;
        Self::open_existing_state_root(Self::state_root_for_resolved_state(&state))
    }

    pub fn open_or_create_state_root(state_root: impl AsRef<Path>) -> Result<Self, String> {
        fs::create_dir_all(state_root.as_ref()).map_err(|error| {
            format!(
                "failed to create agent session state root `{}`: {error}",
                state_root.as_ref().display()
            )
        })?;
        let db_path = Self::db_path_for_state_root(state_root);
        let registry = Self::open_path(&db_path)?;
        registry.ensure_schema()?;
        Ok(registry)
    }

    pub fn open_existing_state_root(state_root: impl AsRef<Path>) -> Result<Option<Self>, String> {
        let db_path = Self::db_path_for_state_root(state_root);
        if !db_path.is_file() {
            return Ok(None);
        }
        let registry = Self::open_path(&db_path)?;
        registry.ensure_schema()?;
        registry.refresh_expired_sessions()?;
        Ok(Some(registry))
    }

    pub fn open_existing_state_root_read_only(
        state_root: impl AsRef<Path>,
    ) -> Result<Option<Self>, String> {
        let db_path = Self::db_path_for_state_root(state_root);
        if !db_path.is_file() {
            return Ok(None);
        }
        Ok(Some(Self { db_path }))
    }

    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn open_path(db_path: &Path) -> Result<Self, String> {
        let registry = Self {
            db_path: db_path.to_path_buf(),
        };
        registry.ensure_schema()?;
        Ok(registry)
    }

    fn ensure_schema(&self) -> Result<(), String> {
        block_on_agent_session_registry_async(bootstrap_turso_agent_session_schema(&self.db_path))
    }
}

pub(in crate::agent_session_registry) fn block_on_agent_session_registry_async<T>(
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build agent session Turso runtime: {error}"))?;
    runtime.block_on(future)
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
    let database = turso::Builder::new_local(db_path.to_string_lossy().as_ref())
        .experimental_index_method(true)
        .experimental_multiprocess_wal(true)
        .build()
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

async fn bootstrap_turso_agent_session_schema(db_path: &Path) -> Result<(), String> {
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
    super::dispatch::bootstrap_turso_agent_dispatch_schema(&connection).await?;
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

pub(super) async fn turso_register_session(
    db_path: &Path,
    request: AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    turso_register_session_once(db_path, &request).await
}

pub(super) async fn turso_claim_resident_session(
    db_path: &Path,
    request: AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_agent_sessions
                     WHERE project_id = ?1
                       AND root_session_id = ?2
                       AND name = ?3
                       AND status IN ('archived', 'closed')",
                    (request.project_id, request.root_session_id, request.name),
                )
                .await
                .map_err(|error| error.to_string())?;

            connection
                .execute(
                    "INSERT INTO asp_agent_sessions (
        project_id,
        root_session_id,
        session_id,
        message_target_id,
        parent_session_id,
        name,
        role,
        model,
        model_observation_source,
        model_observed_at,
        model_evidence_ref,
        status,
        created_at,
        updated_at,
        last_seen_at,
        last_heartbeat_at,
        expires_at,
        metadata_json
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, ?13, ?13, ?14, ?15)
    ON CONFLICT DO NOTHING",
                    (
                        request.project_id,
                        request.root_session_id,
                        request.session_id,
                        request.message_target_id,
                        request.parent_session_id,
                        request.name,
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
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to claim Turso resident session",
    )
    .await?;
    turso_session_by_name(
        db_path,
        request.project_id,
        request.root_session_id,
        request.name,
    )
    .await?
    .ok_or_else(|| "claimed Turso resident session was not readable".to_string())
}

async fn turso_register_session_once(
    db_path: &Path,
    request: &AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
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
        "failed to clear stale Turso session mapping",
    )
    .await?;
    execute_turso_operation(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_agent_sessions (
        project_id,
        root_session_id,
        session_id,
                message_target_id,
                parent_session_id,
                name,
                role,
                model,
                model_observation_source,
                model_observed_at,
                model_evidence_ref,
                status,
                created_at,
                updated_at,
                last_seen_at,
                last_heartbeat_at,
                expires_at,
                metadata_json,
                configured_agent_type,
                profile_evidence_json
    ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, ?13, ?13, ?14, ?15,
        CASE WHEN json_valid(?15) AND json_extract(?15, '$.event') = 'subagent-start' AND json_extract(?15, '$.native') = 1 THEN json_extract(?15, '$.agentType') END,
        CASE WHEN json_valid(?15) AND json_extract(?15, '$.event') = 'subagent-start' AND json_extract(?15, '$.native') = 1 THEN ?15 END
    )
    ON CONFLICT(project_id, root_session_id, name) DO UPDATE SET
        project_id = excluded.project_id,
        root_session_id = excluded.root_session_id,
        session_id = excluded.session_id,
                message_target_id = excluded.message_target_id,
                parent_session_id = excluded.parent_session_id,
                name = excluded.name,
                role = excluded.role,
                model = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model ELSE asp_agent_sessions.model END,
                model_observation_source = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model_observation_source ELSE asp_agent_sessions.model_observation_source END,
                model_observed_at = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model_observed_at ELSE asp_agent_sessions.model_observed_at END,
                model_evidence_ref = CASE
                    WHEN excluded.model IS NOT NULL
                     AND (asp_agent_sessions.model_observed_at IS NULL
                          OR excluded.model_observed_at >= asp_agent_sessions.model_observed_at)
                    THEN excluded.model_evidence_ref ELSE asp_agent_sessions.model_evidence_ref END,
                status = excluded.status,
                updated_at = excluded.updated_at,
                last_seen_at = excluded.last_seen_at,
                last_heartbeat_at = excluded.last_heartbeat_at,
                expires_at = excluded.expires_at,
                metadata_json = excluded.metadata_json,
                configured_agent_type = COALESCE(excluded.configured_agent_type, asp_agent_sessions.configured_agent_type),
                profile_evidence_json = COALESCE(excluded.profile_evidence_json, asp_agent_sessions.profile_evidence_json)
        WHERE asp_agent_sessions.session_id = excluded.session_id",
                    (
                        request.project_id,
                        request.root_session_id,
                        request.session_id,
                        request.message_target_id,
                        request.parent_session_id,
                        request.name,
                        request.role,
                        request.model_observation.as_ref().map(|observation| observation.model),
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
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to register Turso session",
    )
    .await?;
    let registered = turso_session_by_name(
        db_path,
        request.project_id,
        request.root_session_id,
        request.name,
    )
    .await?
    .ok_or_else(|| "registered Turso session was not readable".to_string())?;
    if registered.session_id() != request.session_id {
        return Err(format!(
            "resident slot is owned by generation {} (child {}); replacement requires exact compare-and-swap",
            registered.physical_generation,
            registered.session_id()
        ));
    }
    Ok(registered)
}

pub(in crate::agent_session_registry) async fn turso_session_by_name(
    db_path: &Path,
    project_id: &str,
    root_session_id: &str,
    name: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql =
        super::record::select_sql("WHERE project_id = ?1 AND root_session_id = ?2 AND name = ?3");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (project_id, root_session_id, name))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by name",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso session by name row: {error}"))?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_query_sessions(
    db_path: &Path,
    project_id: &str,
    root_session_id: Option<&str>,
    name: Option<&str>,
) -> Result<Vec<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = match (root_session_id, name) {
        (Some(_), Some(_)) => super::record::select_sql(
            "WHERE project_id = ?1 AND root_session_id = ?2 AND name = ?3 ORDER BY updated_at DESC, session_id",
        ),
        (Some(_), None) => super::record::select_sql(
            "WHERE project_id = ?1 AND root_session_id = ?2 ORDER BY updated_at DESC, session_id",
        ),
        (None, Some(_)) => super::record::select_sql(
            "WHERE project_id = ?1 AND name = ?2 ORDER BY updated_at DESC, session_id",
        ),
        (None, None) => {
            super::record::select_sql("WHERE project_id = ?1 ORDER BY updated_at DESC, session_id")
        }
    };
    let mut rows = match (root_session_id, name) {
        (Some(root_session_id), Some(name)) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, (project_id, root_session_id, name))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (Some(root_session_id), None) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, (project_id, root_session_id))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (None, Some(name)) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, (project_id, name))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (None, None) => {
            run_turso_operation(
                || async {
                    connection
                        .query(&sql, [project_id])
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
    };
    let mut records = Vec::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso session row: {error}"))?
    {
        records.push(super::record::from_turso_row(&row)?);
    }
    Ok(records)
}

pub(super) async fn turso_session_by_id(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = super::record::select_sql("WHERE project_id = ?1 AND session_id = ?2");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (project_id, session_id))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by id",
    )
    .await?;
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso session by id row: {error}"))?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_session_by_id_any_project(
    db_path: &Path,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = super::record::select_sql("WHERE session_id = ?1 ORDER BY updated_at DESC LIMIT 1");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (session_id,))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by id across projects",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso session by id across projects row: {error}")
    })?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_session_for_root_session_id_any_project(
    db_path: &Path,
    root_session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql =
        super::record::select_sql("WHERE root_session_id = ?1 ORDER BY updated_at DESC LIMIT 1");
    let mut rows = run_turso_operation(
        || async {
            connection
                .query(&sql, (root_session_id,))
                .await
                .map_err(|error| error.to_string())
        },
        "failed to read Turso session by root session id across projects",
    )
    .await?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso session by root session id across projects row: {error}")
    })?
    else {
        return Ok(None);
    };
    super::record::from_turso_row(&row).map(Some)
}

pub(super) async fn turso_record_tool_event(
    db_path: &Path,
    request: AgentSessionToolEventRequest<'_>,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let updated = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET last_tool_event = ?1,
                         last_command = ?2,
                         last_evidence_ref = ?3,
                         updated_at = ?4,
                         last_seen_at = ?4
                     WHERE session_id = ?5",
                    (
                        request.tool_event,
                        request.command,
                        request.evidence_ref,
                        request.now,
                        request.session_id,
                    ),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to record Turso session tool event",
    )
    .await?;
    Ok(updated > 0)
}

pub(super) async fn turso_update_session_status(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET status = ?1,
                         updated_at = ?2,
                         last_seen_at = ?2
                     WHERE project_id = ?3 AND session_id = ?4",
                    (status, now, project_id, session_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to update Turso session status",
    )
    .await?;
    Ok(changes > 0)
}

pub(super) async fn turso_set_archived_status(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    archived_at: Option<i64>,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
                     SET status = ?1,
                         archived_at = ?2,
                         updated_at = ?3,
                         last_seen_at = ?3
                     WHERE project_id = ?4 AND session_id = ?5",
                    (status, archived_at, now, project_id, session_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to update Turso session archive status",
    )
    .await?;
    Ok(changes > 0)
}

pub(super) async fn turso_delete_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation(
        || async {
            connection
                .execute(
                    "DELETE FROM asp_agent_sessions WHERE project_id = ?1 AND session_id = ?2",
                    (project_id, session_id),
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to delete Turso session row",
    )
    .await?;
    Ok(changes > 0)
}

pub(super) async fn turso_refresh_expired_sessions(db_path: &Path, now: i64) -> Result<(), String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut expired_rows = run_turso_operation(
        || async {
            connection
                .query(
                    "SELECT 1
             FROM asp_agent_sessions
             WHERE expires_at IS NOT NULL
               AND expires_at <= ?1
               AND status IN ('active', 'idle')
             LIMIT 1",
                    [now],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to check Turso expired session rows",
    )
    .await?;
    let has_expired_rows = expired_rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso expired session row: {error}"))?
        .is_some();
    drop(expired_rows);
    if !has_expired_rows {
        return Ok(());
    }

    let Some(_refresh_lock) = try_acquire_expired_refresh_lock(db_path) else {
        return Ok(());
    };

    execute_turso_operation(
        || async {
            connection
                .execute(
                    "UPDATE asp_agent_sessions
             SET status = 'expired', updated_at = ?1
             WHERE expires_at IS NOT NULL
               AND expires_at <= ?1
               AND status IN ('active', 'idle')",
                    [now],
                )
                .await
                .map_err(|error| error.to_string())
        },
        "failed to refresh Turso expired session rows",
    )
    .await?;
    Ok(())
}
