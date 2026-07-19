//! Candidate source selection for ASP-owned search pipe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_semantic_client::lookup_search_pipe_source_index_for_language;
use agent_semantic_client_db::{ClientDbEngine, TursoClientDbSearchHit};
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
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    source: SourceSpec,
    config: &AspConfig,
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
        );
    }
    let query_clauses = agent_semantic_search::search_pipe_query_clauses(
        agent_semantic_search::SearchPipeQueryClausesRequest::new(
            agent_semantic_search::SearchPipeLanguageId::new(language_id),
            agent_semantic_search::SearchPipeQueryText::new(intent),
        ),
    );
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
            &query_terms,
        ),
        SourceSpec::SearchOverlay => search_overlay_candidates(
            language_id,
            project_root,
            locator_root,
            intent,
            scopes,
            config,
            require_multi_clause,
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
    query_terms: &[agent_semantic_search::SearchPipeQueryTerm],
) -> Result<CandidateAcquisition, String> {
    let language = agent_semantic_client::LanguageId::from(language_id);
    let source_index_query = source_index_lookup_query(language_id, intent);
    let source_index_query_gated = scopes.is_empty()
        && agent_semantic_search::search_pipe_source_index_query_gate(query_terms).is_some();
    let source_index_lookup = if scopes.is_empty() && !source_index_query_gated {
        Some(lookup_search_pipe_source_index_for_language(
            project_root,
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
    })?;
    Ok(candidate_acquisition_from_search(acquisition))
}

fn source_index_lookup_query(language_id: &str, intent: &str) -> String {
    let clauses = super::search_pipe_query_pack::query_clauses(language_id, intent);
    if clauses.len() < 2 {
        return intent.to_string();
    }
    super::search_pipe_query_pack::unique_query_terms(&clauses)
        .into_iter()
        .map(|term| term.raw)
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
}

fn search_source_trace(trace: SearchPipeSourceAcquisitionTrace) -> SearchPipeSourceTrace {
    let mut source_trace = SearchPipeSourceTrace::new(
        trace.source,
        trace.status,
        trace.matched,
        trace.missing,
        trace.normalized,
    );
    if let Some(elapsed) = trace.elapsed {
        source_trace = source_trace.with_fields(elapsed_fields(elapsed));
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
        candidates,
    })
}

fn selector_path(selector: &str) -> Option<&str> {
    selector
        .split_once('#')
        .map(|(path, _)| path)
        .or_else(|| selector.split_once(':').map(|(path, _)| path))
        .filter(|path| !path.is_empty())
}

fn selector_line(selector: &str) -> Option<usize> {
    selector
        .split(':')
        .nth(1)
        .and_then(|line| line.parse::<usize>().ok())
}

fn selector_symbol(selector: &str) -> Option<&str> {
    selector
        .rsplit('/')
        .next()
        .filter(|symbol| !symbol.is_empty())
}

fn symbol_from_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn display_locator_path(locator_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    path.strip_prefix(locator_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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
