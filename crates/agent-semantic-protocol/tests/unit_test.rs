#![recursion_limit = "256"]
#![deny(dead_code)]

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
#[path = "unit/codex_plugin_install.rs"]
mod codex_plugin_install;
#[path = "unit/command/search_pipe_gerbil_owner_items.rs"]
mod command_search_pipe_gerbil_owner_items;
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
#[path = "unit/rs_harness_attribute.rs"]
mod rs_harness_attribute;
#[path = "../../agent-semantic-hook/tests/unit/rust_harness_activation/mod.rs"]
mod rust_harness_activation;
#[path = "unit/source_access_command/mod.rs"]
mod source_access_command;
#[path = "unit/sync_command.rs"]
mod sync_command;
