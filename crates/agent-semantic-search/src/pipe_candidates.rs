//! Search pipe candidate orchestration.

use std::path::{Path, PathBuf};

use crate::{
    DynamicSearchCandidate, DynamicSearchRootCandidateRequest,
    collect_dynamic_lexical_overlay_candidates_from_roots, language_file_spec,
};

/// Candidate returned by the search pipe candidate service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeCandidate {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub symbol: String,
    pub text: String,
    pub source: String,
    pub confidence: String,
}

/// Request for `search pipe` candidate collection.
pub struct SearchPipeCandidateRequest<'a> {
    pub language_id: &'a str,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub query: &'a str,
    pub owners: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub require_multi_clause: bool,
    pub limit: usize,
}

/// Collect search pipe candidates without leaking routing logic into CLI code.
pub fn collect_search_pipe_candidates(
    request: SearchPipeCandidateRequest<'_>,
) -> Result<Vec<SearchPipeCandidate>, String> {
    let terms = query_terms(request.query);
    if terms.is_empty() {
        return Err("search pipe requires a non-empty query".to_string());
    }
    if request.require_multi_clause && terms.len() < 2 {
        return Err(
            "search pipe requires at least two query clauses; use search lexical for plain text search or search owner <path> items --query <terms>"
                .to_string(),
        );
    }

    let file_spec = language_file_spec(request.language_id);
    collect_dynamic_lexical_overlay_candidates_from_roots(DynamicSearchRootCandidateRequest {
        project_root: request.project_root,
        locator_root: request.locator_root,
        terms: &terms,
        owners: request.owners,
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        file_matches: &|path| file_spec.matches(path),
        limit: request.limit,
    })
    .map(|candidates| {
        candidates
            .into_iter()
            .map(SearchPipeCandidate::from)
            .collect()
    })
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

impl From<crate::SearchOverlayCandidate> for SearchPipeCandidate {
    fn from(candidate: crate::SearchOverlayCandidate) -> Self {
        Self {
            path: candidate.path,
            line: candidate.line,
            end_line: candidate.end_line,
            symbol: candidate.symbol,
            text: candidate.text,
            source: candidate.source,
            confidence: candidate.confidence,
        }
    }
}

impl From<DynamicSearchCandidate> for SearchPipeCandidate {
    fn from(candidate: DynamicSearchCandidate) -> Self {
        Self {
            path: candidate.path,
            line: candidate.line,
            end_line: candidate.end_line,
            symbol: candidate.symbol,
            text: candidate.text,
            source: candidate.source,
            confidence: candidate.confidence,
        }
    }
}
