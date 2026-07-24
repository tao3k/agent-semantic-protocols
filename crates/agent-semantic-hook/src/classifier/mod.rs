//! Root semantic agent hook classifier over activated providers.

mod agent_org_artifacts;
mod core;
pub(crate) use core::{
    materialize_agent_search_json_decision, materialize_apply_patch_decision,
    materialize_source_access_decision,
};
mod decision;
#[path = "../classifier_recovery.rs"]
mod recovery;
pub(crate) use recovery::command_line;
mod source_access_routes;

pub use core::{HookClassificationRequest, classify_hook, classify_hook_with_config};
pub use recovery::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, default_hook_trigger_prompt_message,
    hook_trigger_prompt_document, materialize_hook_trigger_prompt_agent_flow_for_client,
    merge_hook_trigger_prompt_document, render_hook_trigger_prompt_document,
};
