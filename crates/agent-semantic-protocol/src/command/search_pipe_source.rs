//! Candidate source selection for ASP-owned search pipe.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_semantic_client::{SourceIndexLookupResult, SourceIndexLookupState, lookup_source_index};
use orgize::document::{
    DocumentElement, DocumentLanguage, DocumentWalkConfig, filter_elements,
    index_project_with_config,
};
use serde_json::Value;

use super::search_config::AspConfig;
use super::search_pipe_candidates::{
    collect_candidates, parse_ingest_candidates, read_piped_stdin,
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
        SourceSpec::Auto => {
            document_auto_candidates(language, project_root, locator_root, intent, scopes, config)
        }
        SourceSpec::Provider => document_element_candidates(
            language,
            project_root,
            locator_root,
            intent,
            scopes,
            config,
        ),
        SourceSpec::Finder => finder_candidates(
            language.id(),
            project_root,
            locator_root,
            intent,
            scopes,
            config,
        ),
        SourceSpec::Ingest => ingest_candidates(project_root, locator_root),
    }
}

fn document_auto_candidates(
    language: DocumentLanguage,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    config: &AspConfig,
) -> Result<CandidateAcquisition, String> {
    let document_acquisition =
        document_element_candidates(language, project_root, locator_root, intent, scopes, config)?;
    if !document_acquisition.candidates.is_empty() {
        return Ok(document_acquisition);
    }
    let finder_acquisition = finder_candidates(
        language.id(),
        project_root,
        locator_root,
        intent,
        scopes,
        config,
    )?;
    let mut source_trace = document_acquisition.source_trace;
    source_trace.extend(finder_acquisition.source_trace);
    Ok(CandidateAcquisition {
        candidates: finder_acquisition.candidates,
        candidate_sources: vec!["document-element".to_string(), "finder".to_string()],
        source_trace,
    })
}

fn document_element_candidates(
    language: DocumentLanguage,
    project_root: &Path,
    locator_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
    config: &AspConfig,
) -> Result<CandidateAcquisition, String> {
    let walk_config = DocumentWalkConfig::new(
        config.search.ignore_dirs.clone(),
        config.search.include_hidden_dirs.clone(),
    );
    let mut elements = Vec::new();
    for root in document_search_roots(project_root, scopes) {
        elements.extend(index_project_with_config(language, &root, &walk_config)?);
    }
    let matches = filter_elements(&elements, intent);
    let candidates = matches
        .iter()
        .take(DOCUMENT_PIPE_CANDIDATE_LIMIT)
        .map(|element| document_candidate(element, locator_root))
        .collect::<Vec<_>>();
    Ok(CandidateAcquisition {
        source_trace: vec![SearchPipeSourceTrace::new(
            "document-element",
            if candidates.is_empty() {
                "empty"
            } else {
                "used"
            },
            candidates.len(),
            usize::from(candidates.is_empty()),
            matches.len(),
        )],
        candidate_sources: vec!["document-element".to_string()],
        candidates,
    })
}

fn document_search_roots(project_root: &Path, scopes: &[PathBuf]) -> Vec<PathBuf> {
    if scopes.is_empty() {
        return vec![project_root.to_path_buf()];
    }
    scopes
        .iter()
        .map(|scope| {
            if scope.is_absolute() {
                scope.clone()
            } else {
                project_root.join(scope)
            }
        })
        .collect()
}

fn document_candidate(element: &DocumentElement, locator_root: &Path) -> Candidate {
    Candidate {
        path: display_document_path(locator_root, &element.path),
        line: element.line,
        end_line: element.end_line.max(element.line),
        symbol: document_symbol(element),
        text: document_candidate_text(element),
        source: "document-element".to_string(),
        confidence: "parser".to_string(),
    }
}

fn document_symbol(element: &DocumentElement) -> String {
    element
        .fields
        .iter()
        .find(|(key, value)| {
            matches!(
                key.as_str(),
                "title" | "key" | "value" | "lang" | "target" | "description"
            ) && !value.trim().is_empty()
        })
        .map(|(_, value)| symbol_from_text(value))
        .filter(|symbol| !symbol.is_empty())
        .unwrap_or_else(|| element.kind.to_string())
}

fn document_candidate_text(element: &DocumentElement) -> String {
    let mut text = format!("{} {}", element.kind, element.source_kind);
    for (key, value) in &element.fields {
        if !value.trim().is_empty() {
            text.push(' ');
            text.push_str(key);
            text.push('=');
            text.push_str(value);
        }
    }
    if !element.text.trim().is_empty() {
        text.push(' ');
        text.push_str(element.text.trim());
    } else if !element.content.trim().is_empty() {
        text.push(' ');
        text.push_str(element.content.trim());
    }
    text
}

fn display_document_path(locator_root: &Path, path: &str) -> String {
    let path = Path::new(path);
    path.strip_prefix(locator_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn symbol_from_text(text: &str) -> String {
    text.split(|character: char| {
        !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
    .find(|part| !part.is_empty())
    .unwrap_or("document")
    .to_ascii_lowercase()
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
    let source_index_acquisition = source_index_candidates(project_root, intent, scopes);
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
    let started_at = Instant::now();
    let candidates = collect_candidates(
        language_id,
        project_root,
        locator_root,
        intent,
        scopes,
        config,
    )?;
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
                .with_fields(elapsed_fields(started_at.elapsed())),
                candidate_trace("finder", &candidates)
                    .with_fields(elapsed_fields(started_at.elapsed())),
            ])
            .collect(),
        candidates,
    })
}

fn source_index_candidates(
    project_root: &Path,
    intent: &str,
    scopes: &[PathBuf],
) -> Option<CandidateAcquisition> {
    if !scopes.is_empty() {
        return None;
    }
    let started_at = Instant::now();
    match lookup_source_index(project_root, intent, DOCUMENT_PIPE_CANDIDATE_LIMIT as u32) {
        Ok(result) => {
            let candidates = result
                .candidates
                .iter()
                .map(source_index_candidate)
                .collect::<Vec<_>>();
            let mut source_trace = vec![source_index_trace(&result, candidates.len(), started_at)];
            if !candidates.is_empty() {
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

fn source_index_trace_prefix(
    acquisition: Option<CandidateAcquisition>,
) -> Vec<SearchPipeSourceTrace> {
    acquisition
        .map(|acquisition| acquisition.source_trace)
        .unwrap_or_default()
}

fn source_index_trace(
    result: &SourceIndexLookupResult,
    candidate_count: usize,
    started_at: Instant,
) -> SearchPipeSourceTrace {
    let mut fields = elapsed_fields(started_at.elapsed());
    fields.insert("state".to_string(), Value::from(result.state.as_str()));
    fields.insert(
        "dbPath".to_string(),
        Value::from(result.db_path.display().to_string()),
    );
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

fn source_index_candidate(candidate: &agent_semantic_client::SourceIndexCandidate) -> Candidate {
    let line_count = candidate.line_count.unwrap_or(1).max(1) as usize;
    Candidate {
        path: candidate.path.clone(),
        line: 1,
        end_line: line_count,
        symbol: source_index_symbol(candidate),
        text: source_index_candidate_text(candidate),
        source: "source-index".to_string(),
        confidence: "rust-sql".to_string(),
    }
}

fn source_index_symbol(candidate: &agent_semantic_client::SourceIndexCandidate) -> String {
    candidate
        .query_keys
        .first()
        .cloned()
        .unwrap_or_else(|| symbol_from_path(&candidate.path))
}

fn source_index_candidate_text(candidate: &agent_semantic_client::SourceIndexCandidate) -> String {
    let language = candidate
        .language_id
        .as_ref()
        .map(|value| value.as_str())
        .unwrap_or("unknown");
    let provider = candidate
        .provider_id
        .as_ref()
        .map(|value| value.as_str())
        .unwrap_or("unknown");
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
    let started_at = Instant::now();
    let candidates = collect_candidates(
        language_id,
        project_root,
        locator_root,
        intent,
        scopes,
        config,
    )?;
    Ok(CandidateAcquisition {
        candidate_sources: vec!["finder".to_string()],
        source_trace: vec![
            candidate_trace("finder", &candidates)
                .with_fields(elapsed_fields(started_at.elapsed())),
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
