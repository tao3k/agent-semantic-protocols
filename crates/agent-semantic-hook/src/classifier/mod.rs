//! Root semantic agent hook classifier over activated providers.

mod agent_org_artifacts;
mod agent_search_json;
mod apply_patch_policy;
mod candidate;
mod command_decision_rebind;
mod core;
mod message;
mod user_prompt;
pub(super) use candidate::higher_priority_candidate;
pub(crate) use core::{default_allow_for_normalized_action, materialize_apply_patch_decision};
pub(super) use message::with_selector_only_subagent_message;
pub(super) use user_prompt::classify_user_prompt;
mod decision;
#[path = "../classifier_recovery.rs"]
mod recovery;
mod source_access_routes;
pub(crate) use agent_search_json::materialize_agent_search_json_decision;
pub use core::runtime_binary_policy_decision_v1;
pub(crate) use source_access_routes::{
    direct_read_language_ids, materialize_source_access_decision, policy_direct_read_routes,
};

pub use command_decision_rebind::{
    ShellCommandKey, rebind_command_decision_to_payload, shell_command_key,
};
pub use core::{
    DirectReadSourceKey, HookClassificationRequest, ShellReadSourceKey, classify_hook,
    classify_hook_with_config, direct_read_source_extension, direct_read_source_key,
    shell_read_source_key, shell_read_source_keys,
};
pub use recovery::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, default_hook_trigger_prompt_message,
    hook_trigger_prompt_document, materialize_hook_trigger_prompt_agent_flow_for_client,
    merge_hook_trigger_prompt_document, render_hook_trigger_prompt_document,
};
