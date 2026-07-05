//! Query-wrapper filesystem candidate scanning.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::dynamic_overlay::QUERY_OVERLAY_ROUTE_SOURCE;
use crate::language_neutral_search_file_spec;
use crate::search_candidate::{RankedSearchCandidate, SearchStageReceipt};

pub const QUERY_WRAPPER_CANDIDATE_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryWrapperCandidateSurface {
    Fd,
    Rg,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperCandidate {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub symbol: String,
    pub selector: Option<String>,
    pub text: String,
    pub source: String,
    pub confidence: String,
}

#[derive(Clone, Copy)]
pub struct QueryWrapperScanConfig<'a> {
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
}

pub struct QueryCandidateAppend<'a> {
    pub surface: QueryWrapperCandidateSurface,
    pub locator_root: &'a Path,
    pub path: &'a Path,
    pub terms: &'a [String],
    pub axis_terms: &'a [String],
    pub config: QueryWrapperScanConfig<'a>,
    pub accept_all_files: bool,
    pub seen: &'a mut HashSet<String>,
    pub candidates: &'a mut Vec<QueryWrapperCandidate>,
}

pub struct QueryWrapperSourceIndexRequest<'a> {
    pub surface: QueryWrapperCandidateSurface,
    pub project_root: &'a Path,
    pub roots: &'a [PathBuf],
    pub terms: &'a [String],
    pub axis_terms: &'a [String],
    pub lookup: &'a QueryWrapperSourceIndexLookup,
}

pub struct QueryWrapperSourceIndexCollection {
    pub candidates: Vec<QueryWrapperCandidate>,
}

pub struct QueryWrapperSearchCandidateRequest<'a> {
    pub project_root: &'a Path,
    pub roots: &'a [PathBuf],
    pub terms: &'a [String],
    pub axis_terms: &'a [String],
    pub ranked: &'a [RankedSearchCandidate],
}

pub struct QueryWrapperSearchCandidateCollection {
    pub candidates: Vec<QueryWrapperCandidate>,
    pub stage_receipt: SearchStageReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperSourceIndexLookup {
    pub db_path: PathBuf,
    pub state: String,
    pub candidates: Vec<QueryWrapperSourceIndexCandidate>,
}

impl QueryWrapperSourceIndexLookup {
    #[must_use]
    pub fn new(
        db_path: PathBuf,
        state: impl Into<String>,
        candidates: Vec<QueryWrapperSourceIndexCandidate>,
    ) -> Self {
        Self {
            db_path,
            state: state.into(),
            candidates,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperSourceIndexCandidate {
    pub path: String,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
    pub source_kind: String,
    pub line_count: Option<u32>,
    pub query_keys: Vec<String>,
}

impl QueryWrapperSourceIndexCandidate {
    #[must_use]
    pub fn new(request: QueryWrapperSourceIndexCandidateRequest) -> Self {
        Self {
            path: request.path,
            language_id: request.language_id,
            provider_id: request.provider_id,
            source_kind: request.source_kind,
            line_count: request.line_count,
            query_keys: request.query_keys,
        }
    }
}

pub struct QueryWrapperSourceIndexCandidateRequest {
    path: String,
    language_id: Option<String>,
    provider_id: Option<String>,
    source_kind: String,
    line_count: Option<u32>,
    query_keys: Vec<String>,
}

impl QueryWrapperSourceIndexCandidateRequest {
    pub fn new(
        path: impl Into<String>,
        language_id: Option<String>,
        provider_id: Option<String>,
        source_kind: impl Into<String>,
        line_count: Option<u32>,
        query_keys: Vec<String>,
    ) -> Self {
        Self {
            path: path.into(),
            language_id,
            provider_id,
            source_kind: source_kind.into(),
            line_count,
            query_keys,
        }
    }
}

impl<P, S>
    From<(
        P,
        Option<String>,
        Option<String>,
        S,
        Option<u32>,
        Vec<String>,
    )> for QueryWrapperSourceIndexCandidateRequest
where
    P: Into<String>,
    S: Into<String>,
{
    fn from(
        (path, language_id, provider_id, source_kind, line_count, query_keys): (
            P,
            Option<String>,
            Option<String>,
            S,
            Option<u32>,
            Vec<String>,
        ),
    ) -> Self {
        Self {
            path: path.into(),
            language_id,
            provider_id,
            source_kind: source_kind.into(),
            line_count,
            query_keys,
        }
    }
}

pub fn append_query_candidates(input: QueryCandidateAppend<'_>) -> Result<(), String> {
    let QueryCandidateAppend {
        surface,
        locator_root,
        path,
        terms,
        axis_terms,
        config,
        accept_all_files,
        seen,
        candidates,
    } = input;
    if candidates.len() >= QUERY_WRAPPER_CANDIDATE_LIMIT || !path.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect query wrapper path {}: {error}",
            path.display()
        )
    })?;
    if metadata.is_file() {
        append_file_query_candidates(
            surface,
            locator_root,
            path,
            terms,
            accept_all_files,
            seen,
            candidates,
        );
        return Ok(());
    }
    let mut entries = fs::read_dir(path)
        .map_err(|error| {
            format!(
                "failed to read query wrapper dir {}: {error}",
                path.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read query wrapper entry under {}: {error}",
                path.display()
            )
        })?;
    entries.sort_by_key(|entry| path_priority(&entry.path(), terms, axis_terms));
    for entry in entries {
        if candidates.len() >= QUERY_WRAPPER_CANDIDATE_LIMIT {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect query wrapper path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            if should_skip_dir(&path, config) {
                continue;
            }
            append_query_candidates(QueryCandidateAppend {
                surface,
                locator_root,
                path: &path,
                terms,
                axis_terms,
                config,
                accept_all_files,
                seen,
                candidates,
            })?;
        } else if file_type.is_file() {
            append_file_query_candidates(
                surface,
                locator_root,
                &path,
                terms,
                accept_all_files,
                seen,
                candidates,
            );
        }
    }
    Ok(())
}

pub fn augment_package_path_candidates(
    locator_root: &Path,
    roots: &[PathBuf],
    terms: &[String],
    config: QueryWrapperScanConfig<'_>,
    candidates: &mut Vec<QueryWrapperCandidate>,
) -> Result<usize, String> {
    let package_terms = terms
        .iter()
        .filter(|term| term.contains('_'))
        .cloned()
        .collect::<Vec<_>>();
    if package_terms.is_empty() {
        return Ok(0);
    }
    let missing_package_terms = package_terms
        .into_iter()
        .filter(|term| {
            !candidates
                .iter()
                .any(|candidate| candidate_covers_term(candidate, term))
        })
        .collect::<Vec<_>>();
    if missing_package_terms.is_empty() {
        return Ok(0);
    }
    let mut package_candidates = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        append_query_candidates(QueryCandidateAppend {
            surface: QueryWrapperCandidateSurface::Fd,
            locator_root,
            path: root,
            terms: &missing_package_terms,
            axis_terms: &missing_package_terms,
            config,
            accept_all_files: false,
            seen: &mut seen,
            candidates: &mut package_candidates,
        })?;
    }
    let mut existing = candidates
        .iter()
        .map(|candidate| (candidate.path.clone(), candidate.symbol.clone()))
        .collect::<HashSet<_>>();
    let mut added = 0usize;
    for candidate in package_candidates {
        if existing.insert((candidate.path.clone(), candidate.symbol.clone())) {
            candidates.push(QueryWrapperCandidate {
                source: QUERY_OVERLAY_ROUTE_SOURCE.to_string(),
                confidence: "package-path".to_string(),
                ..candidate
            });
            added += 1;
        }
    }
    Ok(added)
}

fn append_file_query_candidates(
    surface: QueryWrapperCandidateSurface,
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    accept_all_files: bool,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<QueryWrapperCandidate>,
) {
    if !accept_all_files && !supported_query_file(path) {
        return;
    }
    match surface {
        QueryWrapperCandidateSurface::Fd => {
            append_path_candidate(locator_root, path, terms, seen, candidates)
        }
        QueryWrapperCandidateSurface::Rg => {
            append_content_candidates(locator_root, path, terms, seen, candidates)
        }
    }
}

fn candidate_covers_term(candidate: &QueryWrapperCandidate, term: &str) -> bool {
    format!("{} {} {}", candidate.path, candidate.symbol, candidate.text)
        .to_ascii_lowercase()
        .contains(term)
}

fn supported_query_file(path: &Path) -> bool {
    language_neutral_search_file_spec().matches(path)
}

fn append_path_candidate(
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<QueryWrapperCandidate>,
) {
    let display = display_path(locator_root, path);
    let lower = display.to_ascii_lowercase();
    let Some(term) = terms.iter().find(|term| lower.contains(term.as_str())) else {
        return;
    };
    let key = format!("{display}:1:{term}");
    if !seen.insert(key) {
        return;
    }
    candidates.push(QueryWrapperCandidate {
        path: display.clone(),
        line: 1,
        end_line: 1,
        symbol: term.clone(),
        selector: None,
        text: display,
        source: QUERY_OVERLAY_ROUTE_SOURCE.to_string(),
        confidence: "path".to_string(),
    });
}

fn append_content_candidates(
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<QueryWrapperCandidate>,
) {
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        if candidates.len() >= QUERY_WRAPPER_CANDIDATE_LIMIT {
            break;
        }
        let lower = line.to_ascii_lowercase();
        let Some(term) = terms.iter().find(|term| lower.contains(term.as_str())) else {
            continue;
        };
        let display = display_path(locator_root, path);
        let line_number = line_index + 1;
        let key = format!("{display}:{line_number}:{term}");
        if !seen.insert(key) {
            continue;
        }
        candidates.push(QueryWrapperCandidate {
            path: display,
            line: line_number,
            end_line: line_number,
            symbol: term.clone(),
            selector: None,
            text: line.to_string(),
            source: QUERY_OVERLAY_ROUTE_SOURCE.to_string(),
            confidence: "content".to_string(),
        });
    }
}

fn path_priority(
    path: &Path,
    terms: &[String],
    axis_terms: &[String],
) -> (Reverse<usize>, u8, u8, u8, String) {
    let display = path.to_string_lossy().replace('\\', "/");
    let lower = display.to_ascii_lowercase();
    let secondary_priority = secondary_owner_priority(&lower, terms);
    let axis_coverage = query_axis_coverage(&lower, axis_terms);
    let query_priority = if terms.iter().any(|term| path_basename_matches(&lower, term)) {
        0
    } else if terms.iter().any(|term| lower.contains(term)) {
        1
    } else {
        2
    };
    let layout_priority = if display.ends_with("/src") || display.contains("/src/") {
        0
    } else if display.contains("/test") || display.contains("/examples/") {
        2
    } else {
        1
    };
    (
        Reverse(axis_coverage),
        secondary_priority,
        query_priority,
        layout_priority,
        display,
    )
}

fn path_basename_matches(lower_path: &str, term: &str) -> bool {
    lower_path
        .rsplit('/')
        .next()
        .map(|name| {
            name == term
                || name
                    .rsplit_once('.')
                    .map(|(stem, _)| stem == term)
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub(crate) fn query_candidate_priority(
    path: &str,
    terms: &[String],
    axis_terms: &[String],
) -> (Reverse<usize>, u8, u8, u8, u8, String) {
    let lower = path.to_ascii_lowercase();
    let secondary_priority = secondary_owner_priority(&lower, terms);
    let axis_coverage = query_axis_coverage(&lower, axis_terms);
    let query_priority = if terms.iter().any(|term| path_basename_matches(&lower, term)) {
        0
    } else if terms.iter().any(|term| lower.contains(term)) {
        1
    } else {
        2
    };
    let owner_priority = if lower.contains("/internal/") { 1 } else { 0 };
    let runtime_priority = if lower.contains("/src/") || lower.starts_with("src/") {
        0
    } else if lower.contains("/tests/")
        || lower.starts_with("tests/")
        || lower.contains("/test/")
        || lower.contains("/examples/")
    {
        2
    } else {
        1
    };
    (
        Reverse(axis_coverage),
        secondary_priority,
        query_priority,
        runtime_priority,
        owner_priority,
        lower,
    )
}

fn query_axis_coverage(lower_path: &str, terms: &[String]) -> usize {
    terms
        .iter()
        .filter(|term| lower_path.contains(term.as_str()))
        .count()
}

fn secondary_owner_priority(lower_path: &str, terms: &[String]) -> u8 {
    if has_strong_secondary_owner_intent(terms.iter().map(String::as_str)) {
        return 0;
    }
    u8::from(secondary_like_owner(lower_path))
}

fn secondary_like_owner(owner: &str) -> bool {
    owner
        .split(['/', '\\', '.', '-', '_'])
        .any(|part| secondary_owner_role_token(part.to_ascii_lowercase().as_str()))
}

fn has_strong_secondary_owner_intent<'a>(terms: impl IntoIterator<Item = &'a str>) -> bool {
    terms
        .into_iter()
        .filter(|term| secondary_owner_role_token(term.to_ascii_lowercase().as_str()))
        .take(2)
        .count()
        >= 2
}

fn secondary_owner_role_token(token: &str) -> bool {
    matches!(
        token,
        "test"
            | "tests"
            | "unittest"
            | "unittests"
            | "spec"
            | "specs"
            | "fixture"
            | "fixtures"
            | "baseline"
            | "baselines"
            | "case"
            | "cases"
            | "template"
            | "templates"
            | "example"
            | "examples"
            | "sample"
            | "samples"
            | "demo"
            | "demos"
            | "bench"
            | "benches"
            | "benchmark"
            | "benchmarks"
    )
}

fn should_skip_dir(path: &Path, config: QueryWrapperScanConfig<'_>) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') && !config.include_hidden_dirs.iter().any(|dir| dir == name) {
        return true;
    }
    config.ignore_dirs.iter().any(|dir| dir == name)
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
