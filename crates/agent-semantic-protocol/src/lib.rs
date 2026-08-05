#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;
pub use command::graph_turbo_resident_process::{
    GraphTurboResidentLaunchSpec, GraphTurboResidentProcess, admit_candidate_rank_receipt,
};
pub use command::search_router_graph_state;
mod exact_projection_diagnostic;
mod exact_projection_diagnostic_io;
mod exact_projection_trace;
mod resident_exact_projection;
pub(crate) mod server;
pub use agent_semantic_context_product as context_product_state;

pub mod agent_session_lifecycle_projection;
pub mod codex_multi_agent_v2_control_plane;
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
    SelectorSeedCursorBinding, SelectorSeedCursorError, SelectorSeededSearchCursorRequest,
    SelectorSeededSearchPipeRequest, render_selector_seeded_search_pipe,
    selector_seeded_search_cursor,
};
