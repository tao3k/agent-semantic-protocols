#![recursion_limit = "256"]
#![deny(dead_code)]

#[path = "unit/command/build_profile.rs"]
mod command_build_profile;

#[path = "unit/agent_config_sync.rs"]
mod agent_config_sync;
#[path = "unit/agent_session_lifecycle_projection.rs"]
mod agent_session_lifecycle_projection;
#[path = "unit/ast_patch.rs"]
mod ast_patch;
#[path = "unit/codex/mod.rs"]
mod codex;
#[path = "unit/codex_multi_agent_v2_control_plane.rs"]
mod codex_multi_agent_v2_control_plane;
#[path = "unit/command/ascent_search_router_graph_state.rs"]
mod command_ascent_search_router_graph_state;
#[path = "unit/command/dispatch_agent_session_policy.rs"]
mod command_dispatch_agent_session_policy;
#[path = "unit/command/search_router_graph_state.rs"]
mod command_search_router_graph_state;
#[path = "unit/context_product_state.rs"]
mod context_product_state;
#[path = "unit/document_owner_items_hot_path.rs"]
mod document_owner_items_hot_path;
#[path = "unit/exact_projection.rs"]
mod exact_projection;
#[path = "unit/graph_render.rs"]
mod graph_render;
#[path = "unit/healthcheck.rs"]
mod healthcheck;
#[path = "unit/hook_command.rs"]
mod hook_command;
#[path = "unit/hook_execution_plane.rs"]
mod hook_execution_plane;
#[path = "unit/hook_paths.rs"]
mod hook_paths;
#[path = "unit/install_binary_existing_state_production_command.rs"]
mod install_binary_existing_state_production_command;
#[path = "unit/install_binary_production_command.rs"]
mod install_binary_production_command;
#[path = "unit/install_binary_test_guard.rs"]
mod install_binary_test_guard;
#[path = "unit/install_provider_cli.rs"]
mod install_provider_cli;
#[path = "unit/command/live_corpus_public_route.rs"]
mod live_corpus_public_route;
#[path = "unit/live_corpus_registered_languages.rs"]
mod live_corpus_registered_languages;
#[path = "unit/paths_command.rs"]
mod paths_command;
#[path = "unit/projection_presentation.rs"]
mod projection_presentation;
#[path = "unit/command/provider_language_facade.rs"]
mod provider_language_facade;
#[path = "unit/provider_root_profile.rs"]
mod provider_root_profile;
#[path = "unit/rfc_search_frame.rs"]
mod rfc_search_frame;
#[path = "unit/runtime_server_start_readiness.rs"]
mod runtime_server_start_readiness;
#[path = "unit/sandtable_fixtures.rs"]
mod sandtable_fixtures;
