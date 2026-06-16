//! Shared projection helpers for search-pipe candidates.

use super::{document_provider, search_pipe_model::Candidate};

pub(super) fn is_document_language(language_id: &str) -> bool {
    document_provider::is_document_language(language_id)
}

pub(super) fn candidate_end_line(candidate: &Candidate) -> usize {
    candidate.end_line.max(candidate.line)
}

pub(super) fn candidate_selector(language_id: &str, candidate: &Candidate) -> String {
    let end_line = candidate_end_line(candidate);
    if is_document_language(language_id) {
        format!("{}:{}-{end_line}", candidate.path, candidate.line)
    } else {
        format!("{}:{}:{end_line}", candidate.path, candidate.line)
    }
}

pub(super) fn graph_projection_action(language_id: &str) -> &'static str {
    query_projection_kind(language_id)
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
