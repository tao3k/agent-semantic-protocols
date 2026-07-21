//! Search-pipe source acquisition services.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::pipe_candidates::{SearchPipePathCandidateRequest, collect_search_pipe_path_candidates};
use crate::{SearchPipeCandidate, SearchPipeCandidateRequest, collect_search_pipe_candidates};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSearchOverlayAcquisition {
    pub base_source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
    pub result_source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
    pub candidates: Vec<SearchPipeCandidate>,
    pub elapsed: Duration,
}

pub use crate::pipe_source_document_acquisition::{
    SearchPipeDocumentAcquisitionRequest, SearchPipeSourceAcquisition,
    SearchPipeSourceAcquisitionTrace, SearchPipeSourceMode,
    collect_search_pipe_document_acquisition,
};
use crate::pipe_source_document_acquisition::{
    candidate_trace, ensure_search_overlay_snapshot_matches,
};
pub use crate::pipe_source_index_acquisition::{
    SearchPipeSourceIndexAcquisition, SearchPipeSourceIndexAcquisitionRequest,
    SearchPipeSourceIndexDecision, SearchPipeSourceIndexLookup,
    collect_search_pipe_source_index_acquisition, search_pipe_source_index_query_gate,
};

pub struct SearchPipeSearchOverlayAcquisitionRequest<'a> {
    pub language_id: &'a str,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub query: &'a str,
    pub owners: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub base_snapshot: &'a agent_semantic_artifacts::WorkspaceSnapshot,
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
    pub base_snapshot: &'a agent_semantic_artifacts::WorkspaceSnapshot,
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
        });
    let frame_route = crate::pipe_source_lexical_frame::plan_pipe_lexical_search_frame(
        request.query,
        source_index.as_ref(),
    );
    let source_index_needs_rescue = source_index.as_ref().is_some_and(|source_index| {
        matches!(
            source_index.decision,
            SearchPipeSourceIndexDecision::Busy | SearchPipeSourceIndexDecision::ColdRequired
        )
    });
    let lexical_candidates = source_index
        .as_ref()
        .and_then(|source_index| match frame_route.acquisition_route {
            crate::LexicalAcquisitionRoute::WarmOverlay => Some(source_index.candidates.clone()),
            crate::LexicalAcquisitionRoute::SourceIndexOwnerEvidence => Some(
                crate::pipe_source_lexical_frame::source_index_owner_evidence_candidates(
                    source_index,
                ),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if let Some(source_index) = source_index.as_ref()
        && source_index_auto_route_is_terminal(source_index.decision, frame_route.acquisition_route)
    {
        let source_trace = vec![
            crate::pipe_source_index_projection::source_index_trace(source_index),
            crate::pipe_source_lexical_frame::lexical_search_frame_trace(&frame_route),
            skipped_search_overlay_trace(),
        ];
        let candidate_sources = if source_index.decision == SearchPipeSourceIndexDecision::QueryGate
        {
            vec!["search-overlay".to_string()]
        } else {
            vec!["source-index".to_string()]
        };
        return Ok(SearchPipeSourceAcquisition {
            source_trace,
            candidate_sources,
            candidates: lexical_candidates,
            source_snapshot: source_index.source_snapshot.clone(),
            artifact_digest: source_index.index_artifact_digest.clone(),
        });
    }
    let path_started_at = Instant::now();
    let query_gated = source_index.as_ref().is_some_and(|source_index| {
        source_index.decision == SearchPipeSourceIndexDecision::QueryGate
    });
    let path_candidates = if query_gated {
        Vec::new()
    } else {
        collect_search_pipe_path_candidates(SearchPipePathCandidateRequest {
            language_id: request.language_id,
            project_root: request.project_root,
            locator_root: request.locator_root,
            query: request.query,
            owners: request.owners,
            ignore_dirs: request.ignore_dirs,
            include_hidden_dirs: request.include_hidden_dirs,
            limit: request.limit,
        })?
    };
    let mut candidates = merge_search_pipe_candidates(lexical_candidates, path_candidates.clone());
    let proof_scopes = search_pipe_candidate_scopes(&candidates);
    let proof_scopes = if proof_scopes.is_empty() {
        request.owners.to_vec()
    } else {
        proof_scopes
    };
    let acquisition = collect_search_pipe_search_overlay_acquisition(
        SearchPipeSearchOverlayAcquisitionRequest {
            language_id: request.language_id,
            project_root: request.project_root,
            locator_root: request.locator_root,
            query: request.query,
            owners: &proof_scopes,
            ignore_dirs: request.ignore_dirs,
            include_hidden_dirs: request.include_hidden_dirs,
            base_snapshot: request.base_snapshot,
            provider_digest: request.provider_digest,
            require_multi_clause: request.require_multi_clause,
            limit: request.limit,
        },
    )?;
    ensure_search_overlay_snapshot_matches(
        &acquisition.base_source_snapshot,
        request.base_snapshot,
        request.provider_digest,
    )?;
    let source_snapshot = acquisition.result_source_snapshot;
    let acquisition_elapsed = acquisition.elapsed;
    let proof_candidates = acquisition.candidates;
    let mut source_trace = Vec::new();
    if let Some(source_index) = source_index.as_ref() {
        source_trace.push(crate::pipe_source_index_projection::source_index_trace(
            source_index,
        ));
    }
    source_trace.push(crate::pipe_source_lexical_frame::lexical_search_frame_trace(&frame_route));
    if !query_gated {
        source_trace.push(candidate_trace(
            "fd-path",
            &path_candidates,
            Some(path_started_at.elapsed()),
            None,
            None,
        ));
    }
    let proof_source = if source_index_needs_rescue {
        "search-overlay-rescue"
    } else {
        "search-overlay"
    };
    source_trace.push(candidate_trace(
        proof_source,
        &proof_candidates,
        Some(acquisition_elapsed),
        Some(source_snapshot.clone()),
        None,
    ));
    candidates = merge_search_pipe_candidates(candidates, proof_candidates);
    let mut candidate_sources = Vec::new();
    if source_index.is_some() && !query_gated {
        candidate_sources.push("source-index".to_string());
    }
    if !query_gated {
        candidate_sources.push("fd-path".to_string());
    }
    candidate_sources.push(proof_source.to_string());
    Ok(SearchPipeSourceAcquisition {
        source_trace,
        candidate_sources,
        candidates,
        source_snapshot: Some(source_snapshot),
        artifact_digest: None,
    })
}

fn merge_search_pipe_candidates(
    primary: Vec<SearchPipeCandidate>,
    secondary: Vec<SearchPipeCandidate>,
) -> Vec<SearchPipeCandidate> {
    let mut seen = BTreeSet::new();
    primary
        .into_iter()
        .chain(secondary)
        .filter(|candidate| {
            seen.insert((
                candidate.path.clone(),
                candidate.line,
                candidate.end_line,
                candidate.symbol.clone(),
            ))
        })
        .collect()
}

fn search_pipe_candidate_scopes(candidates: &[SearchPipeCandidate]) -> Vec<PathBuf> {
    candidates
        .iter()
        .filter(|candidate| !candidate.path.is_empty())
        .map(|candidate| candidate.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

pub fn collect_search_pipe_search_overlay_acquisition(
    request: SearchPipeSearchOverlayAcquisitionRequest<'_>,
) -> Result<SearchPipeSearchOverlayAcquisition, String> {
    let started_at = Instant::now();
    let collection = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        language_id: request.language_id,
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
    pub language_id: &'a str,
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
        language_id: request.language_id,
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

fn source_index_auto_route_is_terminal(
    decision: SearchPipeSourceIndexDecision,
    route: crate::LexicalAcquisitionRoute,
) -> bool {
    matches!(
        decision,
        SearchPipeSourceIndexDecision::UseAndSkipSearchOverlay
            | SearchPipeSourceIndexDecision::Busy
            | SearchPipeSourceIndexDecision::ColdRequired
            | SearchPipeSourceIndexDecision::QueryGate
    ) || (decision == SearchPipeSourceIndexDecision::DeferBackend
        && route == crate::LexicalAcquisitionRoute::SourceIndexOwnerEvidence)
}

fn skipped_search_overlay_trace() -> SearchPipeSourceAcquisitionTrace {
    SearchPipeSourceAcquisitionTrace {
        source: "search-overlay".to_string(),
        status: "skipped".to_string(),
        matched: 0,
        missing: 0,
        normalized: 0,
        elapsed: None,
        source_snapshot: None,
        artifact_digest: None,
    }
}

pub(super) fn intent_terms_all_path_like(intent: &str) -> bool {
    let terms = intent
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    !terms.is_empty()
        && terms
            .iter()
            .all(|term| term.contains('/') || term.contains('\\') || term.contains('.'))
}
