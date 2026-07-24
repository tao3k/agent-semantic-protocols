//! Snapshot-keyed source-index acquisition for the search pipe.

use std::path::{Path, PathBuf};

use crate::SearchPipeCandidate;
use crate::pipe_source::intent_terms_all_path_like;

macro_rules! source_index_acquisition_text {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
    };
}

source_index_acquisition_text!(SearchPipeSourceIndexPath);
source_index_acquisition_text!(SearchPipeSourceIndexLanguageId);
source_index_acquisition_text!(SearchPipeSourceIndexProviderId);
source_index_acquisition_text!(SearchPipeSourceIndexSourceKind);
source_index_acquisition_text!(SearchPipeSourceIndexQueryKey);
source_index_acquisition_text!(SearchPipeSourceIndexStructuralSelector);
source_index_acquisition_text!(SearchPipeSourceIndexPayloadKind);
source_index_acquisition_text!(SearchPipeSourceIndexLookupState);
source_index_acquisition_text!(SearchPipeSourceIndexArtifactDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexCandidate {
    pub path: SearchPipeSourceIndexPath,
    pub language_id: Option<SearchPipeSourceIndexLanguageId>,
    pub provider_id: Option<SearchPipeSourceIndexProviderId>,
    pub source_kind: SearchPipeSourceIndexSourceKind,
    pub line_count: Option<u32>,
    pub query_keys: Vec<SearchPipeSourceIndexQueryKey>,
    pub selector_proof: Option<SearchPipeSelectorPayloadProof>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSelectorPayloadProof {
    pub structural_selector: SearchPipeSourceIndexStructuralSelector,
    pub payload_kind: SearchPipeSourceIndexPayloadKind,
    pub bounded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexLookup {
    pub state: SearchPipeSourceIndexLookupState,
    pub candidates: Vec<SearchPipeSourceIndexCandidate>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub index_artifact_digest: Option<SearchPipeSourceIndexArtifactDigest>,
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
