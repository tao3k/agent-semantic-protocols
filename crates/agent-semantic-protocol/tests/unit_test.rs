#![recursion_limit = "256"]
#![deny(dead_code)]

#[path = "unit/command/build_profile.rs"]
mod command_build_profile;

#[path = "unit/agent_session_lifecycle_projection.rs"]
mod agent_session_lifecycle_projection;
#[path = "unit/ast_patch.rs"]
mod ast_patch;
#[path = "unit/client_hook_claude_smoke.rs"]
mod client_hook_claude_smoke;
#[path = "unit/client_hook_codex_cli_e2e.rs"]
mod client_hook_codex_cli_e2e;
#[path = "unit/client_hook_config.rs"]
mod client_hook_config;
#[path = "unit/client_hook_config_doctor/mod.rs"]
mod client_hook_config_doctor;
#[path = "unit/client_hook_config_runtime.rs"]
mod client_hook_config_runtime;
#[path = "unit/client_hook_desktop_smoke/mod.rs"]
mod client_hook_desktop_smoke;
#[path = "unit/codex/mod.rs"]
mod codex;
#[path = "unit/codex_multi_agent_v2_control_plane.rs"]
mod codex_multi_agent_v2_control_plane;
#[path = "unit/codex_plugin_install.rs"]
mod codex_plugin_install;
#[path = "unit/command/ascent_search_router_graph_state.rs"]
mod command_ascent_search_router_graph_state;
#[path = "unit/command/dispatch_agent_session_policy.rs"]
mod command_dispatch_agent_session_policy;
#[path = "unit/command/gerbil_check_cache.rs"]
mod command_gerbil_check_cache;
#[path = "unit/command/global_provider_catalog.rs"]
mod command_global_provider_catalog;
#[path = "unit/command/graph_turbo_resident_runtime.rs"]
mod command_graph_turbo_resident_runtime;
#[path = "unit/command/search_router_graph_state.rs"]
mod command_search_router_graph_state;
#[path = "unit/context_product_state.rs"]
mod context_product_state;
#[path = "unit/document_owner_items_hot_path.rs"]
mod document_owner_items_hot_path;
#[path = "unit/document_provider.rs"]
mod document_provider;
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
#[path = "unit/hook_runtime_skill.rs"]
mod hook_runtime_skill;
#[path = "unit/install_provider_cli.rs"]
mod install_provider_cli;
#[path = "../../agent-semantic-hook/tests/unit/integration_fixture.rs"]
mod integration_fixture;
#[path = "unit/paths_command.rs"]
mod paths_command;
#[path = "unit/provider_command/mod.rs"]
mod provider_command;
#[path = "unit/provider_exact_diagnostic.rs"]
mod provider_exact_diagnostic;
#[path = "unit/provider_exact_query_args.rs"]
mod provider_exact_query_args;
#[path = "unit/command/provider_language_facade.rs"]
mod provider_language_facade;
#[path = "unit/provider_manifest_scope.rs"]
mod provider_manifest_scope;
#[path = "unit/provider_selector.rs"]
mod provider_selector;
#[path = "unit/query_owner_freshness.rs"]
mod query_owner_freshness;
#[path = "unit/rfc_search_frame.rs"]
mod rfc_search_frame;
#[path = "unit/rs_harness_attribute.rs"]
mod rs_harness_attribute;
#[path = "unit/runtime_server_query_purity.rs"]
mod runtime_server_query_purity;
#[path = "unit/runtime_server_singleton_socket.rs"]
mod runtime_server_singleton_socket;
#[path = "../../agent-semantic-hook/tests/unit/rust_harness_activation/mod.rs"]
mod rust_harness_activation;
#[path = "unit/sandtable_fixtures.rs"]
mod sandtable_fixtures;
#[path = "unit/scenario_performance_gate.rs"]
mod scenario_performance_gate;
#[path = "unit/source_access_command/mod.rs"]
mod source_access_command;
#[path = "unit/state_home_fixture.rs"]
mod state_home_fixture;
#[path = "unit/tree_sitter_query_diagnostics.rs"]
mod tree_sitter_query_diagnostics;
#[path = "unit/unit_state_home_fixture.rs"]
mod unit_state_home_fixture;
#[path = "unit/workspace_tree_sitter_query_diagnostics.rs"]
mod workspace_tree_sitter_query_diagnostics;
