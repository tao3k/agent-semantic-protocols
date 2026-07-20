//! Adapter between `SearchPipe` source acquisition and lexical `SearchFrame` routing.

use crate::{
    LexicalSearchFrameCandidate, LexicalSearchFrameRequest, LexicalSearchFrameRoute,
    SearchPipeCandidate,
    pipe_source::{
        SearchPipeSourceAcquisitionTrace, SearchPipeSourceIndexAcquisition,
        SearchPipeSourceIndexDecision,
    },
};

pub(crate) fn plan_pipe_lexical_search_frame(
    query: &str,
    source_index: Option<&SearchPipeSourceIndexAcquisition>,
) -> LexicalSearchFrameRoute {
    let warm_candidates = source_index
        .filter(|source_index| {
            source_index.decision == SearchPipeSourceIndexDecision::UseAndSkipSearchOverlay
        })
        .map(|source_index| lexical_frame_candidates(&source_index.candidates))
        .unwrap_or_default();
    let owner_source_candidates = source_index
        .filter(|source_index| source_index.decision == SearchPipeSourceIndexDecision::DeferBackend)
        .map(source_index_owner_candidates)
        .unwrap_or_default();
    let owner_candidates = lexical_frame_candidates(&owner_source_candidates);
    let terms = lexical_search_frame_terms(query);
    crate::plan_lexical_search_frame(LexicalSearchFrameRequest {
        terms: &terms,
        warm_candidates: &warm_candidates,
        session_candidates: &[],
        owner_candidates: &owner_candidates,
        provider_owner_item_available: false,
        cold_scan_allowed: true,
    })
}

pub(crate) fn lexical_search_frame_trace(
    route: &LexicalSearchFrameRoute,
) -> SearchPipeSourceAcquisitionTrace {
    SearchPipeSourceAcquisitionTrace {
        source: "lexical-search-frame".to_string(),
        status: route.render_receipt(),
        matched: route.selected_candidate_count,
        missing: usize::from(route.fallback_reason != "none"),
        normalized: route.selected_candidate_count,
        elapsed: None,
        source_snapshot: None,
        artifact_digest: None,
    }
}

fn lexical_frame_candidates(
    candidates: &[SearchPipeCandidate],
) -> Vec<LexicalSearchFrameCandidate> {
    candidates
        .iter()
        .map(LexicalSearchFrameCandidate::from)
        .collect()
}

pub(crate) fn source_index_owner_evidence_candidates(
    source_index: &SearchPipeSourceIndexAcquisition,
) -> Vec<SearchPipeCandidate> {
    if source_index.decision != SearchPipeSourceIndexDecision::DeferBackend {
        return Vec::new();
    }
    source_index_owner_candidates(source_index)
}

fn source_index_owner_candidates(
    source_index: &SearchPipeSourceIndexAcquisition,
) -> Vec<SearchPipeCandidate> {
    source_index
        .candidates
        .iter()
        .filter(|candidate| {
            !matches!(
                candidate.confidence.as_str(),
                "invalid-selector" | "stale-index"
            )
        })
        .cloned()
        .collect()
}

fn lexical_search_frame_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(ToString::to_string)
        .collect()
}
