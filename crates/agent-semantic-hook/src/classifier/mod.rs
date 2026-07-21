//! Root semantic agent hook classifier over activated providers.

mod agent_org_artifacts;
mod core;
pub(crate) use core::{
    materialize_agent_search_json_decision, materialize_apply_patch_decision,
    materialize_source_access_decision,
};
pub(crate) use prompt_search_flow::materialize_prompt_search_strategy_decision;
mod decision;
mod prompt_search_flow;
#[path = "../classifier_recovery.rs"]
mod recovery;
mod source_access_routes;

pub use core::{HookClassificationRequest, classify_hook, classify_hook_with_config};
pub use recovery::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, default_hook_trigger_prompt_message,
    hook_trigger_prompt_document, materialize_hook_trigger_prompt_agent_flow_for_client,
    merge_hook_trigger_prompt_document, render_hook_trigger_prompt_document,
};
