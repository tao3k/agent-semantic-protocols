//! Candidate source selection for ASP-owned search pipe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_semantic_client::lookup_search_pipe_source_index_for_language;
use agent_semantic_search::{
    SearchPipeAutoAcquisitionRequest, SearchPipeDocumentAcquisitionRequest,
    SearchPipeSearchOverlayAcquisitionRequest, SearchPipeSourceAcquisition,
    SearchPipeSourceAcquisitionTrace, SearchPipeSourceMode, collect_search_pipe_auto_acquisition,
    collect_search_pipe_document_acquisition, collect_search_pipe_search_overlay_acquisition,
};
use orgize::document::DocumentLanguage;
use serde_json::Value;

use super::search_config::AspConfig;
use super::search_pipe_candidates::{
    PIPE_CANDIDATE_LINE_LIMIT, parse_ingest_candidates, read_piped_stdin,
};
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceSpec {
    Auto,
    Provider,
    SearchOverlay,
    Ingest,
}

impl SourceSpec {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Provider => "provider",
            Self::SearchOverlay => "search-overlay",
            Self::Ingest => "ingest",
        }
    }
}

pub(super) struct CandidateAcquisition {
    pub(super) candidates: Vec<Candidate>,
    pub(super) candidate_sources: Vec<String>,
    pub(super) source_trace: Vec<SearchPipeSourceTrace>,
    pub(super) source_snapshot: Option<agent_semantic_content_identity::SourceSnapshotEvidence>,
}

pub(super) fn parse_source_spec(value: &str) -> Result<SourceSpec, String> {
    match value {
        "auto" => Ok(SourceSpec::Auto),
        "provider" => Ok(SourceSpec::Provider),
        "search-overlay" => Ok(SourceSpec::SearchOverlay),
        "ingest" => Ok(SourceSpec::Ingest),
        _ => Err(format!(
            "unknown search pipe source: {value} (expected auto, provider, search-overlay, ingest)"
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_search_pipe_candidates(
    language_id: &str,
    project_root: &Path,
    current_snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    source: SourceSpec,
    config: &AspConfig,
    provider_context: Option<&super::search_pipe_provider_facts::ProviderGraphFactsContext<'_>>,
    require_multi_clause: bool,
) -> Result<CandidateAcquisition, String> {
    if let Some(language) = document_language(language_id) {
        return collect_document_search_pipe_candidates(
            language,
            project_root,
            locator_root,
            intent,
            scopes,
            source,
            config,
            current_snapshot,
        );
    }
    let query_clauses = super::search_pipe_provider_facts::with_query_pack_descriptor(
        provider_context,
        |descriptor| {
            agent_semantic_search::search_pipe_query_clauses(
                agent_semantic_search::SearchPipeQueryClausesRequest::new(
                    agent_semantic_search::SearchPipeLanguageId::new(language_id),
                    agent_semantic_search::SearchPipeQueryText::new(intent),
                )
                .with_query_pack_descriptor(descriptor),
            )
        },
    )?;
    let query_terms = agent_semantic_search::search_pipe_unique_query_terms(&query_clauses);
    match source {
        SourceSpec::Auto => auto_candidates(
            language_id,
            project_root,
            locator_root,
            intent,
            scopes,
            config,
            require_multi_clause,
            query_clauses.len(),
            &query_terms,
            current_snapshot,
        ),
        SourceSpec::SearchOverlay => search_overlay_candidates(
            language_id,
            project_root,
            locator_root,
            intent,
            scopes,
            config,
            require_multi_clause,
            current_snapshot,
        ),
        SourceSpec::Provider => provider_candidates(),
        SourceSpec::Ingest => ingest_candidates(project_root, locator_root),
    }
}

fn collect_document_search_pipe_candidates(
    language: DocumentLanguage,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    source: SourceSpec,
    config: &AspConfig,
    current_snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
) -> Result<CandidateAcquisition, String> {
    match source {
        SourceSpec::Auto | SourceSpec::Provider | SourceSpec::SearchOverlay => {
            let acquisition =
                collect_search_pipe_document_acquisition(SearchPipeDocumentAcquisitionRequest {
                    language,
                    project_root,
                    locator_root,
                    intent,
                    scopes,
                    mode: document_source_mode(source),
                    ignore_dirs: &config.search.ignore_dirs,
                    include_hidden_dirs: &config.search.include_hidden_dirs,
                    search_overlay_limit: PIPE_CANDIDATE_LINE_LIMIT,
                    base_snapshot: &current_snapshot.workspace_snapshot,
                    provider_digest: &current_snapshot.source_snapshot.provider_digest,
                })?;
            Ok(candidate_acquisition_from_search(acquisition))
        }
        SourceSpec::Ingest => ingest_candidates(project_root, locator_root),
    }
}

fn document_source_mode(source: SourceSpec) -> SearchPipeSourceMode {
    match source {
        SourceSpec::Auto => SearchPipeSourceMode::Auto,
        SourceSpec::Provider => SearchPipeSourceMode::Provider,
        SourceSpec::SearchOverlay => SearchPipeSourceMode::SearchOverlay,
        SourceSpec::Ingest => SearchPipeSourceMode::Auto,
    }
}

fn document_language(language_id: &str) -> Option<DocumentLanguage> {
    match language_id {
        "org" => Some(DocumentLanguage::Org),
        "md" => Some(DocumentLanguage::Markdown),
        _ => None,
    }
}

fn auto_candidates(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    config: &AspConfig,
    require_multi_clause: bool,
    query_clause_count: usize,
    query_terms: &[agent_semantic_search::SearchPipeQueryTerm],
    current_snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
) -> Result<CandidateAcquisition, String> {
    let language = agent_semantic_client::LanguageId::from(language_id);
    let source_index_query = source_index_lookup_query(intent, query_clause_count, query_terms);
    let source_index_query_gated = scopes.is_empty()
        && agent_semantic_search::search_pipe_source_index_query_gate(query_terms).is_some();
    let source_index_lookup = if scopes.is_empty() && !source_index_query_gated {
        Some(lookup_search_pipe_source_index_for_language(
            project_root,
            &current_snapshot.source_snapshot,
            Some(&language),
            &source_index_query,
            PIPE_CANDIDATE_LINE_LIMIT as u32,
        )?)
    } else {
        None
    };
    let acquisition = collect_search_pipe_auto_acquisition(SearchPipeAutoAcquisitionRequest {
        language_id,
        project_root,
        locator_root,
        query: intent,
        query_terms,
        owners: scopes,
        ignore_dirs: &config.search.ignore_dirs,
        include_hidden_dirs: &config.search.include_hidden_dirs,
        require_multi_clause,
        limit: PIPE_CANDIDATE_LINE_LIMIT,
        source_index_lookup: source_index_lookup.as_ref(),
        base_snapshot: &current_snapshot.workspace_snapshot,
        provider_digest: &current_snapshot.source_snapshot.provider_digest,
    })?;
    Ok(candidate_acquisition_from_search(acquisition))
}

fn source_index_lookup_query(
    intent: &str,
    query_clause_count: usize,
    query_terms: &[agent_semantic_search::SearchPipeQueryTerm],
) -> String {
    if query_clause_count < 2 {
        return intent.to_string();
    }
    query_terms
        .iter()
        .map(|term| term.raw.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn candidate_acquisition_from_search(
    acquisition: SearchPipeSourceAcquisition,
) -> CandidateAcquisition {
    CandidateAcquisition {
        candidates: acquisition
            .candidates
            .into_iter()
            .map(Candidate::from)
            .collect(),
        candidate_sources: acquisition.candidate_sources,
        source_trace: acquisition
            .source_trace
            .into_iter()
            .map(search_source_trace)
            .collect(),
        source_snapshot: acquisition.source_snapshot,
    }
}

pub(super) fn collect_workspace_scope_topology_acquisition(
    scope: &agent_semantic_search::SemanticWorkspaceScope,
    locator_root: &Path,
    ignore_dirs: &[String],
    include_hidden_dirs: &[String],
) -> Result<CandidateAcquisition, String> {
    agent_semantic_search::collect_search_pipe_scope_topology_acquisition(
        agent_semantic_search::SearchPipeScopeTopologyAcquisitionRequest {
            workspace_scope: scope,
            locator_root,
            ignore_dirs,
            include_hidden_dirs,
            entry_visit_limit: agent_semantic_search::SEARCH_PIPE_SCOPE_TOPOLOGY_ENTRY_VISIT_LIMIT,
            candidate_limit: agent_semantic_search::SEARCH_PIPE_SCOPE_TOPOLOGY_CANDIDATE_LIMIT,
        },
    )
    .map(candidate_acquisition_from_search)
}

pub(super) fn merge_candidate_acquisitions(
    primary: &mut CandidateAcquisition,
    secondary: CandidateAcquisition,
) {
    let mut seen = primary
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.path.clone(),
                candidate.line,
                candidate.end_line,
                candidate.symbol.clone(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    primary
        .candidates
        .extend(secondary.candidates.into_iter().filter(|candidate| {
            seen.insert((
                candidate.path.clone(),
                candidate.line,
                candidate.end_line,
                candidate.symbol.clone(),
            ))
        }));
    for source in secondary.candidate_sources {
        if !primary.candidate_sources.contains(&source) {
            primary.candidate_sources.push(source);
        }
    }
    primary.source_trace.extend(secondary.source_trace);
    if primary.source_snapshot.is_none() {
        primary.source_snapshot = secondary.source_snapshot;
    }
}

fn search_source_trace(trace: SearchPipeSourceAcquisitionTrace) -> SearchPipeSourceTrace {
    let mut source_trace = SearchPipeSourceTrace::new(
        trace.source,
        trace.status,
        trace.matched,
        trace.missing,
        trace.normalized,
    );
    let mut fields = std::collections::BTreeMap::new();
    if let Some(elapsed) = trace.elapsed {
        fields.extend(elapsed_fields(elapsed));
    }
    if let Some(source_snapshot) = trace.source_snapshot {
        fields.insert(
            "sourceSnapshot".to_owned(),
            serde_json::to_value(source_snapshot)
                .expect("SourceSnapshotEvidence must serialize to JSON"),
        );
    }
    if let Some(artifact_digest) = trace.artifact_digest {
        fields.insert(
            "indexArtifactDigest".to_owned(),
            serde_json::Value::String(artifact_digest),
        );
    }
    if !fields.is_empty() {
        source_trace = source_trace.with_fields(fields);
    }
    source_trace
}

fn search_overlay_candidates(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    config: &AspConfig,
    require_multi_clause: bool,
    current_snapshot: &agent_semantic_client::source_index::CurrentSourceIndexSnapshot,
) -> Result<CandidateAcquisition, String> {
    let acquisition = collect_search_pipe_search_overlay_acquisition(
        SearchPipeSearchOverlayAcquisitionRequest {
            language_id,
            project_root,
            locator_root,
            query: intent,
            owners: scopes,
            ignore_dirs: &config.search.ignore_dirs,
            include_hidden_dirs: &config.search.include_hidden_dirs,
            require_multi_clause,
            limit: PIPE_CANDIDATE_LINE_LIMIT,
            base_snapshot: &current_snapshot.workspace_snapshot,
            provider_digest: &current_snapshot.source_snapshot.provider_digest,
        },
    )?;
    let candidates = acquisition
        .candidates
        .into_iter()
        .map(Candidate::from)
        .collect::<Vec<_>>();
    let source = candidate_route_source(&candidates);
    Ok(CandidateAcquisition {
        candidate_sources: vec![source.to_string()],
        source_trace: vec![
            candidate_trace(source, &candidates).with_fields(elapsed_fields(acquisition.elapsed)),
        ],
        source_snapshot: Some(acquisition.result_source_snapshot),
        candidates,
    })
}

fn candidate_route_source(_candidates: &[Candidate]) -> &'static str {
    "search-overlay"
}

fn provider_candidates() -> Result<CandidateAcquisition, String> {
    Ok(CandidateAcquisition {
        candidates: Vec::new(),
        candidate_sources: vec!["provider".to_string()],
        source_trace: vec![
            SearchPipeSourceTrace::new("provider", "partial", 0, 1, 0),
            SearchPipeSourceTrace::new("search-overlay", "skipped", 0, 0, 0),
        ],
        source_snapshot: None,
    })
}

fn ingest_candidates(
    project_root: &Path,
    locator_root: &Path,
) -> Result<CandidateAcquisition, String> {
    let stdin = read_piped_stdin()?;
    let candidates = parse_ingest_candidates(project_root, locator_root, stdin.as_slice());
    Ok(CandidateAcquisition {
        candidate_sources: vec!["ingest".to_string()],
        source_trace: vec![candidate_trace("ingest", &candidates)],
        source_snapshot: None,
        candidates,
    })
}

fn candidate_trace(source: &str, candidates: &[Candidate]) -> SearchPipeSourceTrace {
    SearchPipeSourceTrace::new(
        source,
        if candidates.is_empty() {
            "empty"
        } else {
            "used"
        },
        candidates.len(),
        usize::from(candidates.is_empty()),
        candidates.len(),
    )
}

fn elapsed_fields(duration: Duration) -> BTreeMap<String, Value> {
    let mut fields = BTreeMap::new();
    fields.insert(
        "elapsedMs".to_string(),
        Value::from(elapsed_millis(duration)),
    );
    fields
}

fn elapsed_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}
