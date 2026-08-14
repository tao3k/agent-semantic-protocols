//! Top-level command dispatch for protocol subcommands.

use std::{env, path::PathBuf};

use super::agent_control_plane::{run_agent_command, run_session_control_plane_command};
use super::ast_patch::run_ast_patch_command;
use super::dispatch_agent_session_policy::is_agent_session_control_json_command;
use super::document_provider;
use super::graph::run_graph_command;
use super::healthcheck::run_healthcheck_command;
use super::hook::run_hook_command;
use super::install_provider::run_install_command;
use super::live_corpus::run_live_corpus_command;
use super::paths::run_paths_command;
use super::provider_dispatch::run_language_command;
use super::root_language_facade::run_root_language_facade;
use super::run_protocol_version_command;
use super::runtime_server::run_runtime_server_command;
use super::source_access::run_source_access_command;

pub(crate) async fn run_protocol_command(args: Vec<String>) -> Result<(), String> {
    run_protocol_command_started(args, tokio::time::Instant::now()).await
}

pub(crate) async fn run_protocol_command_started(
    args: Vec<String>,
    process_started: tokio::time::Instant,
) -> Result<(), String> {
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
        Some("providers" | "doctor" | "cache" | "cloud" | "tools" | "wrap" | "fd" | "rg") => {
            run_client_command(args).await
        }
        Some("search") if args.get(1).is_some_and(|arg| arg == "history") => {
            run_client_command(args).await
        }
        Some("search") => run_root_language_facade("search", &args[1..]).await,
        Some("query") => {
            match super::provider_selector::root_structural_selector_language(&args[1..])? {
                Some(language_id) => {
                    run_language_command(&language_id, &args[1..], process_started).await
                }
                None => run_root_language_facade("query", &args[1..]).await,
            }
        }
        Some("check") => Err(
            "asp check is not a public command surface; use asp <rust|typescript|python|julia> check ..."
                .to_string(),
        ),
        Some("hook") => run_hook_command(&args[1..]).await,
        Some("agent") => run_agent_command(&args[1..]),
        Some("session") => run_session_control_plane_command(&args[1..]).await,
        Some("install") => run_install_command(&args[1..]).await,
        Some("paths") => run_paths_command(&args[1..]),
        Some("healthcheck") => run_healthcheck_command(&args[1..]).await,
        Some("server") => run_runtime_server_command(&args[1..]).await,
        Some("live-corpus") => run_live_corpus_command(&args[1..]).await,
        Some("source-access") => run_source_access_command(&args[1..]),
        Some("ast-patch") => run_ast_patch_command(&args[1..]),
        Some("graph") => run_graph_command(&args[1..]).await,
        Some(document_id) if document_provider::is_document_language(document_id) => {
            document_provider::run_language_command(document_id, &args[1..]).await
        }
        Some(language_id) => run_language_command(language_id, &args[1..], process_started).await,
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

fn usage() -> String {
    "usage: asp [--help|--version] <guide|providers|tools|wrap|cache|cloud|hook|agent|install|paths|healthcheck|server|workspace-db|live-corpus|source-access|ast-patch|graph|fd|rg|search|query|rust|typescript|python|julia|org|md> ...".to_string()
}

async fn run_client_command(args: Vec<String>) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    agent_semantic_client::run_cli_args(None, args, cwd).await
}
