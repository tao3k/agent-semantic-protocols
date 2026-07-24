#![deny(dead_code)]

#[path = "unit/classifier/mod.rs"]
mod classifier;

#[path = "unit/active_artifact_receipt.rs"]
mod active_artifact_receipt;

#[path = "unit/client_hook_config.rs"]
mod client_hook_config;
#[path = "unit/read_only_subagent.rs"]
mod read_only_subagent;

#[path = "unit/codex_config.rs"]
mod codex_config;

#[path = "unit/command.rs"]
mod command;
#[path = "unit/command_apply_patch.rs"]
mod command_apply_patch;
#[path = "unit/command_shell.rs"]
mod command_shell;

#[path = "unit/event_state.rs"]
mod event_state;
#[path = "unit/event_state_subagent_model_drift.rs"]
mod event_state_subagent_model_drift;

#[path = "unit/provider_manifest/mod.rs"]
mod provider_manifest;

#[path = "unit/rust_harness_activation/mod.rs"]
mod rust_harness_activation;
#[path = "unit/rust_project_harness_gate.rs"]
mod rust_project_harness_gate;
#[path = "unit/source_access.rs"]
mod source_access;
