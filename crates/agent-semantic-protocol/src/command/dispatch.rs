//! Top-level command dispatch for protocol subcommands.

use std::env;

use super::ast_patch::run_ast_patch_command;
use super::graph::run_graph_command;
use super::healthcheck::run_healthcheck_command;
use super::hook::run_hook_command;
use super::provider::run_language_command;
use super::source_access::run_source_access_command;

pub(crate) fn run_protocol_command(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("guide" | "providers" | "doctor" | "cache" | "cloud") => run_client_command(args),
        Some(command @ ("search" | "query" | "check")) => Err(format!(
            "asp {command} is not a public command surface; use asp <rust|typescript|python> {command} ..."
        )),
        Some("hook") => run_hook_command(&args[1..]),
        Some("healthcheck") => run_healthcheck_command(&args[1..]),
        Some("source-access") => run_source_access_command(&args[1..]),
        Some("ast-patch") => run_ast_patch_command(&args[1..]),
        Some("graph") => run_graph_command(&args[1..]),
        Some("help" | "--help" | "-h") => Err(usage()),
        Some(language_id) => run_language_command(language_id, &args[1..]),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: asp <guide|providers|doctor|cache|cloud|search|query|check|hook|healthcheck|source-access|ast-patch|graph|rust|typescript|python> ...".to_string()
}

fn run_client_command(args: Vec<String>) -> Result<(), String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    agent_semantic_client::run_cli_args(None, args, cwd)
}
