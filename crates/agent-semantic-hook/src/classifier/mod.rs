//! Root semantic agent hook classifier over activated providers.

mod agent_org_artifacts;
mod core;
mod decision;
#[path = "../classifier_inline_source_read.rs"]
mod inline_source_read;
mod prompt_search_flow;
#[path = "../classifier_recovery.rs"]
mod recovery;
mod source_access_routes;

pub use core::{HookClassificationRequest, classify_hook, classify_hook_with_config};
pub use recovery::{
    HOOK_TRIGGER_PROMPT_FILE_NAME, default_hook_trigger_prompt_message,
    hook_trigger_prompt_document, merge_hook_trigger_prompt_document,
    render_hook_trigger_prompt_document,
};
