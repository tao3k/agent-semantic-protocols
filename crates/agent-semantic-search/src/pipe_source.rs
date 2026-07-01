//! Search-pipe source acquisition services.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use orgize::document::DocumentLanguage;

use crate::{
    DocumentSearchCandidate, DocumentSearchCandidateRequest, SearchPipeCandidate,
    SearchPipeCandidateRequest, collect_document_search_candidates, collect_search_pipe_candidates,
};

const SOURCE_INDEX_GATE_GENERIC_TERMS: &[&str] = &[
    "action",
    "actions",
    "code",
    "collectms",
    "command",
    "compact",
    "elapsedms",
    "fd",
    "frontier",
    "gate",
    "graph",
    "items",
    "latency",
    "low",
    "milliseconds",
    "owner",
    "performance",
    "pipe",
    "quality",
    "query",
    "render",
    "rg",
    "route",
    "search",
    "selector",
    "sourceindex",
    "trace",
    "words",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeFinderAcquisition {
    pub candidates: Vec<SearchPipeCandidate>,
    pub elapsed: Duration,
}

pub struct SearchPipeFinderAcquisitionRequest<'a> {
    pub language_id: &'a str,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub query: &'a str,
    pub owners: &'a [PathBuf],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub limit: usize,
}

pub fn collect_search_pipe_finder_acquisition(
    request: SearchPipeFinderAcquisitionRequest<'_>,
) -> Result<SearchPipeFinderAcquisition, String> {
    let started_at = Instant::now();
    let candidates = collect_search_pipe_candidates(SearchPipeCandidateRequest {
        language_id: request.language_id,
        project_root: request.project_root,
        locator_root: request.locator_root,
        query: request.query,
        owners: request.owners,
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        limit: request.limit,
    })?;
    Ok(SearchPipeFinderAcquisition {
        candidates,
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
}

pub fn collect_search_pipe_failure_acquisition(
    request: SearchPipeFailureAcquisitionRequest<'_>,
) -> Result<SearchPipeFinderAcquisition, String> {
    let query = failure_candidate_query(request.message);
    collect_search_pipe_finder_acquisition(SearchPipeFinderAcquisitionRequest {
        language_id: request.language_id,
        project_root: request.project_root,
        locator_root: request.locator_root,
        query: &query,
        owners: &[],
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        limit: request.limit,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPipeSourceMode {
    Auto,
    Provider,
    Finder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceAcquisitionTrace {
    pub source: String,
    pub status: String,
    pub matched: usize,
    pub missing: usize,
    pub normalized: usize,
    pub elapsed: Option<Duration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceAcquisition {
    pub candidates: Vec<SearchPipeCandidate>,
    pub candidate_sources: Vec<String>,
    pub source_trace: Vec<SearchPipeSourceAcquisitionTrace>,
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
    pub finder_limit: usize,
}

pub fn collect_search_pipe_document_acquisition(
    request: SearchPipeDocumentAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    match request.mode {
        SearchPipeSourceMode::Auto => document_auto_acquisition(request),
        SearchPipeSourceMode::Provider => document_element_acquisition(request),
        SearchPipeSourceMode::Finder => {
            finder_source_acquisition(SearchPipeFinderAcquisitionRequest {
                language_id: request.language.id(),
                project_root: request.project_root,
                locator_root: request.locator_root,
                query: request.intent,
                owners: request.scopes,
                ignore_dirs: request.ignore_dirs,
                include_hidden_dirs: request.include_hidden_dirs,
                limit: request.finder_limit,
            })
        }
    }
}

fn document_auto_acquisition(
    request: SearchPipeDocumentAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    let document_acquisition =
        document_element_acquisition(SearchPipeDocumentAcquisitionRequest {
            mode: SearchPipeSourceMode::Provider,
            ..request
        })?;
    if !document_acquisition.candidates.is_empty() {
        return Ok(document_acquisition);
    }
    let finder_acquisition = finder_source_acquisition(SearchPipeFinderAcquisitionRequest {
        language_id: request.language.id(),
        project_root: request.project_root,
        locator_root: request.locator_root,
        query: request.intent,
        owners: request.scopes,
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        limit: request.finder_limit,
    })?;
    let mut source_trace = document_acquisition.source_trace;
    source_trace.extend(finder_acquisition.source_trace);
    Ok(SearchPipeSourceAcquisition {
        candidates: finder_acquisition.candidates,
        candidate_sources: vec!["document-element".to_string(), "finder".to_string()],
        source_trace,
    })
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
        }],
        candidate_sources: vec!["document-element".to_string()],
        candidates,
    })
}

fn finder_source_acquisition(
    request: SearchPipeFinderAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    let acquisition = collect_search_pipe_finder_acquisition(SearchPipeFinderAcquisitionRequest {
        language_id: request.language_id,
        project_root: request.project_root,
        locator_root: request.locator_root,
        query: request.query,
        owners: request.owners,
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
        limit: request.limit,
    })?;
    let candidates = acquisition.candidates;
    Ok(SearchPipeSourceAcquisition {
        source_trace: vec![candidate_trace(
            "finder",
            &candidates,
            Some(acquisition.elapsed),
        )],
        candidate_sources: vec!["finder".to_string()],
        candidates,
    })
}

fn document_candidate(candidate: DocumentSearchCandidate) -> SearchPipeCandidate {
    SearchPipeCandidate {
        path: candidate.path,
        line: candidate.line,
        end_line: candidate.end_line,
        symbol: candidate.symbol,
        text: candidate.text,
        source: "document-element".to_string(),
        confidence: "parser".to_string(),
    }
}

fn candidate_trace(
    source: &str,
    candidates: &[SearchPipeCandidate],
    elapsed: Option<Duration>,
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
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexCandidate {
    pub path: String,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
    pub source_kind: String,
    pub line_count: Option<u32>,
    pub query_keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexLookup {
    pub state: String,
    pub candidates: Vec<SearchPipeSourceIndexCandidate>,
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
    UseAndSkipFinder,
    Fallthrough,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeSourceIndexAcquisition {
    pub decision: SearchPipeSourceIndexDecision,
    pub gate: Option<SearchPipeSourceIndexGate>,
    pub candidates: Vec<SearchPipeCandidate>,
}

pub struct SearchPipeSourceIndexAcquisitionRequest<'a> {
    pub intent: &'a str,
    pub scopes: &'a [PathBuf],
    pub lookup: Option<&'a SearchPipeSourceIndexLookup>,
}

pub fn collect_search_pipe_source_index_acquisition(
    request: SearchPipeSourceIndexAcquisitionRequest<'_>,
) -> Option<SearchPipeSourceIndexAcquisition> {
    if !request.scopes.is_empty() {
        return None;
    }
    if let Some(gate) = source_index_query_gate(request.intent) {
        return Some(SearchPipeSourceIndexAcquisition {
            decision: SearchPipeSourceIndexDecision::QueryGate,
            gate: Some(gate),
            candidates: Vec::new(),
        });
    }
    let lookup = request.lookup?;
    let candidates = lookup
        .candidates
        .iter()
        .map(source_index_candidate)
        .collect::<Vec<_>>();
    let decision = if intent_terms_all_path_like(request.intent)
        && matches!(lookup.state.as_str(), "missing-db" | "empty-index" | "miss")
    {
        SearchPipeSourceIndexDecision::DeferBackend
    } else if candidates.is_empty() {
        SearchPipeSourceIndexDecision::Fallthrough
    } else {
        SearchPipeSourceIndexDecision::UseAndSkipFinder
    };
    Some(SearchPipeSourceIndexAcquisition {
        decision,
        gate: None,
        candidates,
    })
}

fn source_index_query_gate(intent: &str) -> Option<SearchPipeSourceIndexGate> {
    let terms = source_index_gate_terms(intent);
    if terms.is_empty() {
        return None;
    }
    let generic_term_count = terms
        .iter()
        .filter(|term| SOURCE_INDEX_GATE_GENERIC_TERMS.contains(&term.as_str()))
        .count();
    let all_generic = generic_term_count == terms.len() && terms.len() >= 2;
    let broad_generic = terms.len() >= 8 && generic_term_count * 2 >= terms.len();
    if !all_generic && !broad_generic {
        return None;
    }
    Some(SearchPipeSourceIndexGate {
        term_count: terms.len(),
        generic_term_count,
    })
}

fn source_index_gate_terms(intent: &str) -> Vec<String> {
    intent
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == &term) {
                terms.push(term);
            }
            terms
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

fn source_index_candidate(candidate: &SearchPipeSourceIndexCandidate) -> SearchPipeCandidate {
    let line_count = candidate.line_count.unwrap_or(1).max(1) as usize;
    SearchPipeCandidate {
        path: candidate.path.clone(),
        line: 1,
        end_line: line_count,
        symbol: source_index_symbol(candidate),
        text: source_index_candidate_text(candidate),
        source: "source-index".to_string(),
        confidence: "db-engine".to_string(),
    }
}

fn source_index_symbol(candidate: &SearchPipeSourceIndexCandidate) -> String {
    candidate
        .query_keys
        .first()
        .cloned()
        .unwrap_or_else(|| symbol_from_path(&candidate.path))
}

fn source_index_candidate_text(candidate: &SearchPipeSourceIndexCandidate) -> String {
    let language = candidate.language_id.as_deref().unwrap_or("unknown");
    let provider = candidate.provider_id.as_deref().unwrap_or("unknown");
    let keys = candidate
        .query_keys
        .iter()
        .take(8)
        .cloned()
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "source-index path={} language={} provider={} kind={} queryKeys={}",
        candidate.path, language, provider, candidate.source_kind, keys
    )
}

fn symbol_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("source")
        .to_string()
}
