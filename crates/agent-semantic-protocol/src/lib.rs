#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;
pub mod session_control_plane;
pub use command::search_router_graph_state;
mod agent_session_choice_state;
mod hook_break_glass;
mod multi_agent_session;
pub(crate) mod server;
pub use agent_semantic_context_product as context_product_state;

pub mod agent_session_lifecycle_projection;
pub mod codex_multi_agent_v2_control_plane;
pub mod exact_projection;
pub mod graph;
mod state_cli;

pub use cli::{run_cli_args, run_cli_from_env};
pub mod cli_failure;
#[doc(hidden)]
pub mod hook_bootstrap;
pub use command::protocol_binary::{
    publish_runtime_server_artifact, published_runtime_server_artifact_digest,
};
#[doc(hidden)]
pub use state_cli::run_binary_from_env;
pub(crate) mod codex;
