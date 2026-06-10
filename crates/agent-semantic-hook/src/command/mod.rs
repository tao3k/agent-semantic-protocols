//! Shell command normalization and semantic-search routing helpers.

mod apply_patch;
mod intent;
mod provider_candidates;
mod query;
mod raw_search;
mod search_json;
mod shell;

pub(crate) use shell::looks_like_command_transcript;

pub(crate) use apply_patch::apply_patch_source_paths;
pub(crate) use intent::{CommandIntent, command_intent};
pub(crate) use provider_candidates::path_like_tokens;
pub(crate) use query::{
    infer_query_from_path, search_query_route, search_query_route_for_selector,
    selector_query_route,
};
pub(crate) use raw_search::raw_search_plan;
pub(crate) use search_json::{contains_ingest_pipe, search_json_route};
pub(crate) use shell::semantic_shell_tokens;
