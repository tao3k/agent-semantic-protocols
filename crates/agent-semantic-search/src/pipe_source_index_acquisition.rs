//! Snapshot-keyed source-index acquisition for the search pipe.

use std::path::{Path, PathBuf};

use crate::SearchPipeCandidate;
use crate::pipe_source::intent_terms_all_path_like;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexCandidate {
    pub path: String,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
    pub source_kind: String,
    pub line_count: Option<u32>,
    pub query_keys: Vec<String>,
    pub selector_proof: Option<SearchPipeSelectorPayloadProof>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSelectorPayloadProof {
    pub structural_selector: String,
    pub payload_kind: String,
    pub bounded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexLookup {
    pub state: String,
    pub candidates: Vec<SearchPipeSourceIndexCandidate>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub index_artifact_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexGate {
    pub term_count: usize,
    pub generic_term_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPipeSourceIndexDecision {
    QueryGate,
    DeferBackend,
    UseAndSkipSearchOverlay,
    Busy,
    ColdRequired,
    Fallthrough,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexAcquisition {
    pub decision: SearchPipeSourceIndexDecision,
    pub gate: Option<SearchPipeSourceIndexGate>,
    pub candidates: Vec<SearchPipeCandidate>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub index_artifact_digest: Option<String>,
}

pub struct SearchPipeSourceIndexAcquisitionRequest<'a> {
    pub intent: &'a str,
    pub project_root: &'a Path,
    pub scopes: &'a [PathBuf],
    pub lookup: Option<&'a SearchPipeSourceIndexLookup>,
}

pub fn collect_search_pipe_source_index_acquisition(
    request: SearchPipeSourceIndexAcquisitionRequest<'_>,
) -> Option<SearchPipeSourceIndexAcquisition> {
    if !request.scopes.is_empty() {
        return None;
    }
    let lookup = request.lookup?;
    let candidates = lookup
        .candidates
        .iter()
        .map(|candidate| {
            crate::pipe_source_index_projection::source_index_candidate(
                request.project_root,
                request.intent,
                candidate,
            )
        })
        .collect::<Vec<_>>();
    let decision = if lookup.state == "busy" && candidates.is_empty() {
        SearchPipeSourceIndexDecision::Busy
    } else if lookup.state == "cold-required" && candidates.is_empty() {
        SearchPipeSourceIndexDecision::ColdRequired
    } else if intent_terms_all_path_like(request.intent)
        && matches!(lookup.state.as_str(), "missing-db" | "empty-index" | "miss")
    {
        SearchPipeSourceIndexDecision::DeferBackend
    } else if candidates.is_empty() {
        SearchPipeSourceIndexDecision::Fallthrough
    } else if candidates
        .iter()
        .all(crate::pipe_source_index_projection::source_index_candidate_ready)
    {
        SearchPipeSourceIndexDecision::UseAndSkipSearchOverlay
    } else {
        SearchPipeSourceIndexDecision::DeferBackend
    };
    Some(SearchPipeSourceIndexAcquisition {
        decision,
        gate: None,
        candidates,
        source_snapshot: lookup.source_snapshot.clone(),
        index_artifact_digest: lookup.index_artifact_digest.clone(),
    })
}

#[must_use]
pub fn search_pipe_source_index_query_gate(
    terms: &[crate::SearchPipeQueryTerm],
) -> Option<SearchPipeSourceIndexGate> {
    if terms.len() < 2
        || terms.iter().any(|term| {
            term.role == crate::SearchPipeTermRole::Symbol
                || crate::search_pipe_is_path_like_token(&term.raw)
        })
    {
        return None;
    }
    Some(SearchPipeSourceIndexGate {
        term_count: terms.len(),
        generic_term_count: terms
            .iter()
            .filter(|term| term.role != crate::SearchPipeTermRole::Symbol)
            .count(),
    })
}
