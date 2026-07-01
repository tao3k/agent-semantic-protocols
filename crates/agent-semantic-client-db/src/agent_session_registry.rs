//! DB-owned storage for agent session registry rows.

use agent_semantic_client_core::state_core::ResolvedState;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

pub const AGENT_SESSION_REGISTRY_DB_NAME: &str = "session-registry.sqlite3";

#[derive(Clone, Debug, Serialize)]
pub struct AgentSessionRecord {
    #[serde(rename = "rootSessionId")]
    pub root_session_id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "parentSessionId", skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    pub name: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(rename = "lastSeenAt", skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<i64>,
    #[serde(rename = "lastHeartbeatAt", skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at: Option<i64>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(rename = "archivedAt", skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
    #[serde(rename = "lastToolEvent", skip_serializing_if = "Option::is_none")]
    pub last_tool_event: Option<String>,
    #[serde(rename = "lastCommand", skip_serializing_if = "Option::is_none")]
    pub last_command: Option<String>,
    #[serde(rename = "lastEvidenceRef", skip_serializing_if = "Option::is_none")]
    pub last_evidence_ref: Option<String>,
    #[serde(rename = "metadataJson")]
    pub metadata_json: String,
}

impl AgentSessionRecord {
    #[must_use]
    pub fn is_routable_at(&self, now: i64) -> bool {
        agent_session_status_is_routable(&self.status)
            && self.expires_at.is_none_or(|expires| expires > now)
    }
}

#[must_use]
pub fn agent_session_status_is_routable(status: &str) -> bool {
    matches!(status, "active" | "idle")
}

pub fn agent_session_unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before unix epoch: {error}"))?;
    i64::try_from(duration.as_secs()).map_err(|error| format!("timestamp overflow: {error}"))
}

pub struct AgentSessionRegisterRequest<'a> {
    pub root_session_id: &'a str,
    pub session_id: &'a str,
    pub parent_session_id: Option<&'a str>,
    pub name: &'a str,
    pub role: &'a str,
    pub model: Option<&'a str>,
    pub status: &'a str,
    pub expires_at: Option<i64>,
    pub metadata_json: &'a str,
    pub now: i64,
}

pub struct AgentSessionToolEventRequest<'a> {
    pub session_id: &'a str,
    pub tool_event: &'a str,
    pub command: Option<&'a str>,
    pub evidence_ref: Option<&'a str>,
    pub now: i64,
}

pub struct AgentSessionRegistry {
    db_path: PathBuf,
    conn: Connection,
}

impl AgentSessionRegistry {
    #[must_use]
    pub fn state_root_for_resolved_state(state: &ResolvedState) -> PathBuf {
        state.paths.client_dir.join("agent")
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
        let state_root = Self::state_root_for_project(project_root)?;
        Self::open_existing_state_root(state_root)
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
        self.conn
            .execute(
                "INSERT INTO asp_agent_sessions (
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
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?8, ?8, ?9, ?10)
                ON CONFLICT(root_session_id, name) DO UPDATE SET
                    session_id = excluded.session_id,
                    parent_session_id = excluded.parent_session_id,
                    role = excluded.role,
                    model = excluded.model,
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    last_seen_at = excluded.last_seen_at,
                    last_heartbeat_at = excluded.last_heartbeat_at,
                    expires_at = excluded.expires_at,
                    metadata_json = excluded.metadata_json",
                params![
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
                ],
            )
            .map_err(|error| format!("failed to register session: {error}"))?;

        self.session_by_name(request.root_session_id, request.name)?
            .ok_or_else(|| "registered session was not readable".to_string())
    }

    pub fn query_sessions(
        &self,
        root_session_id: Option<&str>,
        name: Option<&str>,
    ) -> Result<Vec<AgentSessionRecord>, String> {
        match (root_session_id, name) {
            (Some(root_session_id), Some(name)) => query_session_rows(
                &self.conn,
                "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                 FROM asp_agent_sessions
                 WHERE root_session_id = ?1 AND name = ?2
                 ORDER BY updated_at DESC, name ASC",
                params![root_session_id, name],
            ),
            (Some(root_session_id), None) => query_session_rows(
                &self.conn,
                "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                 FROM asp_agent_sessions
                 WHERE root_session_id = ?1
                 ORDER BY updated_at DESC, name ASC",
                params![root_session_id],
            ),
            (None, Some(name)) => query_session_rows(
                &self.conn,
                "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                 FROM asp_agent_sessions
                 WHERE name = ?1
                 ORDER BY updated_at DESC, root_session_id ASC",
                params![name],
            ),
            (None, None) => query_session_rows(
                &self.conn,
                "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                 FROM asp_agent_sessions
                 ORDER BY updated_at DESC, root_session_id ASC, name ASC",
                params![],
            ),
        }
    }

    pub fn session_by_name(
        &self,
        root_session_id: &str,
        name: &str,
    ) -> Result<Option<AgentSessionRecord>, String> {
        self.conn
            .query_row(
                "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                 FROM asp_agent_sessions
                 WHERE root_session_id = ?1 AND name = ?2",
                params![root_session_id, name],
                session_record_from_row,
            )
            .optional()
            .map_err(|error| format!("failed to read session by name: {error}"))
    }

    pub fn session_by_id(&self, session_id: &str) -> Result<Option<AgentSessionRecord>, String> {
        self.conn
            .query_row(
                "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
                 FROM asp_agent_sessions
                 WHERE session_id = ?1",
                params![session_id],
                session_record_from_row,
            )
            .optional()
            .map_err(|error| format!("failed to read session by id: {error}"))
    }

    pub fn record_tool_event(
        &self,
        request: AgentSessionToolEventRequest<'_>,
    ) -> Result<bool, String> {
        let rows = self
            .conn
            .execute(
                "UPDATE asp_agent_sessions
                 SET updated_at = ?2,
                     last_seen_at = ?2,
                     last_heartbeat_at = ?2,
                     last_tool_event = ?3,
                     last_command = COALESCE(?4, last_command),
                     last_evidence_ref = COALESCE(?5, last_evidence_ref)
                 WHERE session_id = ?1
                   AND status IN ('active', 'idle')",
                params![
                    request.session_id,
                    request.now,
                    request.tool_event,
                    request.command,
                    request.evidence_ref,
                ],
            )
            .map_err(|error| format!("failed to record session tool event: {error}"))?;
        Ok(rows > 0)
    }

    pub fn refresh_expired_sessions(&self) -> Result<(), String> {
        let now = agent_session_unix_timestamp()?;
        self.conn
            .execute(
                "UPDATE asp_agent_sessions
                 SET status = 'expired', updated_at = ?1
                 WHERE expires_at IS NOT NULL
                   AND expires_at <= ?1
                   AND status IN ('active', 'idle')",
                params![now],
            )
            .map_err(|error| format!("failed to refresh expired session rows: {error}"))?;
        Ok(())
    }

    fn open_path(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|error| {
            format!(
                "failed to open agent session registry `{}`: {error}",
                db_path.display()
            )
        })?;
        Ok(Self {
            db_path: db_path.to_path_buf(),
            conn,
        })
    }

    fn ensure_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS asp_agent_sessions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
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
                    UNIQUE(root_session_id, name)
                );
                CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_root
                    ON asp_agent_sessions(root_session_id);
                CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_parent
                    ON asp_agent_sessions(parent_session_id);",
            )
            .map_err(|error| format!("failed to initialize session registry schema: {error}"))?;
        self.ensure_session_column("last_seen_at", "INTEGER")?;
        self.ensure_session_column("last_heartbeat_at", "INTEGER")?;
        self.ensure_session_column("expires_at", "INTEGER")?;
        self.ensure_session_column("archived_at", "INTEGER")?;
        self.ensure_session_column("last_tool_event", "TEXT")?;
        self.ensure_session_column("last_command", "TEXT")?;
        self.ensure_session_column("last_evidence_ref", "TEXT")?;
        Ok(())
    }

    fn ensure_session_column(&self, name: &str, sql_type: &str) -> Result<(), String> {
        let mut stmt = self
            .conn
            .prepare("PRAGMA table_info(asp_agent_sessions)")
            .map_err(|error| format!("failed to inspect session registry schema: {error}"))?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| format!("failed to inspect session registry columns: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read session registry columns: {error}"))?;
        if columns.iter().any(|column| column == name) {
            return Ok(());
        }
        self.conn
            .execute(
                &format!("ALTER TABLE asp_agent_sessions ADD COLUMN {name} {sql_type}"),
                [],
            )
            .map_err(|error| format!("failed to add session registry column `{name}`: {error}"))?;
        Ok(())
    }
}

fn query_session_rows<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<AgentSessionRecord>, String> {
    let mut statement = conn
        .prepare(sql)
        .map_err(|error| format!("failed to prepare session query: {error}"))?;
    let rows = statement
        .query_map(params, session_record_from_row)
        .map_err(|error| format!("failed to query sessions: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read session row: {error}"))
}

fn session_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentSessionRecord> {
    Ok(AgentSessionRecord {
        root_session_id: row.get(0)?,
        session_id: row.get(1)?,
        parent_session_id: row.get(2)?,
        name: row.get(3)?,
        role: row.get(4)?,
        model: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        last_seen_at: row.get(9)?,
        last_heartbeat_at: row.get(10)?,
        expires_at: row.get(11)?,
        archived_at: row.get(12)?,
        last_tool_event: row.get(13)?,
        last_command: row.get(14)?,
        last_evidence_ref: row.get(15)?,
        metadata_json: row.get(16)?,
    })
}
