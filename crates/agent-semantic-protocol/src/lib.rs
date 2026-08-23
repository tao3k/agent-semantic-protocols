#![deny(dead_code)]

//! Public facade for the `asp` CLI.

mod cli;
mod command;
pub use command::prepare_runtime_server_provider_artifacts;
pub mod session_control_plane;
pub use command::graph_turbo_resident_process::{
    GraphTurboResidentLaunchSpec, GraphTurboResidentProcess,
};
pub use command::search_router_graph_state;
mod agent_session_choice_state;
mod exact_projection_diagnostic;
mod exact_projection_diagnostic_io;
mod exact_projection_trace;
mod hook_break_glass;
mod multi_agent_session;
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
pub use command::protocol_binary::{
    publish_runtime_server_artifact, published_runtime_server_artifact_digest,
};
#[doc(hidden)]
pub use state_cli::run_binary_from_env;
pub(crate) mod codex;
