//! Shared projection helpers for search-pipe candidates.

use super::{document_provider, search_pipe_model::Candidate};

pub(super) fn is_document_language(language_id: &str) -> bool {
    document_provider::is_document_language(language_id)
}

pub(super) fn candidate_executable_selector(candidate: &Candidate) -> Option<String> {
    candidate.selector.clone()
}

pub(super) fn query_projection_kind(language_id: &str) -> &'static str {
    if is_document_language(language_id) {
        "content"
    } else {
        "code"
    }
}

pub(super) fn query_projection_flag(language_id: &str) -> &'static str {
    if query_projection_kind(language_id) == "content" {
        "--content"
    } else {
        "--code"
    }
}
