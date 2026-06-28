//! Top-level command dispatch for protocol subcommands.

use std::env;

use super::agent_session_registry::run_agent_command;
use super::ast_patch::run_ast_patch_command;
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

pub(crate) fn run_protocol_command(args: Vec<String>) -> Result<(), String> {
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

fn usage() -> String {
    "usage: asp [--help|--version] <guide|providers|tools|wrap|cache|cloud|hook|agent|install|sync|paths|healthcheck|source-access|ast-patch|graph|fd|rg|search|query|rust|typescript|python|julia|org|md> ...".to_string()
}

fn run_client_command(args: Vec<String>) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    agent_semantic_client::run_cli_args(None, args, cwd)
}
