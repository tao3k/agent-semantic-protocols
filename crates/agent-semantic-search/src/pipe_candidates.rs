//! Search pipe candidate orchestration.

use std::path::{Path, PathBuf};

use crate::dynamic_overlay::DynamicOverlayLane;
use crate::{
    DynamicSearchCandidate, DynamicSearchRootCandidateRequest, SearchOverlayCollectionRequest,
    SearchOverlayConfig, SearchOverlaySurface,
    collect_dynamic_lexical_overlay_candidates_from_roots, collect_search_overlay_candidates,
    language_file_spec, language_neutral_search_file_spec,
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

/// Search-pipe candidates bound to the canonical snapshot searched.
#[derive(Clone, Debug)]
pub struct SearchPipeCandidateCollection {
    pub source_snapshot: agent_semantic_artifacts::SourceSnapshotEvidence,
    pub candidates: Vec<SearchPipeCandidate>,
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
    pub base_snapshot: &'a agent_semantic_artifacts::WorkspaceSnapshot,
    pub provider_digest: &'a str,
    pub require_multi_clause: bool,
    pub limit: usize,
}

/// Request for the independent path/module candidate stage used by `search pipe`.
pub struct SearchPipePathCandidateRequest<'a> {
    pub language_id: &'a str,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub query: &'a str,
    pub owners: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub limit: usize,
}

/// Collect search pipe candidates without leaking routing logic into CLI code.
pub fn collect_search_pipe_candidates(
    request: SearchPipeCandidateRequest<'_>,
) -> Result<SearchPipeCandidateCollection, String> {
    let terms = query_terms(request.query);
    if terms.is_empty() {
        return Err("search pipe requires a non-empty query".to_string());
    }
    if request.require_multi_clause && terms.len() < 2 {
        return Err(
            "search pipe requires at least two query clauses; use search lexical --query <seed> --query <seed> for QueryBundle search or search owner <path> items --query <terms>"
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
        base_snapshot: request.base_snapshot,
        provider_digest: request.provider_digest,
        file_matches: &|path| file_spec.matches(path),
        limit: request.limit,
    })
    .map(|collection| SearchPipeCandidateCollection {
        source_snapshot: collection.source_snapshot,
        candidates: collection
            .candidates
            .into_iter()
            .map(SearchPipeCandidate::from)
            .collect(),
    })
}

/// Collect `fd`-equivalent path/module candidates without consulting the source index.
pub fn collect_search_pipe_path_candidates(
    request: SearchPipePathCandidateRequest<'_>,
) -> Result<Vec<SearchPipeCandidate>, String> {
    let terms = query_terms(request.query);
    if terms.is_empty()
        || !terms.iter().any(|term| {
            term.contains('/') || term.contains('.') || term.contains('_') || term.contains('-')
        })
    {
        return Ok(Vec::new());
    }
    let roots = if request.owners.is_empty() {
        vec![request.locator_root.to_path_buf()]
    } else {
        request
            .owners
            .iter()
            .map(|owner| {
                if owner.is_absolute() {
                    owner.clone()
                } else {
                    request.locator_root.join(owner)
                }
            })
            .collect()
    };
    let Some(collection) = collect_search_overlay_candidates(SearchOverlayCollectionRequest {
        lane: DynamicOverlayLane::Query,
        surface: SearchOverlaySurface::Path,
        language_id: request.language_id,
        file_spec_override: Some(language_neutral_search_file_spec()),
        accept_all_files: !request.owners.is_empty(),
        project_root: request.project_root,
        locator_root: request.locator_root,
        roots: &roots,
        terms: &terms,
        config: SearchOverlayConfig {
            ignore_dirs: request.ignore_dirs,
            include_hidden_dirs: request.include_hidden_dirs,
        },
        native_args: &[],
    })?
    else {
        return Ok(Vec::new());
    };
    Ok(collection
        .candidates
        .into_iter()
        .map(SearchPipeCandidate::from)
        .take(request.limit)
        .collect())
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
