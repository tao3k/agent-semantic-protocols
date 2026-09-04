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
pub(super) use message::with_selector_only_subagent_message;
pub(super) use user_prompt::classify_user_prompt;
#[path = "../classifier_recovery.rs"]
mod recovery;

pub use command_decision_rebind::HookMatcherKeys;
pub use command_decision_rebind::ShellCommandKey;
pub use command_decision_rebind::hook_matcher_keys;
pub use command_decision_rebind::rebind_command_decision_to_payload;
pub use command_decision_rebind::rebind_command_decision_to_payload_with_keys;
pub use command_decision_rebind::rebind_direct_read_decision_to_payload;
pub use command_decision_rebind::shell_command_key;
pub use command_decision_rebind::shell_command_keys;
pub use core::DirectReadSourceKey;
pub use core::HookClassificationRequest;
pub use core::ShellReadSourceKey;
pub use core::classify_hook;
pub use core::classify_hook_with_config;
pub use core::direct_read_source_extension;
pub use core::direct_read_source_key;
pub use core::shell_read_source_key;
pub use core::shell_read_source_keys;
pub use recovery::HOOK_TRIGGER_PROMPT_FILE_NAME;
pub use recovery::default_hook_trigger_prompt_message;
pub use recovery::hook_trigger_prompt_document;
pub use recovery::materialize_hook_trigger_prompt_agent_flow_for_client;
pub use recovery::merge_hook_trigger_prompt_document;
pub use recovery::render_hook_trigger_prompt_document;
pub(crate) use recovery::shell_quote_arg;
mod decision;
