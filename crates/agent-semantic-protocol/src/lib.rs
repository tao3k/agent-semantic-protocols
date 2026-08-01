#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;
mod exact_projection_diagnostic;
mod exact_projection_diagnostic_io;
mod exact_projection_trace;
mod resident_exact_projection;
pub use agent_semantic_context_product as context_product_state;

pub mod exact_projection;
pub mod graph;
mod state_cli;

pub use cli::{run_cli_args, run_cli_from_env};
#[doc(hidden)]
pub mod hook_bootstrap;
#[doc(hidden)]
pub use state_cli::run_binary_from_env;
pub(crate) mod codex;
pub use command::search_pipe_selector_seed::{
    SelectorSeededSearchPipeRequest, render_selector_seeded_search_pipe,
};
