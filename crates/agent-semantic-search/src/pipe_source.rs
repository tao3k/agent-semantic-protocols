//! Search-pipe source acquisition services.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{SearchPipeCandidate, SearchPipeCandidateRequest, collect_search_pipe_candidates};

macro_rules! search_pipe_source_text {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

search_pipe_source_text!(SearchPipeSourceTraceSource);
search_pipe_source_text!(SearchPipeSourceTraceStatus);
search_pipe_source_text!(SearchPipeSourceArtifactDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceAcquisitionTrace {
    pub source: SearchPipeSourceTraceSource,
    pub status: SearchPipeSourceTraceStatus,
    pub matched: usize,
    pub missing: usize,
    pub normalized: usize,
    pub elapsed: Option<Duration>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub artifact_digest: Option<SearchPipeSourceArtifactDigest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceAcquisition {
    pub candidates: Vec<SearchPipeCandidate>,
    pub candidate_sources: Vec<String>,
    pub source_trace: Vec<SearchPipeSourceAcquisitionTrace>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub artifact_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSearchOverlayAcquisition {
    pub base_source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub result_source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
    pub candidates: Vec<SearchPipeCandidate>,
    pub elapsed: Duration,
}

pub use crate::pipe_source_index_acquisition::{
    SearchPipeSourceIndexAcquisition, SearchPipeSourceIndexAcquisitionRequest,
    SearchPipeSourceIndexDecision, SearchPipeSourceIndexLookup,
    collect_search_pipe_source_index_acquisition, search_pipe_source_index_query_gate,
};

pub struct SearchPipeSearchOverlayAcquisitionRequest<'a> {
    pub file_spec: &'a crate::LanguageFileSpec,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub query: &'a str,
    pub owners: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub base_snapshot: &'a agent_semantic_content_identity::WorkspaceSnapshot,
    pub provider_digest: &'a str,
    pub require_multi_clause: bool,
    pub limit: usize,
}

pub struct SearchPipeAutoAcquisitionRequest<'a> {
    pub language_id: &'a str,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub query: &'a str,
    pub query_terms: &'a [crate::SearchPipeQueryTerm],
    pub owners: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    /// Full path authority is needed only by the filesystem overlay. A
    /// resident/source-index terminal route omits it so a CLI process never
    /// receives the complete workspace path map.
    pub base_snapshot: Option<&'a agent_semantic_content_identity::WorkspaceSnapshot>,
    pub base_source_snapshot: &'a agent_semantic_content_identity::SourceSnapshotEvidence,
    pub provider_digest: &'a str,
    pub require_multi_clause: bool,
    pub limit: usize,
    pub source_index_lookup: Option<&'a SearchPipeSourceIndexLookup>,
}

pub fn collect_search_pipe_auto_acquisition(
    request: SearchPipeAutoAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    let source_index = search_pipe_source_index_query_gate(request.query_terms)
        .map(|gate| SearchPipeSourceIndexAcquisition {
            workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationAuthority::unavailable(),
            decision: SearchPipeSourceIndexDecision::QueryGate,
            gate: Some(gate),
            candidates: Vec::new(),
            source_snapshot: None,
            index_artifact_digest: None,
        })
        .or_else(|| {
            collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
                intent: request.query,
                project_root: request.project_root,
                scopes: request.owners,
                lookup: request.source_index_lookup,
            })
        })
        .unwrap_or_else(unavailable_search_generation_acquisition);
    let frame_route = crate::pipe_source_lexical_frame::plan_pipe_lexical_search_frame(
        request.query,
        Some(&source_index),
    );
    let lexical_candidates = match frame_route.acquisition_route {
        crate::LexicalAcquisitionRoute::WarmOverlay => {
            Some(source_index.discovery_candidates().to_vec())
        }
        crate::LexicalAcquisitionRoute::SourceIndexOwnerEvidence => Some(
            crate::pipe_source_lexical_frame::source_index_owner_evidence_candidates(&source_index),
        ),
        _ => None,
    }
    .unwrap_or_default();
    let candidate_source = match source_index.decision {
        SearchPipeSourceIndexDecision::QueryGate => "query-gate",
        SearchPipeSourceIndexDecision::GenerationUnavailable => "runtime-generation",
        _ => "source-index",
    };
    Ok(SearchPipeSourceAcquisition {
        source_trace: vec![
            crate::pipe_source_index_projection::source_index_trace(&source_index),
            crate::pipe_source_lexical_frame::lexical_search_frame_trace(&frame_route),
            skipped_search_overlay_trace(),
        ],
        candidate_sources: vec![candidate_source.to_owned()],
        candidates: lexical_candidates,
        source_snapshot: source_index.source_snapshot.clone(),
        artifact_digest: source_index.index_artifact_digest.clone(),
    })
}

fn unavailable_search_generation_acquisition() -> SearchPipeSourceIndexAcquisition {
    SearchPipeSourceIndexAcquisition {
        workspace_generation: agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationAuthority::unavailable(),
        decision: SearchPipeSourceIndexDecision::GenerationUnavailable,
        gate: None,
        candidates: Vec::new(),
        source_snapshot: None,
        index_artifact_digest: None,
    }
}

pub fn collect_search_pipe_search_overlay_acquisition(
    request: SearchPipeSearchOverlayAcquisitionRequest<'_>,
) -> Result<SearchPipeSearchOverlayAcquisition, String> {
    let started_at = Instant::now();
    let collection = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        file_spec: request.file_spec,
        project_root: request.project_root,
        locator_root: request.locator_root,
        query: request.query,
        owners: request.owners,
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        base_snapshot: request.base_snapshot,
        provider_digest: request.provider_digest,
        require_multi_clause: request.require_multi_clause,
        limit: request.limit,
    })?;
    let base_source_snapshot = request.base_snapshot.evidence(
        collection.source_snapshot.source_kind,
        request.provider_digest.to_string(),
    );
    Ok(SearchPipeSearchOverlayAcquisition {
        base_source_snapshot,
        result_source_snapshot: collection.source_snapshot,
        candidates: collection.candidates,
        elapsed: started_at.elapsed(),
    })
}

pub struct SearchPipeFailureAcquisitionRequest<'a> {
    pub file_spec: &'a crate::LanguageFileSpec,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub message: &'a str,
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub limit: usize,
    pub base_snapshot: &'a agent_semantic_content_identity::WorkspaceSnapshot,
    pub provider_digest: &'a str,
}

pub fn collect_search_pipe_failure_acquisition(
    request: SearchPipeFailureAcquisitionRequest<'_>,
) -> Result<SearchPipeSearchOverlayAcquisition, String> {
    let query = failure_candidate_query(request.message);
    collect_search_pipe_search_overlay_acquisition(SearchPipeSearchOverlayAcquisitionRequest {
        require_multi_clause: false,
        file_spec: request.file_spec,
        project_root: request.project_root,
        locator_root: request.locator_root,
        query: &query,
        owners: &[],
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        limit: request.limit,
        base_snapshot: request.base_snapshot,
        provider_digest: request.provider_digest,
    })
}

pub fn failure_candidate_query(message: &str) -> String {
    let mut terms = Vec::new();
    for token in message
        .split(|character: char| !failure_token_character(character))
        .filter(|token| !token.is_empty())
    {
        if token.contains("::") {
            if let Some(last) = token.rsplit("::").find(|part| !part.is_empty()) {
                push_failure_candidate_term(&mut terms, last);
            }
        } else {
            push_failure_candidate_term(&mut terms, token);
        }
    }
    if terms.is_empty() {
        return message.to_string();
    }
    terms.join(" ")
}

fn push_failure_candidate_term(terms: &mut Vec<String>, token: &str) {
    let token = token.trim_matches([':', '.', ',', ';', '(', ')', '[', ']']);
    let lower = token.to_ascii_lowercase();
    if token.len() < 4
        || failure_candidate_stop_word(&lower)
        || !(token.contains('_') || token.contains('-'))
    {
        return;
    }
    if !terms.iter().any(|term| term == token) {
        terms.push(token.to_string());
    }
}

fn failure_candidate_stop_word(token: &str) -> bool {
    matches!(
        token,
        "expected"
            | "actual"
            | "failure"
            | "failed"
            | "panic"
            | "error"
            | "status"
            | "stdout"
            | "stderr"
            | "left"
            | "right"
            | "pass"
            | "fail"
            | "hit"
            | "miss"
            | "observed"
            | "unknown"
            | "request_fingerprint"
            | "file_hash"
    )
}

fn failure_token_character(character: char) -> bool {
    character == '_' || character == '-' || character == ':' || character.is_ascii_alphanumeric()
}

fn skipped_search_overlay_trace() -> SearchPipeSourceAcquisitionTrace {
    SearchPipeSourceAcquisitionTrace {
        source: ("search-overlay".to_string()).into(),
        status: ("skipped".to_string()).into(),
        matched: 0,
        missing: 0,
        normalized: 0,
        elapsed: None,
        source_snapshot: None,
        artifact_digest: None,
    }
}
