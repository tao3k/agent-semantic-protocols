//! Top-level command dispatch for protocol subcommands.

use std::{
    env,
    path::{Path, PathBuf},
};

use agent_semantic_config::render_hook_client_message_template;
use agent_semantic_hook::{
    ClientHookConfig, default_client_config_path, load_client_config_for_project,
};
use agent_semantic_runtime::{codex_rollout_session_metadata, current_agent_runtime_session};

use super::agent_session_registry::run_agent_command;
use super::ast_patch::run_ast_patch_command;
use super::dispatch_agent_session_policy::is_agent_session_direct_inventory_or_fetch_command;
use super::graph::run_graph_command;
use super::healthcheck::run_healthcheck_command;
use super::hook::run_hook_command;
use super::install_provider::run_install_command;
use super::paths::run_paths_command;
use super::protocol_version_line;
use super::provider::run_language_command;
use super::root_language_facade::run_root_language_facade;
use super::search_query_wrapper::{is_query_wrapper, run_query_wrapper_command};
use super::source_access::run_source_access_command;
use super::sync::run_sync_command;

pub(crate) fn run_protocol_command(mut args: Vec<String>) -> Result<(), String> {
    normalize_agent_session_command_args(&mut args)?;
    reject_file_workspace_for_search(&args)?;
    enforce_agent_session_asp_query_gate(&args)?;
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        Some("version" | "--version" | "-V") => {
            println!("{}", protocol_version_line());
            Ok(())
        }
        Some("guide" | "providers" | "doctor" | "cache" | "cloud" | "tools" | "wrap") => {
            run_client_command(args)
        }
        Some("search") if args.get(1).is_some_and(|arg| arg == "history") => {
            run_client_command(args)
        }
        Some(command @ ("search" | "query")) => run_root_language_facade(command, &args[1..]),
        Some("check") => Err(
            "asp check is not a public command surface; use asp <rust|typescript|python|julia> check ..."
                .to_string(),
        ),
        Some("hook") => run_hook_command(&args[1..]),
        Some("agent") => run_agent_command(&args[1..]),
        Some("install") => run_install_command(&args[1..]),
        Some("sync") => run_sync_command(&args[1..]),
        Some("paths") => run_paths_command(&args[1..]),
        Some("healthcheck") => run_healthcheck_command(&args[1..]),
        Some("source-access") => run_source_access_command(&args[1..]),
        Some("ast-patch") => run_ast_patch_command(&args[1..]),
        Some("graph") => run_graph_command(&args[1..]),
        Some(command) if is_query_wrapper(command) => run_query_wrapper_command(command, &args[1..]),
        Some(language_id) => run_language_command(language_id, &args[1..]),
        _ => Err(usage()),
    }
}

fn reject_file_workspace_for_search(args: &[String]) -> Result<(), String> {
    if !is_search_command_args(args) {
        return Ok(());
    }
    let Some(workspace) = arg_option_value(args, "--workspace") else {
        return Ok(());
    };
    if workspace.starts_with('-') {
        return Ok(());
    }
    let workspace_path = PathBuf::from(workspace);
    let workspace_path = if workspace_path.is_absolute() {
        workspace_path
    } else {
        env::current_dir()
            .map_err(|error| format!("failed to resolve current project directory: {error}"))?
            .join(workspace_path)
    };
    if workspace_path.is_file() {
        return Err(format!(
            "--workspace requires a directory project root, got file `{}`. Keep the file path as the owner/selector and use a directory workspace, for example `asp gerbil-scheme search owner <file> items --query '<terms>' --workspace . --view seeds`.",
            workspace_path.display()
        ));
    }
    Ok(())
}

fn is_search_command_args(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        || matches!(args.get(1).map(String::as_str), Some("search"))
}

fn arg_option_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let prefix = format!("{flag}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .or_else(|| {
            args.windows(2)
                .find_map(|window| (window[0] == flag).then_some(window[1].as_str()))
        })
}

fn normalize_agent_session_command_args(args: &mut Vec<String>) -> Result<(), String> {
    if !super::has_current_agent_session() || !is_org_search_memory_command(args) {
        return Ok(());
    }
    if option_is_present(args, "--session") {
        return Ok(());
    }
    let project_root = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current project directory: {error}"))?;
    let hook_config = load_dispatch_hook_config(&project_root)?;
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    let now = agent_semantic_client_db::agent_session_unix_timestamp()?;
    let Some(session) = super::current_registered_session(&project_root)? else {
        return Ok(());
    };
    if session.name == resident_child_name && session.is_routable_at(now) {
        args.push("--session".to_string());
        args.push(session.root_session_id);
    }
    Ok(())
}

fn enforce_agent_session_asp_query_gate(args: &[String]) -> Result<(), String> {
    if !has_agent_session_env() || !is_agent_session_restricted_asp_command(args) {
        return Ok(());
    }
    if is_agent_session_direct_inventory_or_fetch_command(args) {
        return Ok(());
    }
    if is_org_search_memory_command(args) && option_is_present(args, "--session") {
        return Ok(());
    }
    let project_root = env::current_dir()
        .map_err(|error| format!("failed to resolve current project directory: {error}"))?;
    let root_session_id = super::current_root_session_id()
        .or_else(agent_session_env_id)
        .unwrap_or_else(|| "unknown-agent-session".to_string());
    let command = if args.is_empty() {
        "asp".to_string()
    } else {
        format!("asp {}", args.join(" "))
    };
    let hook_config = load_dispatch_hook_config(&project_root)?;
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    if current_rollout_direct_resident_child(&hook_config)? {
        return Ok(());
    }
    let now = match agent_semantic_client_db::agent_session_unix_timestamp() {
        Ok(now) => now,
        Err(error) => {
            return Err(resident_child_registry_blocked_message(
                &hook_config,
                &root_session_id,
                &command,
                &error,
            ));
        }
    };
    let current_session = match super::current_registered_session(&project_root) {
        Ok(session) => session,
        Err(error) => {
            return Err(resident_child_registry_blocked_message(
                &hook_config,
                &root_session_id,
                &command,
                &error,
            ));
        }
    };
    if let Some(session) = current_session {
        if session.name == resident_child_name && session.is_routable_at(now) {
            return Ok(());
        }
    }
    let resident_child =
        match super::asp_explore_session_for_current_root(&project_root, resident_child_name) {
            Ok(session) => session,
            Err(error) => {
                return Err(resident_child_registry_blocked_message(
                    &hook_config,
                    &root_session_id,
                    &command,
                    &error,
                ));
            }
        };
    let resident_child = match resident_child {
        Some(child) => Some(child),
        None => match super::asp_explore_session_record_for_current_root(
            &project_root,
            resident_child_name,
        ) {
            Ok(session) => session,
            Err(error) => {
                return Err(resident_child_registry_blocked_message(
                    &hook_config,
                    &root_session_id,
                    &command,
                    &error,
                ));
            }
        },
    };
    if let Some(child) = resident_child {
        if child.name == resident_child_name && child.is_routable_at(now) {
            return Err(resident_child_with_child_message(
                &hook_config,
                &child.root_session_id,
                &child.session_id,
                &command,
            ));
        }
        return Err(resident_child_invalid_message(
            &hook_config,
            &child.root_session_id,
            &child.session_id,
            &child.status,
            &command,
        ));
    }
    Err(resident_child_missing_message(
        &hook_config,
        &root_session_id,
        &command,
    ))
}

fn load_dispatch_hook_config(project_root: &Path) -> Result<ClientHookConfig, String> {
    let config_path = default_client_config_path(&project_root.to_string_lossy());
    load_client_config_for_project(&config_path, project_root)
        .map_err(|error| format!("failed to load ASP hook config for agent session gate: {error}"))
}

fn current_rollout_direct_resident_child(hook_config: &ClientHookConfig) -> Result<bool, String> {
    let Some(session) = current_agent_runtime_session() else {
        return Ok(false);
    };
    let Some(metadata) = codex_rollout_session_metadata(session.recall_session_id())? else {
        return Ok(false);
    };
    if metadata.thread_source.as_deref() != Some("subagent") {
        return Ok(false);
    }
    if metadata.spawn_depth != Some(1) {
        return Ok(false);
    }
    if metadata.parent_thread_id.as_deref() != metadata.root_session_id.as_deref() {
        return Ok(false);
    }
    Ok(metadata.agent_role.as_deref().is_some_and(|agent_role| {
        configured_rollout_resident_identity_matches(agent_role, hook_config)
    }))
}

fn configured_rollout_resident_identity_matches(
    agent_role: &str,
    hook_config: &ClientHookConfig,
) -> bool {
    let normalized_role = normalize_rollout_agent_identity(agent_role);
    [
        hook_config.resident_asp_explore_child_name(),
        hook_config.resident_asp_explore_codex_agent_name(),
    ]
    .iter()
    .any(|candidate| normalize_rollout_agent_identity(candidate) == normalized_role)
}

fn normalize_rollout_agent_identity(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn resident_child_registry_blocked_message(
    hook_config: &ClientHookConfig,
    root_session_id: &str,
    command: &str,
    registry_error: &str,
) -> String {
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    let template = hook_config
        .agent_session_messages()
        .binary_gate_registry_blocked
        .as_deref()
        .unwrap_or("ASP query/search command denied because the session registry is unavailable.\ncommand={{command}}");
    render_hook_client_message_template(
        template,
        &[
            ("residentChildName", resident_child_name),
            ("rootSessionId", root_session_id),
            ("registryError", registry_error),
            ("command", command),
        ],
    )
}

fn resident_child_with_child_message(
    hook_config: &ClientHookConfig,
    root_session_id: &str,
    child_session_id: &str,
    command: &str,
) -> String {
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    let template = hook_config
        .agent_session_messages()
        .binary_gate_with_child
        .as_deref()
        .unwrap_or("ASP query/search command denied in main agent session.\ncommand={{command}}");
    render_hook_client_message_template(
        template,
        &[
            ("residentChildName", resident_child_name),
            ("rootSessionId", root_session_id),
            ("childSessionId", child_session_id),
            ("command", command),
        ],
    )
}

fn resident_child_invalid_message(
    hook_config: &ClientHookConfig,
    root_session_id: &str,
    child_session_id: &str,
    child_status: &str,
    command: &str,
) -> String {
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    let create_action = resident_child_create_action(hook_config);
    let template = hook_config
        .agent_session_messages()
        .binary_gate_invalid_child
        .as_deref()
        .unwrap_or("ASP query/search command denied by non-routable child.\ncommand={{command}}");
    render_hook_client_message_template(
        template,
        &[
            ("residentChildName", resident_child_name),
            (
                "residentCodexAgentName",
                hook_config.resident_asp_explore_codex_agent_name(),
            ),
            ("createAction", &create_action),
            ("rootSessionId", root_session_id),
            ("childSessionId", child_session_id),
            ("childStatus", child_status),
            ("command", command),
        ],
    )
}

fn resident_child_missing_message(
    hook_config: &ClientHookConfig,
    root_session_id: &str,
    command: &str,
) -> String {
    let resident_child_name = hook_config.resident_asp_explore_child_name();
    let create_action = resident_child_create_action(hook_config);
    let template = hook_config
        .agent_session_messages()
        .binary_gate_without_child
        .as_deref()
        .unwrap_or("ASP query/search command denied in agent session.\ncommand={{command}}");
    render_hook_client_message_template(
        template,
        &[
            ("residentChildName", resident_child_name),
            (
                "residentCodexAgentName",
                hook_config.resident_asp_explore_codex_agent_name(),
            ),
            ("createAction", &create_action),
            ("rootSessionId", root_session_id),
            ("command", command),
        ],
    )
}

fn resident_child_create_action(hook_config: &ClientHookConfig) -> String {
    if env::var_os("CODEX_THREAD_ID").is_some() {
        return format!(
            "Codex action: start the configured ASP managed subagent `{}` only if the host exposes that managed type; otherwise report bootstrapBlocked=host-agent-type-unavailable and do not create a generic subagent",
            hook_config.resident_asp_explore_codex_agent_name()
        );
    }
    if env::var_os("CLAUDE_CODE_SESSION_ID").is_some()
        || env::var_os("CLAUDECODE_SESSION_ID").is_some()
        || env::var_os("CLAUDE_SESSION_ID").is_some()
    {
        return "Claude action: start the configured subagent `asp-explorer`".to_string();
    }
    "Host action: start the configured resident ASP explore subagent".to_string()
}

fn has_agent_session_env() -> bool {
    [
        "CODEX_THREAD_ID",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDE_CODE_REMOTE_SESSION_ID",
        "AGENT_SESSION_ID",
        "SESSION_ID",
    ]
    .iter()
    .any(|name| {
        env::var(name)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn agent_session_env_id() -> Option<String> {
    [
        "CODEX_THREAD_ID",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDE_CODE_REMOTE_SESSION_ID",
        "AGENT_SESSION_ID",
        "SESSION_ID",
    ]
    .iter()
    .find_map(|name| {
        env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn is_agent_session_restricted_asp_command(args: &[String]) -> bool {
    let Some(first) = args.first().map(String::as_str) else {
        return false;
    };
    if matches!(first, "rg" | "fd" | "pipe" | "query") {
        return true;
    }
    if first == "search" {
        return true;
    }
    if first == "org" {
        return args
            .get(1)
            .is_some_and(|stage| matches!(stage.as_str(), "query" | "search"));
    }
    args.get(1)
        .is_some_and(|stage| matches!(stage.as_str(), "query" | "search"))
}

fn is_org_search_memory_command(args: &[String]) -> bool {
    matches!(
        (
            args.first().map(String::as_str),
            args.get(1).map(String::as_str),
            args.get(2).map(String::as_str),
        ),
        (Some("org"), Some("search"), Some("memory"))
    )
}

fn option_is_present(args: &[String], option: &str) -> bool {
    args.iter().any(|arg| arg == option)
}

fn usage() -> String {
    "usage: asp [--help|--version] <guide|providers|tools|wrap|cache|cloud|hook|agent|install|sync|paths|healthcheck|source-access|ast-patch|graph|fd|rg|search|query|rust|typescript|python|julia|org|md> ...".to_string()
}

fn run_client_command(args: Vec<String>) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    agent_semantic_client::run_cli_args(None, args, cwd)
}
