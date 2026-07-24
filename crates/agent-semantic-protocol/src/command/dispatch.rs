//! Top-level command dispatch for protocol subcommands.

use std::{
    env,
    path::{Path, PathBuf},
};

use agent_semantic_hook::{
    ClientHookConfig, default_client_config_path, load_client_config_for_project,
};

use super::agent_session_registry::run_agent_command;
use super::ast_patch::run_ast_patch_command;
use super::dispatch_agent_session_policy::is_agent_session_control_json_command;
use super::graph::run_graph_command;
use super::healthcheck::run_healthcheck_command;
use super::hook::run_hook_command;
use super::install_provider::run_install_command;
use super::paths::run_paths_command;
use super::provider_dispatch::run_language_command;
use super::root_language_facade::run_root_language_facade;
use super::run_protocol_version_command;
use super::source_access::run_source_access_command;
use super::sync::run_sync_command;

pub(crate) fn run_protocol_command(mut args: Vec<String>) -> Result<(), String> {
    normalize_agent_session_command_args(&mut args)?;
    if super::cli_help::print_help_if_requested(&args)? {
        return Ok(());
    }
    reject_agent_platform_json_output(&args)?;
    reject_file_workspace_for_search(&args)?;
    match args.first().map(String::as_str) {
        Some("help" | "--help" | "-h") => {
            println!("{}", usage());
            Ok(())
        }
        Some("version" | "--version" | "-V") => run_protocol_version_command(&args[1..]),
        Some("--contract-fingerprint") => {
            println!("{}", agent_semantic_config::hook_client_contract_fingerprint());
            Ok(())
        }
        Some(
            "guide" | "providers" | "doctor" | "cache" | "cloud" | "tools" | "wrap" | "fd"
            | "rg",
        ) => {
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
        Some(language_id) => run_language_command(language_id, &args[1..]),
        _ => Err(usage()),
    }
}

fn reject_agent_platform_json_output(args: &[String]) -> Result<(), String> {
    if !has_json_output_arg(args)
        || !agent_platform_session_active()
        || explicit_non_agent_platform_output()
        || is_agent_session_control_json_command(args)
    {
        return Ok(());
    }
    Err("warning: --json output is disabled inside agent platform sessions; JSON is for debug or programmatic use only, not normal agent workflow, because it wastes tokens. Use the default compact output, or set ASP_NO_AGENT_PLATFORM=1 only for non-agent/debug automation."
        .to_string())
}

fn has_json_output_arg(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--json" || arg.starts_with("--json="))
}

fn agent_platform_session_active() -> bool {
    const AGENT_PLATFORM_SESSION_ENV_VARS: &[&str] = &[
        "CODEX_THREAD_ID",
        "CODEX_PARENT_THREAD_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDE_CODE_SESSION_ID",
        "AGENT_SESSION_ID",
        "AGENT_PLATFORM_SESSION_ID",
    ];
    AGENT_PLATFORM_SESSION_ENV_VARS
        .iter()
        .any(|name| env_var_nonempty(name))
}

fn explicit_non_agent_platform_output() -> bool {
    env_var_enabled("ASP_NO_AGENT_PLATFORM")
}

fn env_var_nonempty(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn env_var_enabled(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| {
        let value = value.to_string_lossy();
        !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
    })
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

fn load_dispatch_hook_config(project_root: &Path) -> Result<ClientHookConfig, String> {
    let config_path = default_client_config_path(&project_root.to_string_lossy());
    load_client_config_for_project(&config_path, project_root)
        .map_err(|error| format!("failed to load ASP hook config for agent session: {error}"))
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
