//! Root semantic agent hook classifier over activated providers.

mod agent_org_artifacts;
mod command_decision_rebind;
mod core;
pub(crate) use core::{
    default_allow_for_normalized_action, materialize_agent_search_json_decision,
    materialize_apply_patch_decision, materialize_source_access_decision,
};
mod decision;
#[path = "../classifier_recovery.rs"]
mod recovery;
mod source_access_routes;

pub use command_decision_rebind::{
    ShellCommandKey, rebind_command_decision_to_payload, shell_command_key,
};
pub use core::{
    DirectReadSourceKey, HookClassificationRequest, ShellReadSourceKey,
    asp_no_agent_passthrough_decision, asp_no_agent_passthrough_requested, classify_hook,
    classify_hook_with_config, direct_read_source_extension, direct_read_source_key,
    shell_read_source_key,
};
pub use recovery::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, default_hook_trigger_prompt_message,
    hook_trigger_prompt_document, materialize_hook_trigger_prompt_agent_flow_for_client,
    merge_hook_trigger_prompt_document, render_hook_trigger_prompt_document,
};
