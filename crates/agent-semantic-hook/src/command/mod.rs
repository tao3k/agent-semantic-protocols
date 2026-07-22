//! Shell command normalization and semantic-search routing helpers.

mod apply_patch;

mod query;
mod search_json;
mod shell;

pub(crate) use apply_patch::apply_patch_source_paths;
pub(crate) use query::infer_query_from_path;
pub(crate) use search_json::search_json_route;
pub use shell::semantic_shell_tokens;
