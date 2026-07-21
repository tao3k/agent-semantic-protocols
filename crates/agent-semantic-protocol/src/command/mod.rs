//! Command tree for the `asp` binary.

mod agent_session;
mod agent_session_registry;

pub(crate) use agent_session_registry::{
    ResidentChildIdentityProof, codex_transcript_resident_child_identity, current_agent_session_id,
    current_registered_session, current_registered_session_identity,
    current_resident_child_identity_proof, current_root_session_id, has_current_agent_session,
    record_current_session_tool_event, registered_resident_session_for_root,
    rollout_metadata_matches_managed_agent_profile, validate_session_profile,
};
pub(crate) use org_capture::run_org_state_sync;
mod ast_patch;
mod client_backend_worker;
mod dispatch;
mod dispatch_agent_session_policy;
mod document_language_facade;
mod document_provider;
mod gerbil_check_cache;
mod gerbil_deps;
mod graph;
mod healthcheck;
mod hook;
mod hook_enforcement;
mod hook_runtime;
mod hook_runtime_context;
mod hook_runtime_source_access;
mod install_provider;
mod install_provider_archive;
mod install_provider_release;
mod install_provider_target;
mod install_provider_workspace_artifact;
mod install_provider_workspace_cas;
mod install_provider_workspace_descriptor;
mod install_provider_workspace_source;
mod language_owner_items;
mod language_projection_import;
mod org_archive;
mod org_capture;
mod org_capture_contract_materialize;
mod org_capture_interactive;
mod org_recall;
mod paths;
mod protocol_binary;
mod protocol_version;
mod provider_activation;
mod provider_dispatch;
mod provider_execution;
mod provider_fast_path;
mod provider_fast_search;
mod provider_process;
mod provider_roots;
mod provider_selector;
mod provider_usage;
mod query_direct_read;
pub(crate) mod query_owner;
mod root_language_facade;
mod search_config;
mod search_dependency_seed;
mod search_failure_render;
mod search_pipe;
mod search_pipe_action_frontier;
mod search_pipe_action_model;
mod search_pipe_actions;
mod search_pipe_args;
mod search_pipe_candidates;
mod search_pipe_dependency_facts;
mod search_pipe_dependency_seed_cache;
mod search_pipe_evidence_projection;
mod search_pipe_failure;
mod search_pipe_graph_nodes;
mod search_pipe_graph_turbo;
mod search_pipe_graph_turbo_owner_rank;
mod search_pipe_graph_turbo_seed;
mod search_pipe_meta;
mod search_pipe_model;
mod search_pipe_owner_items_fast;
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
mod search_pipe_seed_decision;
pub(super) mod search_pipe_selector_seed;
mod search_pipe_source;
mod search_pipe_surfaces;
mod search_pipe_view;
mod search_query_budget;
mod search_suggest;
mod source_access;
mod sync;

pub(crate) use dispatch::run_protocol_command;
pub(in crate::command) use hook_enforcement::codex_enforcement_report;
pub(in crate::command) use hook_runtime_context::payload_indicates_subagent_context;
pub(in crate::command) use protocol_binary::{
    ensure_protocol_binary_installed_for_path, protocol_binary_on_path,
};
pub(in crate::command) use protocol_version::{
    protocol_version_line, run_protocol_version_command,
};
