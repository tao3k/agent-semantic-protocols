#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;
pub mod graph;

pub use cli::{run_cli_args, run_cli_from_env};
#[doc(hidden)]
pub use command::search_pipe_selector_seed::render_selector_seeded_search_pipe;
