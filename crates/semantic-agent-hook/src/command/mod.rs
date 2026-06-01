//! Shell command normalization and semantic-search routing helpers.

mod intent;
mod profiles;
mod raw_search;
mod search_json;
mod shell;

pub(crate) use intent::{CommandIntent, command_intent};
pub(crate) use profiles::path_like_tokens;
pub(crate) use raw_search::profiles_for_raw_search;
pub(crate) use search_json::{contains_ingest_pipe, search_json_route};
pub(crate) use shell::semantic_shell_tokens;
