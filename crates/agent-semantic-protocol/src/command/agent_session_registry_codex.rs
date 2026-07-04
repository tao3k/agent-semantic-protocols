use agent_semantic_client_db::AgentSessionRegistry;

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_state::{
    current_project_session_scope_id, resolved_root_session_id,
};
use std::{
    env,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn run_codex_session_wrapper(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    command_name: &str,
    supports_force: bool,
) -> Result<(), String> {
    if args.force && !supports_force {
        return Err(format!(
            "asp agent session {command_name} does not support --force"
        ));
    }
    let mut codex_args = args.codex_args.clone();
    let target_session_id = resolved_codex_target_session(registry, args)?;
    if let Some(target_session_id) = target_session_id.as_ref() {
        codex_args.insert(0, target_session_id.clone());
    }
    if args.force && supports_force && !codex_args.iter().any(|arg| arg == "--force") {
        codex_args.insert(0, "--force".to_string());
    }

    let codex_bin = env::var("ASP_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());
    let status = Command::new(&codex_bin)
        .arg(command_name)
        .args(&codex_args)
        .status()
        .map_err(|error| format!("failed to run {codex_bin} {command_name}: {error}"))?;
    if status.success() {
        sync_codex_lifecycle_to_registry(registry, command_name, target_session_id.as_deref())?;
        return Ok(());
    }

    if command_name == "delete" && args.force {
        sync_codex_lifecycle_to_registry(registry, command_name, target_session_id.as_deref())?;
        eprintln!(
            "[agent-session-delete] codex delete exited with {status}; registry row was removed because --force was set"
        );
        return Ok(());
    }

    Err(format!("{codex_bin} {command_name} exited with {status}"))
}

fn resolved_codex_target_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<Option<String>, String> {
    let project_id = current_project_session_scope_id()?;
    if let Some(child_session_id) = args.child_session_id.as_deref() {
        return Ok(Some(child_session_id.to_string()));
    }
    let Some(name) = args.name.as_deref() else {
        return Ok(None);
    };
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?
        .ok_or_else(|| {
            "asp agent session codex wrapper with --name requires --root-session-id or agent session env"
                .to_string()
        })?;
    let record = registry
        .session_by_name(&project_id, &root_session_id, name)?
        .ok_or_else(|| format!("session registry entry `{name}` not found"))?;
    Ok(Some(record.session_id))
}

fn sync_codex_lifecycle_to_registry(
    registry: &AgentSessionRegistry,
    command_name: &str,
    target_session_id: Option<&str>,
) -> Result<(), String> {
    let Some(session_id) = target_session_id else {
        return Ok(());
    };
    let project_id = current_project_session_scope_id()?;
    match command_name {
        "archive" => {
            let now = codex_lifecycle_unix_timestamp()?;
            registry.archive_session(&project_id, session_id, now)?;
        }
        "unarchive" => {
            let now = codex_lifecycle_unix_timestamp()?;
            registry.unarchive_session(&project_id, session_id, now)?;
        }
        "delete" => {
            registry.delete_session(&project_id, session_id)?;
        }
        _ => {}
    }
    Ok(())
}

fn codex_lifecycle_unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before UNIX_EPOCH: {error}"))?;
    i64::try_from(duration.as_secs())
        .map_err(|_| "current UNIX timestamp does not fit i64".to_string())
}
