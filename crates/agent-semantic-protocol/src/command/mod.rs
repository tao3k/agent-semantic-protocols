//! Command tree for the `asp` binary.

mod agent_session;

pub(crate) use agent_config_sync::synchronize_agent_config_from_project_root;
mod agent_config_sync;
mod agent_control_plane;
mod agent_window;
mod ast_patch;
mod cli_help;
mod client_backend_worker;
mod dispatch;
mod dispatch_agent_session_policy;
mod document_provider;
mod gerbil_check_cache;
mod gerbil_deps;
pub(crate) mod global_provider_catalog;
mod graph;
pub mod graph_turbo_resident_process;
mod healthcheck;
mod hook;
mod hook_break_glass;
mod hook_enforcement;
mod hook_host_acceptance;
pub(crate) mod hook_runtime;
mod hook_runtime_context;
pub(crate) mod hook_runtime_memory_inbox;
mod install_binary_config_admission;
mod install_provider;
mod install_provider_archive;
mod install_provider_development;
mod install_provider_reconcile;
mod install_provider_release;
mod install_provider_runtime_reconcile;
pub(crate) use install_provider_runtime_reconcile::reconcile_global_provider_catalog_for_runtime;
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
mod provider_activation;
mod provider_dispatch;
mod provider_exact_args;
mod provider_execution;
mod provider_fast_path;
mod provider_fast_search;
mod provider_owner_native;
mod provider_process;
mod provider_resident_exact;
mod provider_roots;
mod provider_selector;
mod provider_usage;
mod root_language_facade;
mod runtime_server;
#[cfg(test)]
#[path = "../../tests/unit/command/runtime_server_agent_facing_wall_budget.rs"]
mod runtime_server_agent_facing_wall_budget;
mod search_config;
mod search_dependency_seed;
mod search_failure_render;
mod search_pipe;
mod search_pipe_action_frontier;
mod search_pipe_action_model;
mod search_pipe_actions;
mod search_pipe_args;
mod search_pipe_candidates;
mod search_pipe_dependency_seed_cache;
mod search_pipe_evidence_projection;
mod search_pipe_failure;
mod search_pipe_graph_nodes;
mod search_pipe_graph_turbo;
mod search_pipe_meta;
mod search_pipe_model;
mod search_pipe_owner_items;
mod search_pipe_plan;
mod search_pipe_projection;
mod search_pipe_provider_facts;
mod search_pipe_quality;
mod search_pipe_quality_model;
mod search_pipe_query_evidence;
mod search_pipe_query_model;
mod search_pipe_query_pack;
mod search_pipe_read_memory;
mod search_pipe_render;
pub(super) mod search_pipe_selector_seed;
mod search_pipe_source;
mod search_pipe_surfaces;
mod search_pipe_view;
mod search_query_budget;
mod search_suggest;
mod source_access;
mod tree_sitter_query_diagnostics;
mod workspace_tree_sitter_inventory;
mod workspace_tree_sitter_query;
mod workspace_tree_sitter_query_trace;

pub(crate) use dispatch::{run_protocol_command, run_protocol_command_started};
pub(crate) use hook::evaluate_hook_event_locally;
pub(in crate::command) use hook_enforcement::codex_enforcement_report;
pub(in crate::command) use hook_runtime_context::payload_indicates_subagent_context;
pub(in crate::command) use protocol_binary::{
    ProtocolBinaryInstallPlan, ensure_protocol_binary_installed,
    protocol_binary_artifact_path_digest, protocol_binary_contract_fingerprint,
    protocol_binary_in_codex_hook_shell,
};
pub(in crate::command) use protocol_version::{
    protocol_version_line, run_protocol_version_command,
};
pub mod search_router_graph_state;
