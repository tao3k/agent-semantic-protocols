//! Public facade for the `semantic-agent-protocol` CLI.

mod cli;
mod command;

pub use cli::{run_cli_args, run_cli_from_env};
