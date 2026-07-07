#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;

pub mod graph;
pub mod query_owner_core;
mod state_cli;

pub use cli::{run_cli_args, run_cli_from_env};
#[doc(hidden)]
pub use state_cli::run_binary_from_env;
pub(crate) mod codex;
pub use command::search_pipe_selector_seed::{
    SelectorSeededSearchPipeRequest, render_selector_seeded_search_pipe,
};
