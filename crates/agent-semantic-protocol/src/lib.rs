#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;
pub mod graph;
mod state_cli;

pub use cli::{run_cli_args, run_cli_from_env};
#[doc(hidden)]
pub use command::search_pipe_selector_seed::render_selector_seeded_search_pipe;
pub(crate) use command::{
    RegisteredSession, asp_explore_session_for_current_root, current_registered_session,
    has_current_agent_session,
};
pub use state_cli::run_binary_from_env;
