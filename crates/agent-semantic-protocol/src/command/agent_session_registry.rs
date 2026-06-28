//! Durable agent session and subagent registry.

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
    }
}

#[derive(Clone, Copy)]
enum SessionCommand {
    Register,
    List,
    Show,
}

struct SessionArgs {
    help: bool,
    command: SessionCommand,
    state_root: Option<PathBuf>,
    name: Option<String>,
    session_id: Option<String>,
    root_session_id: Option<String>,
    parent_session_id: Option<String>,
    role: Option<String>,
    model: Option<String>,
    status: Option<String>,
    metadata_json: Option<String>,
    all: bool,
    json: bool,
}

impl SessionArgs {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut parsed = Self {
            help: false,
            command: SessionCommand::List,
            state_root: None,
            name: None,
            session_id: None,
            root_session_id: None,
            parent_session_id: None,
            role: None,
            model: None,
            status: None,
            metadata_json: None,
            all: false,
            json: false,
        };
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "-h" | "--help" | "help" => parsed.help = true,
                "register" | "add" | "upsert" if index == 0 => {
                    parsed.command = SessionCommand::Register;
                }
                "list" | "ls" if index == 0 => parsed.command = SessionCommand::List,
                "show" | "get" if index == 0 => parsed.command = SessionCommand::Show,
                "--state-root" => {
                    index += 1;
                    parsed.state_root = Some(PathBuf::from(required_flag_value(
                        args,
                        index,
                        "--state-root",
                    )?));
                }
                "--name" => {
                    index += 1;
                    parsed.name = Some(non_empty_flag(args, index, "--name")?.to_string());
                }
                "--session-id" | "--session" => {
                    index += 1;
                    parsed.session_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--root-session-id" | "--root" => {
                    index += 1;
                    parsed.root_session_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--parent-session-id" | "--parent" => {
                    index += 1;
                    parsed.parent_session_id =
                        Some(non_empty_flag(args, index, arg.as_str())?.to_string());
                }
                "--role" => {
                    index += 1;
                    parsed.role = Some(non_empty_flag(args, index, "--role")?.to_string());
                }
                "--model" => {
                    index += 1;
                    parsed.model = Some(non_empty_flag(args, index, "--model")?.to_string());
                }
                "--status" => {
                    index += 1;
                    parsed.status = Some(non_empty_flag(args, index, "--status")?.to_string());
                }
                "--metadata-json" => {
                    index += 1;
                    parsed.metadata_json =
                        Some(non_empty_flag(args, index, "--metadata-json")?.to_string());
                }
                "--all" => parsed.all = true,
                "--json" => parsed.json = true,
                _ if arg.starts_with('-') => return Err(format!("unknown session flag `{arg}`")),
                _ => return Err(format!("unknown session subcommand `{arg}`")),
            }
            index += 1;
        }
        Ok(parsed)
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
    #[serde(rename = "metadataJson")]
    metadata_json: String,
}

#[derive(Serialize)]
struct SessionReport<'a> {
    owner: &'static str,
    #[serde(rename = "dbPath")]
    db_path: &'a str,
    #[serde(rename = "rootSessionId", skip_serializing_if = "Option::is_none")]
    root_session_id: Option<&'a str>,
    sessions: Vec<SessionRecord>,
}

fn register_session(conn: &Connection, db_path: &Path, args: &SessionArgs) -> Result<(), String> {
    let platform_session = super::agent_session::current_agent_session();
    let session_id = args
        .session_id
        .clone()
        .or_else(|| platform_session.as_ref().map(|session| session.id.clone()))
        .ok_or_else(|| {
            "asp agent session register requires --session-id or an agent session env".to_string()
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
            metadata_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9)
        ON CONFLICT(root_session_id, name) DO UPDATE SET
            session_id = excluded.session_id,
            parent_session_id = excluded.parent_session_id,
            role = excluded.role,
            model = excluded.model,
            status = excluded.status,
            updated_at = excluded.updated_at,
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
    let sessions = query_sessions(conn, root_filter.as_deref(), args.name.as_deref())?;
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

fn show_session(conn: &Connection, db_path: &Path, args: &SessionArgs) -> Result<(), String> {
    let record = if let Some(session_id) = args.session_id.as_deref() {
        session_by_id(conn, session_id)?
    } else {
        let name = required_non_empty(args.name.as_deref(), "--name or --session-id")?;
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
            metadata_json TEXT NOT NULL DEFAULT '{}',
            UNIQUE(root_session_id, name)
        );
        CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_root
            ON asp_agent_sessions(root_session_id);
        CREATE INDEX IF NOT EXISTS idx_asp_agent_sessions_parent
            ON asp_agent_sessions(parent_session_id);",
    )
    .map_err(|error| format!("failed to initialize session registry schema: {error}"))
}

fn query_sessions(
    conn: &Connection,
    root_session_id: Option<&str>,
    name: Option<&str>,
) -> Result<Vec<SessionRecord>, String> {
    match (root_session_id, name) {
        (Some(root_session_id), Some(name)) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, metadata_json
             FROM asp_agent_sessions
             WHERE root_session_id = ?1 AND name = ?2
             ORDER BY updated_at DESC, name ASC",
            params![root_session_id, name],
        ),
        (Some(root_session_id), None) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, metadata_json
             FROM asp_agent_sessions
             WHERE root_session_id = ?1
             ORDER BY updated_at DESC, name ASC",
            params![root_session_id],
        ),
        (None, Some(name)) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, metadata_json
             FROM asp_agent_sessions
             WHERE name = ?1
             ORDER BY updated_at DESC, root_session_id ASC",
            params![name],
        ),
        (None, None) => query_session_rows(
            conn,
            "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, metadata_json
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
        "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, metadata_json
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
        "SELECT root_session_id, session_id, parent_session_id, name, role, model, status, created_at, updated_at, metadata_json
         FROM asp_agent_sessions
         WHERE session_id = ?1",
        params![session_id],
        session_record_from_row,
    )
    .optional()
    .map_err(|error| format!("failed to read session by id: {error}"))
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
        metadata_json: row.get(9)?,
    })
}

fn print_json_report(
    db_path: &Path,
    root_session_id: Option<&str>,
    sessions: Vec<SessionRecord>,
) -> Result<(), String> {
    let report = SessionReport {
        owner: "rust",
        db_path: &db_path.display().to_string(),
        root_session_id,
        sessions,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to render session json: {error}"))?
    );
    Ok(())
}

fn print_session_row(session: &SessionRecord) {
    println!(
        "|session name=\"{}\" session=\"{}\" rootSession=\"{}\" parentSession={} role=\"{}\" model={} status=\"{}\" updatedAt={}",
        escape_field(&session.name),
        escape_field(&session.session_id),
        escape_field(&session.root_session_id),
        optional_field(session.parent_session_id.as_deref()),
        escape_field(&session.role),
        optional_field(session.model.as_deref()),
        escape_field(&session.status),
        session.updated_at
    );
}

fn optional_field(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_field(value)))
        .unwrap_or_else(|| "\"\"".to_string())
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

fn required_flag_value<'a>(
    args: &'a [String],
    index: usize,
    flag: &str,
) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn non_empty_flag<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    required_non_empty(Some(required_flag_value(args, index, flag)?), flag)
}

fn required_non_empty<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, String> {
    let value = value.ok_or_else(|| format!("asp agent session requires {name}"))?;
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(value.trim())
    }
}

fn escape_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn agent_usage() -> &'static str {
    "usage: asp agent <session> ..."
}

fn session_usage() -> &'static str {
    "usage: asp agent session <register|list|show> [--state-root PATH] [--name NAME] [--session-id ID] [--root-session-id ID] [--parent-session-id ID] [--role ROLE] [--model MODEL] [--status STATUS] [--json]"
}
