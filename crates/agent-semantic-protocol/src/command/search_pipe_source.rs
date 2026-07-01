//! Candidate source selection for ASP-owned search pipe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_client::{
    SourceIndexLookupResult, SourceIndexLookupState, lookup_source_index_for_language,
};
use agent_semantic_search::{
    SearchPipeDocumentAcquisitionRequest, SearchPipeFinderAcquisitionRequest,
    SearchPipeSourceAcquisition, SearchPipeSourceAcquisitionTrace,
    SearchPipeSourceIndexAcquisition, SearchPipeSourceIndexAcquisitionRequest,
    SearchPipeSourceIndexCandidate, SearchPipeSourceIndexDecision, SearchPipeSourceIndexGate,
    SearchPipeSourceIndexLookup, SearchPipeSourceMode, collect_search_pipe_document_acquisition,
    collect_search_pipe_finder_acquisition, collect_search_pipe_source_index_acquisition,
};
use orgize::document::DocumentLanguage;
use serde_json::Value;

use super::search_config::AspConfig;
use super::search_pipe_candidates::{
    PIPE_CANDIDATE_LINE_LIMIT, parse_ingest_candidates, read_piped_stdin,
};
use super::search_pipe_model::{Candidate, SearchPipeSourceTrace};

const DOCUMENT_PIPE_CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceSpec {
    Auto,
    Provider,
    Finder,
    Ingest,
}

impl SourceSpec {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Provider => "provider",
            Self::Finder => "finder",
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
        "finder" => Ok(SourceSpec::Finder),
        "ingest" => Ok(SourceSpec::Ingest),
        _ => Err(format!(
            "unknown search pipe source: {value} (expected auto, provider, finder, ingest)"
        )),
    }
}

pub(super) fn collect_search_pipe_candidates(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    source: SourceSpec,
    config: &AspConfig,
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
    match source {
        SourceSpec::Auto => auto_candidates(
            language_id,
            project_root,
            locator_root,
            intent,
            scopes,
            config,
        ),
        SourceSpec::Finder => finder_candidates(
            language_id,
            project_root,
            locator_root,
            intent,
            scopes,
            config,
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
        SourceSpec::Auto | SourceSpec::Provider | SourceSpec::Finder => {
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
                    finder_limit: PIPE_CANDIDATE_LINE_LIMIT,
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
        SourceSpec::Finder => SearchPipeSourceMode::Finder,
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
) -> Result<CandidateAcquisition, String> {
    let source_index_acquisition =
        source_index_candidates(language_id, project_root, intent, scopes);
    if let Some(acquisition) = source_index_acquisition
        .as_ref()
        .filter(|acquisition| source_index_acquisition_blocks_backend(acquisition))
    {
        return Ok(CandidateAcquisition {
            candidates: acquisition.candidates.clone(),
            candidate_sources: vec!["source-index".to_string()],
            source_trace: acquisition.source_trace.clone(),
        });
    }
    if let Some(acquisition) = source_index_acquisition
        .as_ref()
        .filter(|acquisition| source_index_path_query_defers_backend(acquisition, intent))
    {
        return Ok(CandidateAcquisition {
            candidates: acquisition.candidates.clone(),
            candidate_sources: vec!["source-index".to_string()],
            source_trace: acquisition.source_trace.clone(),
        });
    }
    if let Some(acquisition) = source_index_acquisition
        .as_ref()
        .filter(|acquisition| !acquisition.candidates.is_empty())
    {
        return Ok(CandidateAcquisition {
            candidates: acquisition.candidates.clone(),
            candidate_sources: vec!["source-index".to_string(), "finder".to_string()],
            source_trace: acquisition.source_trace.clone(),
        });
    }
    let finder_acquisition =
        collect_search_pipe_finder_acquisition(SearchPipeFinderAcquisitionRequest {
            language_id,
            project_root,
            locator_root,
            query: intent,
            owners: scopes,
            ignore_dirs: &config.search.ignore_dirs,
            include_hidden_dirs: &config.search.include_hidden_dirs,
            limit: PIPE_CANDIDATE_LINE_LIMIT,
        })?;
    let elapsed = finder_acquisition.elapsed;
    let candidates = finder_acquisition
        .candidates
        .into_iter()
        .map(Candidate::from)
        .collect::<Vec<_>>();
    Ok(CandidateAcquisition {
        candidate_sources: vec!["provider".to_string(), "finder".to_string()],
        source_trace: source_index_trace_prefix(source_index_acquisition)
            .into_iter()
            .chain([
                SearchPipeSourceTrace::new(
                    "provider",
                    "partial",
                    0,
                    usize::from(!candidates.is_empty()),
                    0,
                )
                .with_fields(elapsed_fields(elapsed)),
                candidate_trace("finder", &candidates).with_fields(elapsed_fields(elapsed)),
            ])
            .collect(),
        candidates,
    })
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

fn source_index_acquisition_blocks_backend(acquisition: &CandidateAcquisition) -> bool {
    acquisition.source_trace.iter().any(|trace| {
        trace.source == "sourceIndex"
            && trace.status == "skipped"
            && trace
                .fields
                .get("reason")
                .and_then(Value::as_str)
                .is_some_and(|reason| reason == "query-gate")
    })
}

fn source_index_path_query_defers_backend(
    acquisition: &CandidateAcquisition,
    intent: &str,
) -> bool {
    intent_terms_all_path_like(intent)
        && acquisition.source_trace.iter().any(|trace| {
            trace.source == "sourceIndex"
                && matches!(trace.status.as_str(), "missing-db" | "empty-index" | "miss")
        })
}

fn intent_terms_all_path_like(intent: &str) -> bool {
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

fn source_index_candidates(
    language_id: &str,
    project_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
) -> Option<CandidateAcquisition> {
    if !scopes.is_empty() {
        return None;
    }
    if let Some(acquisition) =
        collect_search_pipe_source_index_acquisition(SearchPipeSourceIndexAcquisitionRequest {
            intent,
            scopes,
            lookup: None,
        })
    {
        return Some(source_index_acquisition_from_search(acquisition, None));
    }
    let started_at = Instant::now();
    let language_scope = agent_semantic_client::LanguageId::from(language_id);
    match lookup_source_index_for_language(
        project_root,
        Some(&language_scope),
        intent,
        DOCUMENT_PIPE_CANDIDATE_LIMIT as u32,
    ) {
        Ok(result) => {
            let lookup = search_source_index_lookup_from_client(&result);
            let acquisition = collect_search_pipe_source_index_acquisition(
                SearchPipeSourceIndexAcquisitionRequest {
                    intent,
                    scopes,
                    lookup: Some(&lookup),
                },
            )?;
            let candidates = acquisition
                .candidates
                .iter()
                .cloned()
                .map(Candidate::from)
                .collect::<Vec<_>>();
            let mut source_trace = vec![source_index_trace(&result, candidates.len(), started_at)];
            if acquisition.decision == SearchPipeSourceIndexDecision::UseAndSkipFinder {
                source_trace.push(SearchPipeSourceTrace::new("finder", "skipped", 0, 0, 0));
            }
            Some(CandidateAcquisition {
                candidate_sources: vec!["source-index".to_string()],
                source_trace,
                candidates,
            })
        }
        Err(error) => {
            let mut fields = elapsed_fields(started_at.elapsed());
            fields.insert("state".to_string(), Value::from("error"));
            fields.insert("detail".to_string(), Value::from(compact_detail(&error)));
            fields.insert(
                "nextCommand".to_string(),
                Value::from("asp cache source-index refresh"),
            );
            Some(CandidateAcquisition {
                candidates: Vec::new(),
                candidate_sources: vec!["source-index".to_string()],
                source_trace: vec![
                    SearchPipeSourceTrace::new("sourceIndex", "error", 0, 1, 0).with_fields(fields),
                ],
            })
        }
    }
}

fn source_index_acquisition_from_search(
    acquisition: SearchPipeSourceIndexAcquisition,
    source_trace: Option<SearchPipeSourceTrace>,
) -> CandidateAcquisition {
    let candidates = acquisition
        .candidates
        .into_iter()
        .map(Candidate::from)
        .collect::<Vec<_>>();
    let source_trace = source_trace
        .map(|trace| vec![trace])
        .unwrap_or_else(|| source_index_gate_trace(acquisition.gate));
    CandidateAcquisition {
        candidate_sources: vec!["source-index".to_string()],
        source_trace,
        candidates,
    }
}

fn source_index_gate_trace(gate: Option<SearchPipeSourceIndexGate>) -> Vec<SearchPipeSourceTrace> {
    let Some(gate) = gate else {
        return Vec::new();
    };
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("reason".to_string(), Value::from("query-gate"));
    fields.insert("termCount".to_string(), Value::from(gate.term_count));
    fields.insert(
        "genericTermCount".to_string(),
        Value::from(gate.generic_term_count),
    );
    vec![SearchPipeSourceTrace::new("sourceIndex", "skipped", 0, 0, 0).with_fields(fields)]
}

fn search_source_index_lookup_from_client(
    result: &SourceIndexLookupResult,
) -> SearchPipeSourceIndexLookup {
    SearchPipeSourceIndexLookup {
        state: result.state.as_str().to_string(),
        candidates: result
            .candidates
            .iter()
            .map(|candidate| SearchPipeSourceIndexCandidate {
                path: candidate.path.clone(),
                language_id: candidate
                    .language_id
                    .as_ref()
                    .map(|value| value.as_str().to_string()),
                provider_id: candidate
                    .provider_id
                    .as_ref()
                    .map(|value| value.as_str().to_string()),
                source_kind: candidate.source_kind.as_str().to_string(),
                line_count: candidate.line_count,
                query_keys: candidate.query_keys.clone(),
            })
            .collect(),
    }
}

fn source_index_trace_prefix(
    acquisition: Option<CandidateAcquisition>,
) -> Vec<SearchPipeSourceTrace> {
    acquisition
        .map(|acquisition| acquisition.source_trace)
        .unwrap_or_default()
}

pub(super) fn source_index_trace(
    result: &SourceIndexLookupResult,
    candidate_count: usize,
    started_at: Instant,
) -> SearchPipeSourceTrace {
    source_index_trace_with_elapsed(result, candidate_count, started_at.elapsed())
}

pub(super) fn source_index_trace_with_elapsed(
    result: &SourceIndexLookupResult,
    candidate_count: usize,
    elapsed: std::time::Duration,
) -> SearchPipeSourceTrace {
    let mut fields = elapsed_fields(elapsed);
    fields.insert(
        "collectMs".to_string(),
        Value::from(elapsed_millis(elapsed)),
    );
    fields.insert("state".to_string(), Value::from(result.state.as_str()));
    if result.state != SourceIndexLookupState::Hit {
        fields.insert(
            "nextCommand".to_string(),
            Value::from("asp cache source-index refresh"),
        );
    }
    SearchPipeSourceTrace::new(
        "sourceIndex",
        source_index_trace_status(&result.state),
        candidate_count,
        usize::from(candidate_count == 0),
        candidate_count,
    )
    .with_fields(fields)
}

fn source_index_trace_status(state: &SourceIndexLookupState) -> &'static str {
    match state {
        SourceIndexLookupState::Hit => "used",
        SourceIndexLookupState::MissingDb => "missing-db",
        SourceIndexLookupState::EmptyIndex => "empty-index",
        SourceIndexLookupState::Miss => "miss",
    }
}

fn compact_detail(detail: &str) -> String {
    detail.split_whitespace().collect::<Vec<_>>().join("_")
}

fn finder_candidates(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    config: &AspConfig,
) -> Result<CandidateAcquisition, String> {
    let acquisition = collect_search_pipe_finder_acquisition(SearchPipeFinderAcquisitionRequest {
        language_id,
        project_root,
        locator_root,
        query: intent,
        owners: scopes,
        ignore_dirs: &config.search.ignore_dirs,
        include_hidden_dirs: &config.search.include_hidden_dirs,
        limit: PIPE_CANDIDATE_LINE_LIMIT,
    })?;
    let candidates = acquisition
        .candidates
        .into_iter()
        .map(Candidate::from)
        .collect::<Vec<_>>();
    Ok(CandidateAcquisition {
        candidate_sources: vec!["finder".to_string()],
        source_trace: vec![
            candidate_trace("finder", &candidates).with_fields(elapsed_fields(acquisition.elapsed)),
        ],
        candidates,
    })
}

fn provider_candidates() -> Result<CandidateAcquisition, String> {
    Ok(CandidateAcquisition {
        candidates: Vec::new(),
        candidate_sources: vec!["provider".to_string()],
        source_trace: vec![
            SearchPipeSourceTrace::new("provider", "partial", 0, 1, 0),
            SearchPipeSourceTrace::new("finder", "skipped", 0, 0, 0),
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
