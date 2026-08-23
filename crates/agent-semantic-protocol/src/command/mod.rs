//! Command tree for the `asp` binary.

mod agent_session;

mod agent_config_sync;
mod agent_control_plane;
mod agent_window;
mod ast_patch;
mod cli_help;
mod dispatch;
mod dispatch_agent_session_policy;
mod document_provider;
mod gerbil_deps;
mod graph;
pub mod graph_turbo_resident_process;
mod healthcheck;
mod hook;
mod hook_break_glass;
mod hook_host_acceptance;
pub(crate) mod hook_runtime;
mod hook_runtime_context;
mod install_binary_config_admission;
mod install_provider;
mod install_provider_archive;
mod install_provider_development;
mod install_provider_reconcile;
mod install_provider_release;
mod install_provider_runtime_reconcile;
pub(crate) mod installed_provider_artifacts;
pub use install_provider_runtime_reconcile::prepare_runtime_server_provider_artifacts;
pub(crate) use install_provider_runtime_reconcile::reconcile_installed_provider_artifacts_for_runtime;

mod install_provider_target;
mod live_corpus;
mod managed_hook_config;
mod org_archive;
mod org_capture;
mod org_capture_contract_materialize;
pub(crate) mod org_capture_interactive;
mod org_recall;
mod paths;
pub(crate) mod protocol_binary;
mod protocol_version;
mod provider_dispatch;
mod provider_exact_args;
mod provider_execution;
mod provider_install_registry;
mod provider_resident_exact;
mod provider_roots;
mod provider_selector;
mod provider_usage;
mod root_language_facade;
mod runtime_server;
mod schema;
mod search_config;
mod search_owner_items;

pub(crate) use dispatch::{run_protocol_command, run_protocol_command_started};
pub(crate) use hook::evaluate_hook_event_locally;
pub(in crate::command) use hook_runtime_context::payload_indicates_subagent_context;
pub(in crate::command) use protocol_binary::{
    ProtocolBinaryInstallPlan, ensure_protocol_binary_installed_under_guard,
};
pub(in crate::command) use protocol_version::{
    protocol_version_line, run_protocol_version_command,
};
pub mod search_router_graph_state;
