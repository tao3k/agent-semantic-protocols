//! Durable agent session and subagent registry.

#[path = "agent_session_registry_args.rs"]
mod agent_session_registry_args;
#[path = "agent_session_registry_render.rs"]
mod agent_session_registry_render;
#[path = "agent_session_registry_tool_event.rs"]
mod agent_session_registry_tool_event;

use agent_semantic_client_db::{
    AgentSessionRecord, AgentSessionRegisterRequest, AgentSessionRegistry,
    agent_session_status_is_routable, agent_session_unix_timestamp,
};
use agent_session_registry_args::{
    SessionArgs, SessionCommand, agent_usage, session_guide, session_usage,
};
use agent_session_registry_render::{
    escape_field, print_json_report, print_reuse_miss, print_reuse_session, print_session_row,
};
pub(crate) use agent_session_registry_tool_event::record_current_session_tool_event;
use std::{
    env,
    path::{Path, PathBuf},
};

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
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    Ok(registry
        .session_by_id(session_id)?
        .map(|record| record.root_session_id))
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
    let registry = match args.state_root.as_deref() {
        Some(state_root) => {
            let state_root = resolve_explicit_state_root(&project_root, state_root);
            AgentSessionRegistry::open_or_create_state_root(state_root)?
        }
        None => AgentSessionRegistry::open_or_create_project(&project_root)?,
    };

    match args.command {
        SessionCommand::Register => register_session(&registry, &args),
        SessionCommand::List => list_sessions(&registry, &args),
        SessionCommand::Show => show_session(&registry, &args),
        SessionCommand::Reuse => reuse_session(&registry, &args),
    }
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
    fn from_record(record: AgentSessionRecord) -> Self {
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
        agent_session_status_is_routable(&self.status)
            && self.expires_at.is_none_or(|expires| expires > now)
    }
}

fn register_session(registry: &AgentSessionRegistry, args: &SessionArgs) -> Result<(), String> {
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
    let now = agent_session_unix_timestamp()?;
    if !args.replace
        && let Some(existing) = registry.session_by_name(&root_session_id, &name)?
        && existing.session_id != session_id
        && existing.is_routable_at(now)
    {
        return print_reuse_session(
            registry.db_path(),
            Some(&root_session_id),
            existing,
            args.json,
        );
    }

    let record = registry.register_session(AgentSessionRegisterRequest {
        root_session_id: &root_session_id,
        session_id: &session_id,
        parent_session_id: args.parent_session_id.as_deref(),
        name: &name,
        role: &role,
        model: args.model.as_deref(),
        status: &status,
        expires_at: args.expires_at,
        metadata_json: &metadata_json,
        now,
    })?;
    if args.json {
        print_json_report(registry.db_path(), Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-register] owner=rust rootSession=\"{}\" session=\"{}\" name=\"{}\" role=\"{}\" status=\"{}\" db=\"{}\"",
            escape_field(&root_session_id),
            escape_field(&session_id),
            escape_field(&name),
            escape_field(&role),
            escape_field(&status),
            registry.db_path().display()
        );
        Ok(())
    }
}

fn list_sessions(registry: &AgentSessionRegistry, args: &SessionArgs) -> Result<(), String> {
    let root_filter = if args.all {
        None
    } else {
        args.root_session_id
            .clone()
            .or_else(current_recall_session_id)
    };
    registry.refresh_expired_sessions()?;
    let mut sessions = registry.query_sessions(root_filter.as_deref(), args.name.as_deref())?;
    if args.active {
        let now = agent_session_unix_timestamp()?;
        sessions.retain(|session| session.is_routable_at(now));
    }
    if args.json {
        return print_json_report(registry.db_path(), root_filter.as_deref(), sessions);
    }
    println!(
        "[agent-session-list] owner=rust rootSession={} sessions={} db=\"{}\"",
        root_filter
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"*\"".to_string()),
        sessions.len(),
        registry.db_path().display()
    );
    for session in sessions {
        print_session_row(&session);
    }
    Ok(())
}

fn reuse_session(registry: &AgentSessionRegistry, args: &SessionArgs) -> Result<(), String> {
    registry.refresh_expired_sessions()?;
    let root_session_id = args
        .root_session_id
        .clone()
        .or_else(current_recall_session_id)
        .ok_or_else(|| {
            "asp agent session reuse requires --root-session-id or agent session env".to_string()
        })?;
    let name = required_non_empty(args.name.as_deref(), "--name")?;
    let Some(record) = registry.session_by_name(&root_session_id, name)? else {
        return print_reuse_miss(
            registry.db_path(),
            Some(&root_session_id),
            name,
            "missing",
            args.json,
        );
    };
    let now = agent_session_unix_timestamp()?;
    if record.is_routable_at(now) {
        print_reuse_session(
            registry.db_path(),
            Some(&root_session_id),
            record,
            args.json,
        )
    } else {
        let reason = record.status.clone();
        print_reuse_miss(
            registry.db_path(),
            Some(&root_session_id),
            name,
            &reason,
            args.json,
        )
    }
}

fn show_session(registry: &AgentSessionRegistry, args: &SessionArgs) -> Result<(), String> {
    registry.refresh_expired_sessions()?;
    let record = if let Some(session_id) = args.child_session_id.as_deref() {
        registry.session_by_id(session_id)?
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
        registry.session_by_name(&root_session_id, name)?
    }
    .ok_or_else(|| "session registry entry not found".to_string())?;

    if args.json {
        let root_session_id = record.root_session_id.clone();
        print_json_report(registry.db_path(), Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-show] owner=rust rootSession=\"{}\" sessions=1 db=\"{}\"",
            escape_field(&record.root_session_id),
            registry.db_path().display()
        );
        print_session_row(&record);
        Ok(())
    }
}

pub(crate) fn current_registered_session(
    project_root: &Path,
) -> Result<Option<RegisteredSession>, String> {
    let Some(session) = super::agent_session::current_agent_session() else {
        return Ok(None);
    };
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    registry
        .session_by_id(&session.id)
        .map(|record| record.map(RegisteredSession::from_record))
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
    let Some(registry) = open_existing_registry(project_root)? else {
        return Ok(None);
    };
    registry
        .session_by_name(&root_session_id, session_name)
        .map(|record| record.map(RegisteredSession::from_record))
}

fn open_existing_registry(project_root: &Path) -> Result<Option<AgentSessionRegistry>, String> {
    AgentSessionRegistry::open_existing_project(project_root)
}

fn current_recall_session_id() -> Option<String> {
    super::agent_session::current_agent_session()
        .map(|session| session.recall_session_id().to_string())
}

fn resolve_explicit_state_root(project_root: &Path, state_root: &Path) -> PathBuf {
    if state_root.is_absolute() {
        state_root.to_path_buf()
    } else {
        project_root.join(state_root)
    }
}

fn normalized_metadata(value: Option<&str>) -> Result<String, String> {
    let Some(value) = value else {
        return Ok("{}".to_string());
    };
    let parsed: serde_json::Value = serde_json::from_str(value)
        .map_err(|error| format!("--metadata-json must be valid JSON: {error}"))?;
    Ok(parsed.to_string())
}

fn required_non_empty<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str, String> {
    let value = value.ok_or_else(|| format!("asp agent session requires {name}"))?;
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(value.trim())
    }
}
