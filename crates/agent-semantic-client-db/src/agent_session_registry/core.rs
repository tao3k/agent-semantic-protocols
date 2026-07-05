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
    turso_statement::{
        execute_turso_operation_with_lock_retry, execute_turso_statement_with_lock_retry,
        run_turso_operation_with_lock_retry,
    },
};

use super::bootstrap::dedupe_turso_agent_sessions_by_session_id;
use super::types::{
    AGENT_SESSION_REGISTRY_DB_NAME, AGENT_SESSION_STATUS_ACTIVE, AGENT_SESSION_STATUS_ARCHIVED,
    AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionToolEventRequest, agent_session_status_is_routable,
    agent_session_unix_timestamp,
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

impl AgentSessionRecord {
    /// Return whether this session can receive routed work at `now`.
    #[must_use]
    pub fn is_routable_at(&self, now: i64) -> bool {
        agent_session_status_is_routable(&self.status)
            && self.expires_at.is_none_or(|expires| expires > now)
    }
}

/// Turso-backed registry for agent session routing state.
pub struct AgentSessionRegistry {
    db_path: PathBuf,
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

    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn register_session(
        &self,
        request: AgentSessionRegisterRequest<'_>,
    ) -> Result<AgentSessionRecord, String> {
        block_on_agent_session_registry_async(turso_register_session(&self.db_path, request))
    }

    /// Return registered sessions for one project, optionally narrowed by root session and name.
    pub fn query_sessions(
        &self,
        project_id: &str,
        root_session_id: Option<&str>,
        name: Option<&str>,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_query_sessions(
            &self.db_path,
            project_id,
            root_session_id,
            name,
        ))
    }

    /// Return one registered session by its concrete session id.
    pub fn session_by_id(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_session_by_id(
            &self.db_path,
            project_id,
            session_id,
        ))
    }

    /// Return one registered session by its stable root/name route.
    pub fn session_by_name(
        &self,
        project_id: &str,
        root_session_id: &str,
        name: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_session_by_name(
            &self.db_path,
            project_id,
            root_session_id,
            name,
        ))
    }

    /// Record the latest tool event for one registered session.
    pub fn record_tool_event(
        &self,
        request: AgentSessionToolEventRequest<'_>,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_record_tool_event(&self.db_path, request))
    }

    /// Generic lookup used by registry CLI commands.
    pub fn lookup_session(
        &self,
        request: AgentSessionLookupRequest<'_>,
    ) -> Result<Option<AgentSessionRecord>, String> {
        if let Some(session_id) = request.session_id {
            return self.session_by_id(request.project_id, session_id);
        }
        if let (Some(root_session_id), Some(name)) = (request.root_session_id, request.name) {
            return self.session_by_name(request.project_id, root_session_id, name);
        }
        let sessions =
            self.query_sessions(request.project_id, request.root_session_id, request.name)?;
        Ok(sessions.into_iter().next())
    }

    /// Update one session row to the supplied routing status.
    pub fn update_session_status(
        &self,
        project_id: &str,
        session_id: &str,
        status: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_update_session_status(
            &self.db_path,
            project_id,
            session_id,
            status,
            now,
        ))
    }

    /// Mark one session row invalid.
    pub fn mark_session_invalid(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        self.update_session_status(project_id, session_id, AGENT_SESSION_STATUS_INVALID, now)
    }

    /// Archive one session row.
    pub fn archive_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_set_archived_status(
            &self.db_path,
            project_id,
            session_id,
            AGENT_SESSION_STATUS_ARCHIVED,
            Some(now),
            now,
        ))
    }

    /// Unarchive one session row.
    pub fn unarchive_session(
        &self,
        project_id: &str,
        session_id: &str,
        now: i64,
    ) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_set_archived_status(
            &self.db_path,
            project_id,
            session_id,
            AGENT_SESSION_STATUS_ACTIVE,
            None,
            now,
        ))
    }

    /// Delete one session row.
    pub fn delete_session(&self, project_id: &str, session_id: &str) -> Result<bool, String> {
        block_on_agent_session_registry_async(turso_delete_session(
            &self.db_path,
            project_id,
            session_id,
        ))
    }

    /// Refresh expired routable sessions in this registry DB.
    pub fn refresh_expired_sessions(&self) -> Result<(), String> {
        let now = agent_session_unix_timestamp()?;
        block_on_agent_session_registry_async(turso_refresh_expired_sessions(&self.db_path, now))
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

    /// Return one registered session by its concrete session id across all projects.
    pub fn session_by_id_any_project(
        &self,
        session_id: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        block_on_agent_session_registry_async(turso_session_by_id_any_project(
            &self.db_path,
            session_id,
        ))
    }

    /// Return the project id for the most recent session registered under one root.
    pub fn project_id_for_root_session_id(
        &self,
        root_session_id: &str,
    ) -> Result<Option<String>, String> {
        Ok(
            block_on_agent_session_registry_async(turso_session_for_root_session_id_any_project(
                &self.db_path,
                root_session_id,
            ))?
            .map(|record| record.project_id),
        )
    }
}

pub(super) fn block_on_agent_session_registry_async<T>(
    future: impl std::future::Future<Output = Result<T, String>>,
) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to build agent session Turso runtime: {error}"))?;
    runtime.block_on(future)
}

pub(super) async fn connect_turso_agent_session_registry(
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

async fn bootstrap_turso_agent_session_schema(db_path: &Path) -> Result<(), String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_statement_with_lock_retry(
        &connection,
        "CREATE TABLE IF NOT EXISTS asp_agent_sessions (
            project_id TEXT NOT NULL DEFAULT 'default',
            root_session_id TEXT NOT NULL,
            session_id TEXT NOT NULL UNIQUE,
            parent_session_id TEXT,
            name TEXT NOT NULL,
            role TEXT NOT NULL,
            model TEXT,
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
    ensure_turso_agent_sessions_project_id_column(&connection).await?;
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
        "CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_session
            ON asp_agent_sessions(project_id, session_id)",
    ] {
        execute_turso_statement_with_lock_retry(
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
    let mut rows = run_turso_operation_with_lock_retry(
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
        if column_name == "project_id" {
            return Ok(());
        }
    }
    execute_turso_statement_with_lock_retry(
        connection,
        "ALTER TABLE asp_agent_sessions ADD COLUMN project_id TEXT NOT NULL DEFAULT 'default'",
        "failed to migrate Turso session registry project_id",
    )
    .await?;
    Ok(())
}

async fn turso_register_session(
    db_path: &Path,
    request: AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    let mut last_lock_error = None;
    for attempt in 0..TURSO_CLIENT_DB_LOCK_RETRY_ATTEMPTS {
        match turso_register_session_once(db_path, &request).await {
            Ok(record) => return Ok(record),
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
        "failed to register Turso session after lock retries: {}",
        last_lock_error.unwrap_or_else(|| "unknown lock error".to_string())
    ))
}

async fn turso_register_session_once(
    db_path: &Path,
    request: &AgentSessionRegisterRequest<'_>,
) -> Result<AgentSessionRecord, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    execute_turso_operation_with_lock_retry(
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
    execute_turso_operation_with_lock_retry(
        || async {
            connection
                .execute(
                    "INSERT INTO asp_agent_sessions (
        project_id,
        root_session_id,
        session_id,
                parent_session_id,
                name,
                role,
                model,
                status,
                created_at,
                updated_at,
                last_seen_at,
                last_heartbeat_at,
                expires_at,
                metadata_json
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?9, ?9, ?10, ?11)
    ON CONFLICT DO UPDATE SET
        project_id = excluded.project_id,
        root_session_id = excluded.root_session_id,
        session_id = excluded.session_id,
                parent_session_id = excluded.parent_session_id,
                name = excluded.name,
                role = excluded.role,
                model = excluded.model,
                status = excluded.status,
                updated_at = excluded.updated_at,
                last_seen_at = excluded.last_seen_at,
                last_heartbeat_at = excluded.last_heartbeat_at,
                expires_at = excluded.expires_at,
                metadata_json = excluded.metadata_json",
                    (
                        request.project_id,
                        request.root_session_id,
                        request.session_id,
                        request.parent_session_id,
                        request.name,
                        request.role,
                        request.model,
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
    turso_session_by_name(
        db_path,
        request.project_id,
        request.root_session_id,
        request.name,
    )
    .await?
    .ok_or_else(|| "registered Turso session was not readable".to_string())
}

async fn turso_session_by_name(
    db_path: &Path,
    project_id: &str,
    root_session_id: &str,
    name: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    AGENT_SESSION_SELECT_ONE_BY_ROOT_AND_NAME,
                    (project_id, root_session_id, name),
                )
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
    turso_session_record_from_row(&row).map(Some)
}

async fn turso_query_sessions(
    db_path: &Path,
    project_id: &str,
    root_session_id: Option<&str>,
    name: Option<&str>,
) -> Result<Vec<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let sql = match (root_session_id, name) {
        (Some(_), Some(_)) => {
            "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE project_id = ?1 AND root_session_id = ?2 AND name = ?3
             ORDER BY updated_at DESC, session_id"
        }
        (Some(_), None) => {
            "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE project_id = ?1 AND root_session_id = ?2
             ORDER BY updated_at DESC, session_id"
        }
        (None, Some(_)) => {
            "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE project_id = ?1 AND name = ?2
             ORDER BY updated_at DESC, session_id"
        }
        (None, None) => {
            "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE project_id = ?1
             ORDER BY updated_at DESC, session_id"
        }
    };
    let mut rows = match (root_session_id, name) {
        (Some(root_session_id), Some(name)) => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(sql, (project_id, root_session_id, name))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (Some(root_session_id), None) => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(sql, (project_id, root_session_id))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (None, Some(name)) => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(sql, (project_id, name))
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to query Turso sessions",
            )
            .await?
        }
        (None, None) => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(sql, [project_id])
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
        records.push(turso_session_record_from_row(&row)?);
    }
    Ok(records)
}

async fn turso_session_by_id(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                     FROM asp_agent_sessions
                     WHERE project_id = ?1 AND session_id = ?2",
                    (project_id, session_id),
                )
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
    turso_session_record_from_row(&row).map(Some)
}

async fn turso_session_by_id_any_project(
    db_path: &Path,
    session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                     FROM asp_agent_sessions
                     WHERE session_id = ?1
                     ORDER BY updated_at DESC
                     LIMIT 1",
                    (session_id,),
                )
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
    turso_session_record_from_row(&row).map(Some)
}

async fn turso_session_for_root_session_id_any_project(
    db_path: &Path,
    root_session_id: &str,
) -> Result<Option<AgentSessionRecord>, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut rows = run_turso_operation_with_lock_retry(
        || async {
            connection
                .query(
                    "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                     FROM asp_agent_sessions
                     WHERE root_session_id = ?1
                     ORDER BY updated_at DESC
                     LIMIT 1",
                    (root_session_id,),
                )
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
    turso_session_record_from_row(&row).map(Some)
}

async fn turso_record_tool_event(
    db_path: &Path,
    request: AgentSessionToolEventRequest<'_>,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let updated = execute_turso_operation_with_lock_retry(
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

async fn turso_update_session_status(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation_with_lock_retry(
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

async fn turso_set_archived_status(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
    status: &str,
    archived_at: Option<i64>,
    now: i64,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation_with_lock_retry(
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

async fn turso_delete_session(
    db_path: &Path,
    project_id: &str,
    session_id: &str,
) -> Result<bool, String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let changes = execute_turso_operation_with_lock_retry(
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

async fn turso_refresh_expired_sessions(db_path: &Path, now: i64) -> Result<(), String> {
    let connection = connect_turso_agent_session_registry(db_path).await?;
    let mut expired_rows = run_turso_operation_with_lock_retry(
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

    execute_turso_operation_with_lock_retry(
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

fn turso_session_record_from_row(row: &turso::Row) -> Result<AgentSessionRecord, String> {
    Ok(AgentSessionRecord {
        project_id: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso project id: {error}"))?,
        root_session_id: row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso root session id: {error}"))?,
        session_id: row
            .get::<String>(2)
            .map_err(|error| format!("failed to read Turso session id: {error}"))?,
        parent_session_id: row
            .get::<Option<String>>(3)
            .map_err(|error| format!("failed to read Turso parent session id: {error}"))?,
        name: row
            .get::<String>(4)
            .map_err(|error| format!("failed to read Turso session name: {error}"))?,
        role: row
            .get::<String>(5)
            .map_err(|error| format!("failed to read Turso session role: {error}"))?,
        model: row
            .get::<Option<String>>(6)
            .map_err(|error| format!("failed to read Turso session model: {error}"))?,
        status: row
            .get::<String>(7)
            .map_err(|error| format!("failed to read Turso session status: {error}"))?,
        created_at: row
            .get::<i64>(8)
            .map_err(|error| format!("failed to read Turso session created_at: {error}"))?,
        updated_at: row
            .get::<i64>(9)
            .map_err(|error| format!("failed to read Turso session updated_at: {error}"))?,
        last_seen_at: row
            .get::<Option<i64>>(10)
            .map_err(|error| format!("failed to read Turso session last_seen_at: {error}"))?,
        last_heartbeat_at: row
            .get::<Option<i64>>(11)
            .map_err(|error| format!("failed to read Turso session last_heartbeat_at: {error}"))?,
        expires_at: row
            .get::<Option<i64>>(12)
            .map_err(|error| format!("failed to read Turso session expires_at: {error}"))?,
        archived_at: row
            .get::<Option<i64>>(13)
            .map_err(|error| format!("failed to read Turso session archived_at: {error}"))?,
        last_tool_event: row
            .get::<Option<String>>(14)
            .map_err(|error| format!("failed to read Turso session last_tool_event: {error}"))?,
        last_command: row
            .get::<Option<String>>(15)
            .map_err(|error| format!("failed to read Turso session last_command: {error}"))?,
        last_evidence_ref: row
            .get::<Option<String>>(16)
            .map_err(|error| format!("failed to read Turso session last_evidence_ref: {error}"))?,
        metadata_json: row
            .get::<String>(17)
            .map_err(|error| format!("failed to read Turso session metadata_json: {error}"))?,
    })
}

const AGENT_SESSION_SELECT_ONE_BY_ROOT_AND_NAME: &str = "SELECT project_id, root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
FROM asp_agent_sessions
WHERE project_id = ?1 AND root_session_id = ?2 AND name = ?3";
