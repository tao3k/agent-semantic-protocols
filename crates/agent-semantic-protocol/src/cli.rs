//! Process argument entrypoints for the `asp` CLI.

use crate::command::run_protocol_command;
use std::env;

/// Run the `asp` CLI using process arguments.
pub fn run_cli_from_env() -> Result<(), String> {
    run_cli_from_env_started(tokio::time::Instant::now())
}

pub(crate) fn run_cli_from_env_started(
    process_started: tokio::time::Instant,
) -> Result<(), String> {
    crate::command::run_protocol_command_started(env::args().skip(1).collect(), process_started)
}

/// Run the `asp` CLI using caller-provided arguments.
pub fn run_cli_args<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run_protocol_command(args.into_iter().map(Into::into).collect())
}
