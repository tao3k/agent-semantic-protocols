#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

#[path = "unit/classifier/mod.rs"]
mod classifier;

#[path = "unit/active_artifact_receipt.rs"]
mod active_artifact_receipt;

#[path = "unit/client_hook_config.rs"]
mod client_hook_config;
#[path = "unit/decision_message.rs"]
mod decision_message;

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

#[path = "unit/protocol_roundtrip.rs"]
mod protocol_roundtrip;

#[path = "unit/hook_recovery_admission.rs"]
mod hook_recovery_admission;
#[path = "unit/hook_workspace_candidate.rs"]
mod hook_workspace_candidate;
#[path = "unit/match_policy_contract.rs"]
mod match_policy_contract;
#[path = "unit/test_process_env.rs"]
mod test_process_env;
#[path = "unit/tool_action.rs"]
mod tool_action;
