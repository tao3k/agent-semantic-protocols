//! Root semantic agent hook classifier over activated providers.

mod core;
mod decision;
#[path = "../classifier_inline_source_read.rs"]
mod inline_source_read;
#[path = "../classifier_recovery.rs"]
mod recovery;
mod source_access_routes;

pub use core::{HookClassificationRequest, classify_hook, classify_hook_with_config};
