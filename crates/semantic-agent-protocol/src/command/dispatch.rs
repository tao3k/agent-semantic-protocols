//! Top-level command dispatch for protocol subcommands.

use super::ast_patch::run_ast_patch_command;
use super::hook::run_hook_command;

pub(crate) fn run_protocol_command(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("hook") => run_hook_command(&args[1..]),
        Some("ast-patch") => run_ast_patch_command(&args[1..]),
        Some("help" | "--help" | "-h") => Err(usage()),
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: semantic-agent-protocol <hook|ast-patch> ...".to_string()
}
