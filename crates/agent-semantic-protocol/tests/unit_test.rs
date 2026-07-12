#![recursion_limit = "256"]
#![deny(dead_code)]

#[path = "unit/command/agent_session_registry_control_plane.rs"]
mod agent_session_registry_control_plane;
#[path = "unit/command/agent_session_registry_resume_cli.rs"]
mod agent_session_registry_resume_cli;
#[path = "unit/command/agent_session_registry_resume_model_create_cli.rs"]
mod agent_session_registry_resume_model_create_cli;
#[path = "unit/command/build_profile.rs"]
mod command_build_profile;

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
#[path = "unit/codex_plugin_install.rs"]
mod codex_plugin_install;
#[path = "unit/command/agent_session_registry_render.rs"]
mod command_agent_session_registry_render;
#[path = "unit/command/dispatch_agent_session_policy.rs"]
mod command_dispatch_agent_session_policy;
#[path = "unit/command/gerbil_check_cache.rs"]
mod command_gerbil_check_cache;
#[path = "unit/command/search_pipe_evidence_projection.rs"]
mod command_search_pipe_evidence_projection;
#[path = "unit/command/search_pipe_projection.rs"]
mod command_search_pipe_projection;
#[path = "unit/document_owner_items_hot_path.rs"]
mod document_owner_items_hot_path;
#[path = "unit/document_provider.rs"]
mod document_provider;
#[path = "unit/graph_render.rs"]
mod graph_render;
#[path = "unit/healthcheck.rs"]
mod healthcheck;
#[path = "unit/hook_command.rs"]
mod hook_command;
#[path = "unit/hook_paths.rs"]
mod hook_paths;
#[path = "unit/hook_runtime_skill.rs"]
mod hook_runtime_skill;
#[path = "unit/install_provider_cli.rs"]
mod install_provider_cli;
#[path = "unit/paths_command.rs"]
mod paths_command;
#[path = "unit/provider_command/mod.rs"]
mod provider_command;
#[path = "unit/command/provider_language_facade.rs"]
mod provider_language_facade;
#[path = "unit/query_owner_freshness.rs"]
mod query_owner_freshness;
#[path = "unit/rfc_search_frame.rs"]
mod rfc_search_frame;
#[path = "unit/rs_harness_attribute.rs"]
mod rs_harness_attribute;
#[path = "../../agent-semantic-hook/tests/unit/rust_harness_activation/mod.rs"]
mod rust_harness_activation;
#[path = "unit/sandtable_fixtures.rs"]
mod sandtable_fixtures;
#[path = "unit/scenario_performance_gate.rs"]
mod scenario_performance_gate;
#[path = "unit/source_access_command/mod.rs"]
mod source_access_command;
#[path = "unit/sync_command.rs"]
mod sync_command;
