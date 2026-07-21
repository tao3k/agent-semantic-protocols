use std::path::{Path, PathBuf};
use std::time::Duration;

use orgize::document::DocumentLanguage;

use crate::document_candidates::{
    DocumentSearchCandidate, DocumentSearchCandidateRequest, collect_document_search_candidates,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPipeSourceMode {
    Auto,
    Provider,
    SearchOverlay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceAcquisitionTrace {
    pub source: String,
    pub status: String,
    pub matched: usize,
    pub missing: usize,
    pub normalized: usize,
    pub elapsed: Option<Duration>,
    pub source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    pub artifact_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceAcquisition {
    pub candidates: Vec<crate::SearchPipeCandidate>,
    pub candidate_sources: Vec<String>,
    pub source_trace: Vec<SearchPipeSourceAcquisitionTrace>,
    pub source_snapshot: Option<agent_semantic_artifacts::SourceSnapshotEvidence>,
    pub artifact_digest: Option<String>,
}

pub struct SearchPipeDocumentAcquisitionRequest<'a> {
    pub language: DocumentLanguage,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub intent: &'a str,
    pub scopes: &'a [PathBuf],
    pub mode: SearchPipeSourceMode,
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub search_overlay_limit: usize,
    pub base_snapshot: &'a agent_semantic_content_identity::WorkspaceSnapshot,
    pub provider_digest: &'a str,
}

pub fn collect_search_pipe_document_acquisition(
    request: SearchPipeDocumentAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    match request.mode {
        SearchPipeSourceMode::Auto => document_auto_acquisition(request),
        SearchPipeSourceMode::Provider => document_element_acquisition(request),
        SearchPipeSourceMode::SearchOverlay => search_overlay_source_acquisition(
            crate::pipe_source::SearchPipeSearchOverlayAcquisitionRequest {
                require_multi_clause: false,
                language_id: request.language.id(),
                project_root: request.project_root,
                locator_root: request.locator_root,
                query: request.intent,
                owners: request.scopes,
                ignore_dirs: request.ignore_dirs,
                include_hidden_dirs: request.include_hidden_dirs,
                limit: request.search_overlay_limit,
                base_snapshot: request.base_snapshot,
                provider_digest: request.provider_digest,
            },
        ),
    }
}

fn document_auto_acquisition(
    request: SearchPipeDocumentAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    search_overlay_source_acquisition(
        crate::pipe_source::SearchPipeSearchOverlayAcquisitionRequest {
            require_multi_clause: false,
            language_id: request.language.id(),
            project_root: request.project_root,
            locator_root: request.locator_root,
            query: request.intent,
            owners: request.scopes,
            ignore_dirs: request.ignore_dirs,
            include_hidden_dirs: request.include_hidden_dirs,
            limit: request.search_overlay_limit,
            base_snapshot: request.base_snapshot,
            provider_digest: request.provider_digest,
        },
    )
}

fn document_element_acquisition(
    request: SearchPipeDocumentAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    let collection = collect_document_search_candidates(DocumentSearchCandidateRequest {
        language: request.language,
        project_root: request.project_root,
        locator_root: request.locator_root,
        intent: request.intent,
        scopes: request.scopes,
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
    })?;
    let candidates = collection
        .candidates
        .into_iter()
        .map(document_candidate)
        .collect::<Vec<_>>();
    Ok(SearchPipeSourceAcquisition {
        source_trace: vec![SearchPipeSourceAcquisitionTrace {
            source: "document-element".to_string(),
            status: if candidates.is_empty() {
                "empty".to_string()
            } else {
                "used".to_string()
            },
            matched: candidates.len(),
            missing: usize::from(candidates.is_empty()),
            normalized: collection.matched_count,
            elapsed: None,
            source_snapshot: None,
            artifact_digest: None,
        }],
        candidate_sources: vec!["document-element".to_string()],
        candidates,
        source_snapshot: None,
        artifact_digest: None,
    })
}

fn search_overlay_source_acquisition(
    request: crate::pipe_source::SearchPipeSearchOverlayAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    let base_snapshot = request.base_snapshot;
    let provider_digest = request.provider_digest;
    let acquisition = crate::pipe_source::collect_search_pipe_search_overlay_acquisition(request)?;
    finalize_search_overlay_source_acquisition(base_snapshot, provider_digest, acquisition)
}

pub(crate) fn finalize_search_overlay_source_acquisition(
    base_snapshot: &agent_semantic_artifacts::WorkspaceSnapshot,
    provider_digest: &str,
    acquisition: crate::pipe_source::SearchPipeSearchOverlayAcquisition,
) -> Result<SearchPipeSourceAcquisition, String> {
    ensure_search_overlay_snapshot_matches(
        &acquisition.base_source_snapshot,
        base_snapshot,
        provider_digest,
    )?;
    let source_snapshot = acquisition.result_source_snapshot;
    let acquisition_elapsed = acquisition.elapsed;
    let candidates = acquisition.candidates;
    let source = search_pipe_candidate_route_source(&candidates);
    Ok(SearchPipeSourceAcquisition {
        source_trace: vec![candidate_trace(
            source,
            &candidates,
            Some(acquisition_elapsed),
            Some(source_snapshot.clone()),
            None,
        )],
        candidate_sources: vec![source.to_string()],
        candidates,
        source_snapshot: Some(source_snapshot),
        artifact_digest: None,
    })
}

fn search_pipe_candidate_route_source(_candidates: &[crate::SearchPipeCandidate]) -> &'static str {
    "search-overlay"
}

fn document_candidate(candidate: DocumentSearchCandidate) -> crate::SearchPipeCandidate {
    crate::SearchPipeCandidate {
        path: candidate.path,
        line: candidate.line,
        end_line: candidate.end_line,
        symbol: candidate.symbol,
        text: candidate.text,
        source: "document-element".to_string(),
        confidence: "parser".to_string(),
    }
}

pub(super) fn ensure_search_overlay_snapshot_matches(
    receipt_base_source_snapshot: &agent_semantic_artifacts::SourceSnapshotEvidence,
    base_snapshot: &agent_semantic_artifacts::WorkspaceSnapshot,
    provider_digest: &str,
) -> Result<(), String> {
    let requested_base_source_snapshot = base_snapshot.evidence(
        receipt_base_source_snapshot.source_kind,
        provider_digest.to_string(),
    );
    if receipt_base_source_snapshot != &requested_base_source_snapshot {
        return Err(format!(
            "search overlay source snapshot mismatch: requestedRoot={requested_root} actualRoot={}; retry against a fresh source snapshot",
            receipt_base_source_snapshot.root_digest,
            requested_root = requested_base_source_snapshot.root_digest,
        ));
    }
    Ok(())
}

pub(super) fn candidate_trace(
    source: &str,
    candidates: &[crate::SearchPipeCandidate],
    elapsed: Option<Duration>,
    source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
    artifact_digest: Option<String>,
) -> SearchPipeSourceAcquisitionTrace {
    SearchPipeSourceAcquisitionTrace {
        source: source.to_string(),
        status: if candidates.is_empty() {
            "empty".to_string()
        } else {
            "used".to_string()
        },
        matched: candidates.len(),
        missing: usize::from(candidates.is_empty()),
        normalized: candidates.len(),
        elapsed,
        source_snapshot,
        artifact_digest,
    }
}
