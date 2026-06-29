//! Durable agent session and subagent registry.

#[path = "agent_session_registry_args.rs"]
mod agent_session_registry_args;
#[path = "agent_session_registry_render.rs"]
mod agent_session_registry_render;
#[path = "agent_session_registry_tool_event.rs"]
mod agent_session_registry_tool_event;

use agent_session_registry_args::{
    SessionArgs, SessionCommand, agent_usage, session_guide, session_usage,
};
use agent_session_registry_render::{
    escape_field, print_json_report, print_reuse_miss, print_reuse_session, print_session_row,
};
pub(crate) use agent_session_registry_tool_event::record_current_session_tool_event;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const REGISTRY_DB_NAME: &str = "session-registry.sqlite3";

pub(crate) fn run_agent_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("session") => run_agent_session_command(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            println!("{}", agent_usage());
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown agent command `{command}`\n{}",
            agent_usage()
        )),
    }
}

pub(crate) fn registered_root_session_id(
    project_root: &Path,
    session_id: &str,
) -> Result<Option<String>, String> {
    let db_path = agent_state_root_for_project(project_root).join(REGISTRY_DB_NAME);
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = Connection::open(&db_path).map_err(|error| {
        format!(
            "failed to open agent session registry `{}`: {error}",
            db_path.display()
        )
    })?;
    Ok(session_by_id(&conn, session_id)?.map(|record| record.root_session_id))
}

pub(crate) fn run_agent_session_command(args: &[String]) -> Result<(), String> {
    let args = SessionArgs::parse(args)?;
    if args.help {
        println!("{}", session_usage());
        return Ok(());
    }
    if args.guide {
        println!("{}", session_guide(args.command)?);
        return Ok(());
    }

    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_root = resolve_state_root(&project_root, args.state_root.as_deref());
    fs::create_dir_all(&state_root).map_err(|error| {
        format!(
            "failed to create agent session state root `{}`: {error}",
            state_root.display()
        )
    })?;
    let db_path = state_root.join(REGISTRY_DB_NAME);
    let conn = Connection::open(&db_path).map_err(|error| {
        format!(
            "failed to open agent session registry `{}`: {error}",
            db_path.display()
        )
    })?;
    ensure_schema(&conn)?;

    match args.command {
        SessionCommand::Register => register_session(&conn, &db_path, &args),
        SessionCommand::List => list_sessions(&conn, &db_path, &args),
        SessionCommand::Show => show_session(&conn, &db_path, &args),
        SessionCommand::Reuse => reuse_session(&conn, &db_path, &args),
    }
}

#[derive(Serialize)]
struct SessionRecord {
    #[serde(rename = "rootSessionId")]
    root_session_id: String,
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "parentSessionId", skip_serializing_if = "Option::is_none")]
    parent_session_id: Option<String>,
    name: String,
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    status: String,
    #[serde(rename = "createdAt")]
    created_at: i64,
    #[serde(rename = "updatedAt")]
    updated_at: i64,
    #[serde(rename = "lastSeenAt", skip_serializing_if = "Option::is_none")]
    last_seen_at: Option<i64>,
    #[serde(rename = "lastHeartbeatAt", skip_serializing_if = "Option::is_none")]
    last_heartbeat_at: Option<i64>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
    #[serde(rename = "archivedAt", skip_serializing_if = "Option::is_none")]
    archived_at: Option<i64>,
    #[serde(rename = "lastToolEvent", skip_serializing_if = "Option::is_none")]
    last_tool_event: Option<String>,
    #[serde(rename = "lastCommand", skip_serializing_if = "Option::is_none")]
    last_command: Option<String>,
    #[serde(rename = "lastEvidenceRef", skip_serializing_if = "Option::is_none")]
    last_evidence_ref: Option<String>,
    #[serde(rename = "metadataJson")]
    metadata_json: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RegisteredSession {
    pub(crate) root_session_id: String,
    pub(crate) session_id: String,
    pub(crate) name: String,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) expires_at: Option<i64>,
}

impl RegisteredSession {
    fn from_record(record: SessionRecord) -> Self {
        Self {
            root_session_id: record.root_session_id,
            session_id: record.session_id,
            name: record.name,
            role: record.role,
            status: record.status,
            expires_at: record.expires_at,
        }
    }

    pub(crate) fn is_routable_at(&self, now: i64) -> bool {
        session_status_is_routable(&self.status)
            && self.expires_at.map_or(true, |expires| expires > now)
    }
}

impl SessionRecord {
    fn is_routable_at(&self, now: i64) -> bool {
        session_status_is_routable(&self.status)
            && self.expires_at.map_or(true, |expires| expires > now)
    }
}

fn session_status_is_routable(status: &str) -> bool {
    matches!(status, "active" | "idle")
}

fn register_session(conn: &Connection, db_path: &Path, args: &SessionArgs) -> Result<(), String> {
    let platform_session = super::agent_session::current_agent_session();
    let session_id = args
        .child_session_id
        .clone()
        .or_else(|| platform_session.as_ref().map(|session| session.id.clone()))
        .ok_or_else(|| {
            "asp agent session register requires --child-session-id or an agent session env"
                .to_string()
        })?;
    let root_session_id = args
        .root_session_id
        .clone()
        .or_else(|| {
            platform_session
                .as_ref()
                .map(|session| session.recall_session_id().to_string())
        })
        .unwrap_or_else(|| session_id.clone());
    let name = required_non_empty(args.name.as_deref(), "--name")?.to_string();
    let role = args.role.as_deref().unwrap_or("agent").to_string();
    let status = args.status.as_deref().unwrap_or("active").to_string();
    let metadata_json = normalized_metadata(args.metadata_json.as_deref())?;
    let now = unix_timestamp()?;
    if !args.replace
        && let Some(existing) = session_by_name(conn, &root_session_id, &name)?
        && existing.session_id != session_id
        && existing.is_routable_at(now)
    {
        return print_reuse_session(db_path, Some(&root_session_id), existing, args.json);
    }

    conn.execute(
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
            root_session_id,
            session_id,
            args.parent_session_id,
            name,
            role,
            args.model,
            status,
            now,
            args.expires_at,
            metadata_json,
        ],
    )
    .map_err(|error| format!("failed to register session: {error}"))?;

    let record = session_by_name(conn, &root_session_id, &name)?
        .ok_or_else(|| "registered session was not readable".to_string())?;
    if args.json {
        print_json_report(db_path, Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-register] owner=rust rootSession=\"{}\" session=\"{}\" name=\"{}\" role=\"{}\" status=\"{}\" db=\"{}\"",
            escape_field(&root_session_id),
            escape_field(&session_id),
            escape_field(&name),
            escape_field(&role),
            escape_field(&status),
            db_path.display()
        );
        Ok(())
    }
}

fn list_sessions(conn: &Connection, db_path: &Path, args: &SessionArgs) -> Result<(), String> {
    let root_filter = if args.all {
        None
    } else {
        args.root_session_id
            .clone()
            .or_else(current_recall_session_id)
    };
    refresh_expired_sessions(conn)?;
    let mut sessions = query_sessions(conn, root_filter.as_deref(), args.name.as_deref())?;
    if args.active {
        let now = unix_timestamp()?;
        sessions.retain(|session| session.is_routable_at(now));
    }
    if args.json {
        return print_json_report(db_path, root_filter.as_deref(), sessions);
    }
    println!(
        "[agent-session-list] owner=rust rootSession={} sessions={} db=\"{}\"",
        root_filter
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"*\"".to_string()),
        sessions.len(),
        db_path.display()
    );
    for session in sessions {
        print_session_row(&session);
    }
    Ok(())
}

fn reuse_session(conn: &Connection, db_path: &Path, args: &SessionArgs) -> Result<(), String> {
    refresh_expired_sessions(conn)?;
    let root_session_id = args
        .root_session_id
        .clone()
        .or_else(current_recall_session_id)
        .ok_or_else(|| {
            "asp agent session reuse requires --root-session-id or agent session env".to_string()
        })?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let Some(record) = session_by_name(conn, &root_session_id, name)? else {
        return print_reuse_miss(db_path, Some(&root_session_id), name, "missing", args.json);
    };
    let now = unix_timestamp()?;
    if record.is_routable_at(now) {
        print_reuse_session(db_path, Some(&root_session_id), record, args.json)
    } else {
        let reason = record.status.clone();
        print_reuse_miss(db_path, Some(&root_session_id), name, &reason, args.json)
    }
}

fn show_session(conn: &Connection, db_path: &Path, args: &SessionArgs) -> Result<(), String> {
    refresh_expired_sessions(conn)?;
    let record = if let Some(session_id) = args.child_session_id.as_deref() {
        session_by_id(conn, session_id)?
    } else {
        let name = required_non_empty(args.name.as_deref(), "--name or --child-session-id")?;
        let root_session_id = args
            .root_session_id
            .clone()
            .or_else(current_recall_session_id)
            .ok_or_else(|| {
                "asp agent session show --name requires --root-session-id or agent session env"
                    .to_string()
            })?;
        session_by_name(conn, &root_session_id, name)?
    }
    .ok_or_else(|| "session registry entry not found".to_string())?;

    if args.json {
        let root_session_id = record.root_session_id.clone();
        print_json_report(db_path, Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-show] owner=rust rootSession=\"{}\" sessions=1 db=\"{}\"",
            escape_field(&record.root_session_id),
            db_path.display()
        );
        print_session_row(&record);
        Ok(())
    }
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
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
    ensure_session_column(conn, "last_seen_at", "INTEGER")?;
    ensure_session_column(conn, "last_heartbeat_at", "INTEGER")?;
    ensure_session_column(conn, "expires_at", "INTEGER")?;
    ensure_session_column(conn, "archived_at", "INTEGER")?;
    ensure_session_column(conn, "last_tool_event", "TEXT")?;
    ensure_session_column(conn, "last_command", "TEXT")?;
    ensure_session_column(conn, "last_evidence_ref", "TEXT")?;
    Ok(())
}

fn ensure_session_column(conn: &Connection, name: &str, sql_type: &str) -> Result<(), String> {
    let mut stmt = conn
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
    conn.execute(
        &format!("ALTER TABLE asp_agent_sessions ADD COLUMN {name} {sql_type}"),
        [],
    )
    .map_err(|error| format!("failed to add session registry column `{name}`: {error}"))?;
    Ok(())
}

fn query_sessions(
    conn: &Connection,
    root_session_id: Option<&str>,
    name: Option<&str>,
) -> Result<Vec<SessionRecord>, String> {
    match (root_session_id, name) {
        (Some(root_session_id), Some(name)) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE root_session_id = ?1 AND name = ?2
             ORDER BY updated_at DESC, name ASC",
            params![root_session_id, name],
        ),
        (Some(root_session_id), None) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE root_session_id = ?1
             ORDER BY updated_at DESC, name ASC",
            params![root_session_id],
        ),
        (None, Some(name)) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             WHERE name = ?1
             ORDER BY updated_at DESC, root_session_id ASC",
            params![name],
        ),
        (None, None) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
             FROM asp_agent_sessions
             ORDER BY updated_at DESC, root_session_id ASC, name ASC",
            params![],
        ),
    }
}

fn query_session_rows<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<SessionRecord>, String> {
    let mut statement = conn
        .prepare(sql)
        .map_err(|error| format!("failed to prepare session query: {error}"))?;
    let rows = statement
        .query_map(params, session_record_from_row)
        .map_err(|error| format!("failed to query sessions: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to read session row: {error}"))
}

fn session_by_name(
    conn: &Connection,
    root_session_id: &str,
    name: &str,
) -> Result<Option<SessionRecord>, String> {
    conn.query_row(
        "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
         FROM asp_agent_sessions
         WHERE root_session_id = ?1 AND name = ?2",
        params![root_session_id, name],
        session_record_from_row,
    )
    .optional()
    .map_err(|error| format!("failed to read session by name: {error}"))
}

fn session_by_id(conn: &Connection, session_id: &str) -> Result<Option<SessionRecord>, String> {
    conn.query_row(
        "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, last_seen_at, last_heartbeat_at, expires_at, archived_at, last_tool_event, last_command, last_evidence_ref, metadata_json
         FROM asp_agent_sessions
         WHERE session_id = ?1",
        params![session_id],
        session_record_from_row,
    )
    .optional()
    .map_err(|error| format!("failed to read session by id: {error}"))
}

pub(crate) fn current_registered_session(
    project_root: &Path,
) -> Result<Option<RegisteredSession>, String> {
    let Some(session) = super::agent_session::current_agent_session() else {
        return Ok(None);
    };
    let Some(conn) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    session_by_id(&conn, &session.id).map(|record| record.map(RegisteredSession::from_record))
}

pub(crate) fn has_current_agent_session() -> bool {
    super::agent_session::current_agent_session().is_some()
}

pub(crate) fn current_root_session_id() -> Option<String> {
    current_recall_session_id()
}

pub(crate) fn asp_explore_session_for_current_root(
    project_root: &Path,
    session_name: &str,
) -> Result<Option<RegisteredSession>, String> {
    let Some(root_session_id) = current_recall_session_id() else {
        return Ok(None);
    };
    let Some(conn) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    session_by_name(&conn, &root_session_id, session_name)
        .map(|record| record.map(RegisteredSession::from_record))
}

fn open_existing_registry(project_root: &Path) -> Result<Option<Connection>, String> {
    let db_path = agent_state_root_for_project(project_root).join(REGISTRY_DB_NAME);
    if !db_path.is_file() {
        return Ok(None);
    }
    let conn = Connection::open(&db_path).map_err(|error| {
        format!(
            "failed to open agent session registry `{}`: {error}",
            db_path.display()
        )
    })?;
    ensure_schema(&conn)?;
    refresh_expired_sessions(&conn)?;
    Ok(Some(conn))
}

fn session_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
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

fn current_recall_session_id() -> Option<String> {
    super::agent_session::current_agent_session()
        .map(|session| session.recall_session_id().to_string())
}

fn resolve_state_root(project_root: &Path, state_root: Option<&Path>) -> PathBuf {
    match state_root {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => project_root.join(path),
        None => agent_state_root_for_project(project_root),
    }
}

fn agent_state_root_for_project(project_root: &Path) -> PathBuf {
    env::var_os("PRJ_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.join(".cache"))
        .join("agent-semantic-protocol")
        .join("agent")
}

fn normalized_metadata(value: Option<&str>) -> Result<String, String> {
    let Some(value) = value else {
        return Ok("{}".to_string());
    };
    let parsed: serde_json::Value = serde_json::from_str(value)
        .map_err(|error| format!("--metadata-json must be valid JSON: {error}"))?;
    Ok(parsed.to_string())
}

fn unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before unix epoch: {error}"))?;
    i64::try_from(duration.as_secs()).map_err(|error| format!("timestamp overflow: {error}"))
}

fn refresh_expired_sessions(conn: &Connection) -> Result<(), String> {
    let now = unix_timestamp()?;
    conn.execute(
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

fn required_non_empty<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, String> {
    let value = value.ok_or_else(|| format!("asp agent session requires {name}"))?;
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(value.trim())
    }
}
