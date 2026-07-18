use std::collections::BTreeSet;

use crate::{SearchPipeQueryTerm, SearchPipeTermRole};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPipeCohesionTerm {
    pub raw: String,
    pub lower: String,
}

impl SearchPipeCohesionTerm {
    #[must_use]
    pub fn new(raw: impl Into<String>, lower: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            lower: lower.into(),
        }
    }
}

#[must_use]
pub fn search_pipe_package_key(path: &str) -> String {
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        return parts[index..(index + 3).min(parts.len())].join("/");
    }
    if let Some(index) = parts.iter().position(|part| *part == "crates") {
        return parts[index..(index + 2).min(parts.len())].join("/");
    }
    parts.into_iter().take(2).collect::<Vec<_>>().join("/")
}

#[must_use]
pub fn search_pipe_candidate_packages(paths: impl Iterator<Item = String>) -> Vec<String> {
    let mut packages = BTreeSet::new();
    paths
        .filter_map(|path| {
            packages
                .insert(search_pipe_package_key(&path))
                .then_some(())
        })
        .take(6)
        .for_each(drop);
    packages.into_iter().collect()
}

#[must_use]
pub fn search_pipe_package_cohesion(
    packages: &[String],
    best_owner_matched: Option<&[String]>,
    high_value_terms: &[SearchPipeCohesionTerm],
) -> String {
    let high_value_count = high_value_terms.len().max(1);
    let best_owner_high_value_hits = best_owner_matched
        .map(|matched| {
            high_value_terms
                .iter()
                .filter(|term| matched.iter().any(|matched| matched == &term.lower))
                .count()
        })
        .unwrap_or_default();
    let package_axis_terms = high_value_terms
        .iter()
        .filter(|term| is_search_pipe_package_axis_term(&term.raw))
        .collect::<Vec<_>>();
    let best_owner_package_axis_hits = best_owner_matched
        .map(|matched| {
            package_axis_terms
                .iter()
                .filter(|term| matched.iter().any(|matched| matched == &term.lower))
                .count()
        })
        .unwrap_or_default();
    let has_strong_owner_anchor =
        high_value_terms.len() >= 2 && best_owner_high_value_hits >= high_value_terms.len();
    if (package_axis_terms.len() > 1 && best_owner_package_axis_hits < package_axis_terms.len())
        || (packages.len() > 3 && !has_strong_owner_anchor)
        || best_owner_high_value_hits * 2 < high_value_count
    {
        "low".to_string()
    } else if packages.len() > 1 {
        "medium".to_string()
    } else {
        "high".to_string()
    }
}

#[must_use]
pub fn is_search_pipe_package_axis_term(raw: &str) -> bool {
    raw.matches('-').count() >= 2 && !matches!(raw, "long-field-signatures")
}

#[must_use]
pub fn search_pipe_quality_risks(
    terms: &[SearchPipeQueryTerm],
    mut candidate_texts: impl Iterator<Item = String>,
    global_missing: &[String],
    strong_matched: &[String],
    weak_terms: &[String],
    package_cohesion: &str,
    clause_count: usize,
) -> Vec<String> {
    let mut risks = Vec::new();
    if clause_count == 1
        && terms.len() >= 5
        && terms.iter().filter(|term| is_high_value_term(term)).count() >= 3
    {
        risks.push("single-broad-clause".to_string());
    }
    if global_missing.is_empty() && !weak_terms.is_empty() {
        risks.push("coverage-inflation".to_string());
    }
    if package_cohesion == "low" {
        risks.push("package-drift".to_string());
    }
    if terms.iter().any(is_high_value_term) && !weak_terms.is_empty() {
        risks.push("weak-camelcase-match".to_string());
    }
    if candidate_texts.any(|text| text.len() > 160 || text.contains('\n')) {
        risks.push("long-field-signatures".to_string());
    }
    if strong_matched.is_empty() && terms.iter().filter(|term| is_high_value_term(term)).count() > 1
    {
        risks.push("no-strong-symbol-coverage".to_string());
    }
    risks
}

#[must_use]
pub fn search_pipe_query_pack_quality(
    terms: &[SearchPipeQueryTerm],
    global_missing: &[String],
    weak_terms: &[String],
    risks: &[String],
) -> String {
    if risks.iter().any(|risk| {
        matches!(
            risk.as_str(),
            "single-broad-clause" | "package-drift" | "no-strong-symbol-coverage"
        )
    }) {
        "low"
    } else if weak_terms.is_empty() && global_missing.is_empty() {
        "high"
    } else if terms.is_empty() {
        "low"
    } else {
        "medium"
    }
    .to_string()
}

#[must_use]
pub fn search_pipe_fd_query_terms(
    terms: &[SearchPipeQueryTerm],
    weak_terms: &[String],
    strong_matched: &[String],
    risks: &[String],
) -> Option<String> {
    let symbol_terms = terms
        .iter()
        .filter(|term| matches!(term.role, SearchPipeTermRole::Symbol))
        .filter(|term| {
            weak_terms.is_empty()
                || weak_terms.iter().any(|weak| weak == &term.raw)
                || strong_matched.iter().any(|matched| matched == &term.raw)
        })
        .map(|term| term.raw.clone())
        .collect::<Vec<_>>();
    if !symbol_terms.is_empty() {
        return Some(symbol_terms.join("|"));
    }
    if !risks
        .iter()
        .any(|risk| matches!(risk.as_str(), "single-broad-clause" | "package-drift"))
    {
        return None;
    }
    let owner_axis_terms = terms
        .iter()
        .filter(|term| !matches!(term.role, SearchPipeTermRole::Symbol))
        .filter(|term| search_pipe_fd_owner_axis_term(&term.raw))
        .map(|term| term.raw.clone())
        .take(8)
        .collect::<Vec<_>>();
    (!owner_axis_terms.is_empty()).then(|| owner_axis_terms.join("|"))
}

#[must_use]
pub fn search_pipe_missing_path_terms(
    terms: &[SearchPipeQueryTerm],
    global_matched: &[String],
) -> Vec<String> {
    terms
        .iter()
        .filter(|term| crate::search_pipe_is_path_like_token(&term.raw))
        .filter(|term| !global_matched.iter().any(|matched| matched == &term.raw))
        .map(|term| term.raw.clone())
        .collect()
}

#[must_use]
pub fn search_pipe_owner_seed_terms(
    terms: &[SearchPipeQueryTerm],
    missing_path_terms: &[String],
) -> Vec<String> {
    crate::search_pipe_role_terms(terms, SearchPipeTermRole::Symbol)
        .into_iter()
        .filter(|term| !crate::search_pipe_is_path_like_token(term))
        .filter(|term| !missing_path_terms.iter().any(|missing| missing == term))
        .collect()
}

#[must_use]
pub fn search_pipe_fd_owner_axis_term(term: &str) -> bool {
    let lower = term.to_ascii_lowercase();
    if lower.len() < 4 {
        return false;
    }
    if matches!(
        lower.as_str(),
        "query"
            | "search"
            | "pipe"
            | "fd"
            | "rg"
            | "owner"
            | "owners"
            | "graph"
            | "turbo"
            | "command"
            | "commands"
            | "frontier"
            | "frontiers"
            | "action"
            | "actions"
            | "result"
            | "results"
            | "quality"
            | "wide"
            | "drift"
            | "handoff"
    ) {
        return false;
    }
    term.chars()
        .all(|ch| ch == '.' || ch == '_' || ch == '-' || ch.is_ascii_alphanumeric())
}

fn is_high_value_term(term: &SearchPipeQueryTerm) -> bool {
    matches!(term.role, SearchPipeTermRole::Symbol)
}
/// Compatibility surface for legacy query-wrapper source-index trace tests.
/// Compatibility alias for legacy query-wrapper candidate consumers.
pub type QueryWrapperCandidate = crate::SearchPipeCandidate;

/// Compatibility surface for legacy query-wrapper search surface selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryWrapperSearchSurface {
    Fd,
    Rg,
}

/// Compatibility lookup container for legacy query-wrapper source-index tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperSourceIndexLookup {
    pub candidates: Vec<String>,
}

impl QueryWrapperSourceIndexLookup {
    pub fn new(candidates: Vec<String>) -> Self {
        Self { candidates }
    }
}

/// Compatibility projection container for legacy query-wrapper trace tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperSourceIndexTraceProjection {
    pub source: String,
    pub status: String,
    pub candidate_count: usize,
    pub skipped_count: usize,
    pub fields: Vec<String>,
}

pub struct QueryWrapperSearchSourceIndexTrace {
    pub lookup: QueryWrapperSourceIndexLookup,
    pub candidate_count: usize,
    pub elapsed: std::time::Duration,
}

/// Compatibility projection for legacy query-wrapper source-index trace tests.
pub fn query_wrapper_source_index_trace_projection(
    trace: &QueryWrapperSearchSourceIndexTrace,
) -> QueryWrapperSourceIndexTraceProjection {
    QueryWrapperSourceIndexTraceProjection {
        source: String::from("query-wrapper"),
        status: String::from("ok"),
        candidate_count: trace.candidate_count,
        skipped_count: trace
            .lookup
            .candidates
            .len()
            .saturating_sub(trace.candidate_count),
        fields: vec![format!("elapsed_ms={}", trace.elapsed.as_millis())],
    }
}

/// Returns stable owner candidates from legacy query-wrapper path input.
pub fn query_wrapper_owner_candidates<I, P>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<std::path::Path>,
{
    paths
        .into_iter()
        .map(|path| path.as_ref().to_string_lossy().into_owned())
        .collect()
}

/// Returns stable package clusters from legacy query-wrapper path input.
pub fn query_wrapper_package_clusters_from_paths<I, P>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<std::path::Path>,
{
    query_wrapper_package_keys(paths)
}

/// Returns the next rg scope candidates from legacy query-wrapper path input.
pub fn query_wrapper_rg_scope_next<I, P>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<std::path::Path>,
{
    query_wrapper_package_keys(paths)
}

/// Converts raw query strings into search-pipe query clauses.
pub fn query_wrapper_clauses(raw_queries: &[String]) -> Vec<crate::SearchPipeQueryClause> {
    raw_queries
        .iter()
        .flat_map(|query| {
            crate::search_pipe_query_clauses(crate::SearchPipeQueryClausesRequest::new(
                crate::SearchPipeLanguageId::new("rust"),
                crate::SearchPipeQueryText::new(query),
            ))
        })
        .collect()
}

/// Extracts unique clause terms through the search-pipe query-pack API.
pub fn query_wrapper_unique_clause_terms(clauses: &[crate::SearchPipeQueryClause]) -> Vec<String> {
    crate::search_pipe_unique_query_terms(clauses)
        .into_iter()
        .map(|term| term.raw)
        .collect()
}

fn query_wrapper_package_keys<I, P>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = P>,
    P: AsRef<std::path::Path>,
{
    paths
        .into_iter()
        .map(|path| path.as_ref().to_string_lossy().into_owned())
        .map(|path| path.split('/').take(2).collect::<Vec<_>>().join("/"))
        .filter(|key| !key.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}
