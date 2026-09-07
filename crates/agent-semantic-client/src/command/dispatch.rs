// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Top-level command dispatch for protocol subcommands.

use std::env;
use std::path::PathBuf;

use super::agent_control_plane::run_config_command;
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
use super::root_language_facade::{run_workspace_query, run_workspace_search_playbook};
use super::run_protocol_version_command;
use super::runtime_server::run_runtime_server_command;
use super::schema::run_schema_command;
use super::session::run_session_command;

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
    if args.first().is_some_and(|command| command == "hook")
        && super::hook::is_runtime_independent_control_command(&args[1..])
    {
        return run_hook_command(&args[1..]).await;
    }
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
        Some("--hook-artifact-fingerprint") => {
            println!("{}", agent_semantic_hook::hook_runtime_artifact_fingerprint());
            Ok(())
        }
        Some(
            "providers" | "doctor" | "cache" | "clean" | "cloud" | "tools" | "wrap",
        ) => {
            run_client_command(args).await
        }
        Some("search") if args.get(1).is_some_and(|arg| arg == "history") => {
            run_client_command(args).await
        }
        Some("search") => run_workspace_search_playbook(&args[1..]).await,
        Some("query") => run_workspace_query(&args[1..]).await,
        Some("check") => Err(
            "asp check is not a public command surface; use asp <rust|typescript|python|julia> check ..."
                .to_string(),
        ),
        Some("hook") => run_hook_command(&args[1..]).await,
        Some("config") => run_config_command(&args[1..]),
        Some("install") => run_install_command(&args[1..]).await,
        Some("paths") => run_paths_command(&args[1..]),
        Some("healthcheck") => run_healthcheck_command(&args[1..]).await,
        Some("server") => run_runtime_server_command(&args[1..]).await,
        Some("schema") => run_schema_command(&args[1..]).await,
        Some("session") => run_session_command(&args[1..]).await,
        Some("live-corpus") => run_live_corpus_command(&args[1..]).await,
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
        || is_agent_session_control_json_command(args)
    {
        return Ok(());
    }
    Err("warning: --json output is disabled inside agent platform sessions because it is a debug/programmatic format. Normal ASP Explorer search and query use the typed compact receipt without `--json`."
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

fn env_var_nonempty(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
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
            "--workspace requires a directory project root, got file `{}`. Keep the file path as the search scope and use a directory workspace, for example `asp gerbil-scheme search '<terms>' --scope owner:<file> --workspace .`.",
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
    "usage: asp [--help|--version] <guide|providers|tools|wrap|cache|clean|cloud|hook|config|session|install|paths|healthcheck|server|schema|workspace-db|live-corpus|ast-patch|graph|search|query|rust|typescript|python|julia|org|md> ...".to_string()
}

async fn run_client_command(args: Vec<String>) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    agent_semantic_client::run_client_cli_args(None, args, cwd).await
}
