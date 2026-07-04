//! Durable agent session and subagent registry.

#[path = "agent_session_registry_args.rs"]
mod agent_session_registry_args;
#[path = "agent_session_registry_codex.rs"]
mod agent_session_registry_codex;
#[path = "agent_session_registry_commands.rs"]
mod agent_session_registry_commands;
#[path = "agent_session_registry_render.rs"]
mod agent_session_registry_render;
#[path = "agent_session_registry_rollout_activity.rs"]
mod agent_session_registry_rollout_activity;
#[path = "agent_session_registry_state.rs"]
mod agent_session_registry_state;
#[path = "agent_session_registry_tool_event.rs"]
mod agent_session_registry_tool_event;
#[path = "agent_session_registry_validation.rs"]
mod agent_session_registry_validation;

use agent_semantic_client_db::AgentSessionRegistry;
use agent_session_registry_args::{
    SessionArgs, SessionCommand, agent_usage, session_guide, session_usage,
};
use agent_session_registry_codex::run_codex_session_wrapper;
use agent_session_registry_commands::{
    close_session, gc_sessions, lifecycle_audit_session, list_sessions, reconcile_sessions,
    register_session, reuse_session, show_session, status_session,
};
use agent_session_registry_state::open_or_create_default_registry;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub(crate) use agent_session_registry_state::{
    asp_explore_session_for_current_root, asp_explore_session_record_for_current_root,
    current_registered_session, current_root_session_id, has_current_agent_session,
    registered_root_session_id,
};
pub(crate) use agent_session_registry_tool_event::record_current_session_tool_event;

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
            let state_root =
                AgentSessionRegistry::resolve_state_root_override(&project_root, state_root);
            AgentSessionRegistry::open_or_create_state_root(state_root)?
        }
        None => open_or_create_default_registry(&project_root)?,
    };

    match args.command {
        SessionCommand::Register => register_session(&registry, &args),
        SessionCommand::List => list_sessions(&registry, &args),
        SessionCommand::Show => show_session(&registry, &args),
        SessionCommand::Reuse => reuse_session(&registry, &args),
        SessionCommand::Status => status_session(&registry, &args, &project_root),
        SessionCommand::LifecycleAudit => lifecycle_audit_session(&registry, &args),
        SessionCommand::Close => close_session(&registry, &args),
        SessionCommand::Gc => gc_sessions(&registry, &args),
        SessionCommand::Reconcile => reconcile_sessions(&registry, &args),
        SessionCommand::Resume => run_codex_session_wrapper(&registry, &args, "resume", false),
        SessionCommand::Fork => run_codex_session_wrapper(&registry, &args, "fork", false),
        SessionCommand::Archive => run_codex_session_wrapper(&registry, &args, "archive", false),
        SessionCommand::Delete => run_codex_session_wrapper(&registry, &args, "delete", true),
        SessionCommand::Unarchive => {
            run_codex_session_wrapper(&registry, &args, "unarchive", false)
        }
        SessionCommand::SwitchModel => switch_model(&args),
    }
}

fn switch_model(args: &SessionArgs) -> Result<(), String> {
    let model = args
        .model
        .as_deref()
        .ok_or_else(|| "agent session switch-model requires --model".to_string())?;
    let platform = active_platform()
        .ok_or_else(|| "failed to detect active agent platform session".to_string())?;
    match platform {
        "codex" => switch_codex_model(model, args),
        other => Err(format!(
            "agent session switch-model does not support platform `{other}`"
        )),
    }
}

fn active_platform() -> Option<&'static str> {
    if env::var_os("CODEX_THREAD_ID").is_some() {
        return Some("codex");
    }
    if env::var_os("CLAUDE_CODE_SESSION_ID").is_some()
        || env::var_os("CLAUDE_CODE_REMOTE_SESSION_ID").is_some()
    {
        return Some("claude-code");
    }
    None
}

fn switch_codex_model(model: &str, args: &SessionArgs) -> Result<(), String> {
    let agents_config_path = asp_agents_config_path()?;
    write_codex_dynamic_model(&agents_config_path, model)?;

    let mut updated_agent_configs = Vec::new();
    let asp_agents_dir = agents_config_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", agents_config_path.display()))?;
    update_asp_codex_agent_sources_and_projections(
        asp_agents_dir,
        &codex_home().join("agents"),
        model,
        &mut updated_agent_configs,
    )?;

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "status": "switched",
                "platform": "codex",
                "model": model,
                "configPath": agents_config_path,
                "updatedAgentConfigs": updated_agent_configs,
            })
        );
    } else {
        println!(
            "switched codex model to {model}; config={}; updatedAgentConfigs={}",
            agents_config_path.display(),
            updated_agent_configs.len()
        );
    }
    Ok(())
}

fn asp_agents_config_path() -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::state_core::resolve_state_home()?
        .join("agents")
        .join("config.toml"))
}

fn codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

fn write_codex_dynamic_model(config_path: &Path, model: &str) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut value = if config_path.exists() {
        let text = fs::read_to_string(config_path)
            .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
        toml::from_str::<toml::Value>(&text)
            .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))?
    } else {
        toml::Value::Table(Default::default())
    };
    let root = value
        .as_table_mut()
        .ok_or_else(|| format!("{} must contain a TOML table", config_path.display()))?;
    let platform = root
        .entry("platform".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| format!("{}.platform must be a TOML table", config_path.display()))?;
    let codex = platform
        .entry("codex".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| {
            format!(
                "{}.platform.codex must be a TOML table",
                config_path.display()
            )
        })?;
    let models = codex
        .entry("models".to_string())
        .or_insert_with(|| toml::Value::Table(Default::default()))
        .as_table_mut()
        .ok_or_else(|| {
            format!(
                "{}.platform.codex.models must be a TOML table",
                config_path.display()
            )
        })?;
    models.insert(
        "primary".to_string(),
        toml::Value::String(model.to_string()),
    );
    write_toml_value(config_path, &value)
}

fn update_agent_model_file(path: &Path, model: &str) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut value = toml::from_str::<toml::Value>(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| format!("{} must contain a TOML table", path.display()))?;
    table.insert("model".to_string(), toml::Value::String(model.to_string()));
    write_toml_value(path, &value)
}

fn write_toml_value(path: &Path, value: &toml::Value) -> Result<(), String> {
    let mut text = toml::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {error}", path.display()))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    fs::write(path, text).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn update_asp_codex_agent_sources_and_projections(
    asp_agents_dir: &Path,
    codex_agents_dir: &Path,
    model: &str,
    updated_agent_configs: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !asp_agents_dir.exists() {
        return Ok(());
    }
    fs::create_dir_all(codex_agents_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_agents_dir.display()))?;
    for entry in fs::read_dir(asp_agents_dir)
        .map_err(|error| format!("failed to read {}: {error}", asp_agents_dir.display()))?
    {
        let entry = entry
            .map_err(|error| format!("failed to read {}: {error}", asp_agents_dir.display()))?;
        let source_path = entry.path();
        if !source_path.is_file() {
            continue;
        }
        let Some(file_name) = source_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
        else {
            continue;
        };
        let Some(projection_stem) = file_name.strip_suffix("_codex.toml") else {
            continue;
        };
        update_agent_model_file(&source_path, model)?;
        updated_agent_configs.push(source_path.clone());

        let projection_path = codex_agents_dir.join(format!("{projection_stem}.toml"));
        fs::copy(&source_path, &projection_path).map_err(|error| {
            format!(
                "failed to copy {} to {}: {error}",
                source_path.display(),
                projection_path.display()
            )
        })?;
        updated_agent_configs.push(projection_path);
    }
    Ok(())
}
