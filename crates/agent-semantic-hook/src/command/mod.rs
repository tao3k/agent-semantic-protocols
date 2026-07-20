//! Shell command normalization and semantic-search routing helpers.

mod apply_patch;
mod asp;
mod intent;
mod provider_candidates;
mod query;
mod search_json;
mod shell;
mod source_intent;

pub(crate) use apply_patch::apply_patch_source_paths;
pub use asp::{
    AspLanguageCommand, asp_invocation_indices, classify_asp_language_command_tokens,
    classify_asp_language_command_tokens_with_policy,
};
pub(crate) use intent::{CommandIntent, command_intent};
pub(crate) use provider_candidates::{command_source_paths, path_like_token_matches};
pub(crate) use query::infer_query_from_path;
pub(crate) use search_json::search_json_route;
pub use shell::semantic_shell_tokens;
pub use source_intent::{SourceCommandIntent, classify_source_command_intent};
