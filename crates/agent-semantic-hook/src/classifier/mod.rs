// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Root semantic agent hook classifier over activated providers.

mod agent_org_artifacts;
mod candidate;
mod command_decision_rebind;
mod core;
mod message;
mod user_prompt;
pub(super) use candidate::higher_priority_candidate;
pub(crate) use core::default_allow_for_normalized_action;
pub use message::materialize_source_access_deny_message;
pub(super) use message::with_executable_evidence_subagent_message;
pub(super) use user_prompt::classify_user_prompt;

pub use command_decision_rebind::ShellCommandKey;
pub use command_decision_rebind::rebind_command_decision_to_payload;
pub use command_decision_rebind::rebind_command_decision_to_payload_with_keys;
pub use command_decision_rebind::shell_command_key;
pub use command_decision_rebind::shell_command_keys;
pub use core::HookClassificationRequest;
pub use core::ShellReadSourceKey;
pub use core::classify_hook;
pub use core::classify_hook_with_config;
pub use core::shell_read_source_key;
pub use core::shell_read_source_keys;
mod decision;
