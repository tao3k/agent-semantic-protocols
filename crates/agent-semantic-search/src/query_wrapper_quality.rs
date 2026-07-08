//! Query-wrapper clause parsing and quality analysis.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Search-owned representation of one query clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperSearchClause {
    pub id: usize,
    pub raw: String,
    pub terms: Vec<String>,
    pub axis_terms: Vec<String>,
}

/// Search-owned candidate view used by query-wrapper quality analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperQualityCandidate {
    pub path: String,
    pub symbol: String,
    pub text: String,
}

/// Query-wrapper clause coverage result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperClauseCoverage {
    pub id: usize,
    pub matched: Vec<String>,
    pub missing: Vec<String>,
}

/// Query-wrapper quality analysis result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperQuality {
    pub query_pack_quality: String,
    pub scope_quality: String,
    pub package_cohesion: String,
    pub packages: Vec<String>,
    pub risks: Vec<String>,
    pub noise: Vec<String>,
    pub allow_query_selector: bool,
    pub clause_coverages: Vec<QueryWrapperClauseCoverage>,
}

/// Split one query-wrapper raw query into stable lowercase terms.
#[must_use]
pub fn query_wrapper_terms(query: &str) -> Vec<String> {
    query_wrapper_terms_and_axis(query).0
}

/// Expand one query-wrapper raw query into search axes, including identifier
/// components such as camelCase and PascalCase segments.
#[must_use]
pub fn query_wrapper_axis_terms(raw: &str) -> Vec<String> {
    query_wrapper_terms_and_axis(raw).1
}

/// Build query-wrapper clauses from raw query strings.
#[must_use]
pub fn query_wrapper_clauses(queries: &[String]) -> Vec<QueryWrapperSearchClause> {
    queries
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let (terms, axis_terms) = query_wrapper_terms_and_axis(raw);
            (!terms.is_empty()).then_some(QueryWrapperSearchClause {
                id: index + 1,
                raw: raw.clone(),
                terms,
                axis_terms,
            })
        })
        .collect()
}

/// Return deduplicated clause terms in first-seen order.
#[must_use]
pub fn query_wrapper_unique_clause_terms(clauses: &[QueryWrapperSearchClause]) -> Vec<String> {
    let mut terms = Vec::new();
    for clause in clauses {
        for term in &clause.terms {
            push_unique_term(&mut terms, term.clone());
        }
    }
    terms
}

fn query_wrapper_terms_and_axis(raw: &str) -> (Vec<String>, Vec<String>) {
    let mut terms = Vec::new();
    let mut axis_terms = Vec::new();
    for token in raw
        .split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
    {
        let normalized = token.to_ascii_lowercase();
        push_unique_term(&mut terms, normalized.clone());
        push_unique_term(&mut axis_terms, normalized.clone());
        for component in identifier_components(token) {
            if component.len() >= 2 && component != normalized {
                push_unique_term(&mut axis_terms, component);
            }
        }
    }
    (terms, axis_terms)
}

fn push_unique_term(terms: &mut Vec<String>, term: String) {
    if !terms.iter().any(|seen| seen == &term) {
        terms.push(term);
    }
}

fn identifier_components(raw: &str) -> Vec<String> {
    let chars = raw.chars().collect::<Vec<_>>();
    let mut components = Vec::new();
    let mut current = String::new();
    for (index, character) in chars.iter().enumerate() {
        if !character.is_alphanumeric() {
            push_component(&mut components, &mut current);
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|previous| chars.get(previous));
        let next = chars.get(index + 1);
        let uppercase_boundary = character.is_uppercase()
            && previous.is_some_and(|previous| {
                previous.is_lowercase()
                    || previous.is_ascii_digit()
                    || (previous.is_uppercase() && next.is_some_and(|next| next.is_lowercase()))
            });
        if uppercase_boundary {
            push_component(&mut components, &mut current);
        }
        current.push(character.to_ascii_lowercase());
    }
    push_component(&mut components, &mut current);
    components
}

fn push_component(components: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        components.push(std::mem::take(current));
    }
}

/// Analyze query-wrapper quality gates over search-owned candidate facts.
#[must_use]
pub fn analyze_query_wrapper_quality(
    scopes: &[PathBuf],
    clauses: &[QueryWrapperSearchClause],
    terms: &[String],
    candidates: &[QueryWrapperQualityCandidate],
) -> QueryWrapperQuality {
    let scope_quality = query_wrapper_scope_quality(scopes);
    let packages = query_wrapper_package_clusters(candidates);
    let package_count = candidates
        .iter()
        .map(|candidate| query_wrapper_package_key(&candidate.path))
        .filter(|package| !package.is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let package_cohesion = if candidates.is_empty() || package_count == 0 {
        "low"
    } else if package_count <= 1 {
        "high"
    } else if package_count <= 3 {
        "medium"
    } else {
        "low"
    }
    .to_string();
    let clause_coverages = query_wrapper_clause_coverages(clauses, candidates);
    let noise = query_wrapper_noise_paths(candidates);
    let single_or_clause = clauses.len() == 1 && terms.len() > 1;
    let generic_count = terms
        .iter()
        .filter(|term| is_query_wrapper_generic_term(term))
        .count();
    let generic_ratio_high = !terms.is_empty() && generic_count * 5 >= terms.len() * 2;
    let all_clauses_covered = clause_coverages
        .iter()
        .all(|coverage| !coverage.matched.is_empty());
    let flat_recall_risk = single_or_clause
        && (scope_quality == "low"
            || terms.len() >= 4
            || package_cohesion == "low"
            || generic_ratio_high
            || !noise.is_empty());
    let mut risks = Vec::new();
    if candidates.is_empty() {
        risks.push("no-candidates".to_string());
    }
    if single_or_clause {
        risks.push("single-flat-or-recall".to_string());
    }
    if scope_quality == "low" {
        risks.push("broad-scope".to_string());
    }
    if package_cohesion == "low" {
        risks.push("low-package-cohesion".to_string());
    }
    if !all_clauses_covered {
        risks.push("clause-missing".to_string());
    }
    if generic_ratio_high {
        risks.push("generic-terms".to_string());
    }
    if !noise.is_empty() {
        risks.push("noisy-candidates".to_string());
    }
    let query_pack_quality = if candidates.is_empty() || !all_clauses_covered || flat_recall_risk {
        "low"
    } else if clauses.len() >= 2 && scope_quality == "high" && package_cohesion == "high" {
        "high"
    } else {
        "medium"
    }
    .to_string();
    let allow_query_selector =
        clauses.len() >= 2 && query_pack_quality != "low" && package_cohesion != "low";
    QueryWrapperQuality {
        query_pack_quality,
        scope_quality,
        package_cohesion,
        packages,
        risks,
        noise,
        allow_query_selector,
        clause_coverages,
    }
}

/// Return the package/area key used by query-wrapper quality and next-action
/// projection.
#[must_use]
pub fn query_wrapper_package_key(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        let end = (index + 3).min(parts.len());
        return parts[index..end].join("/");
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty() && *part != ".")
        .take(2)
        .collect::<Vec<_>>()
        .join("/")
}

/// Return stable package clusters for query-wrapper candidates.
#[must_use]
pub fn query_wrapper_package_clusters(candidates: &[QueryWrapperQualityCandidate]) -> Vec<String> {
    query_wrapper_unique_take(
        candidates
            .iter()
            .map(|candidate| query_wrapper_package_key(&candidate.path)),
        6,
    )
}

/// Return compact owner path candidates for query-wrapper render hints.
#[must_use]
pub fn query_wrapper_owner_candidates(paths: impl Iterator<Item = String>) -> Vec<String> {
    query_wrapper_unique_take(paths, 8)
}

/// Return compact package clusters for query-wrapper render hints.
#[must_use]
pub fn query_wrapper_package_clusters_from_paths(
    paths: impl Iterator<Item = String>,
) -> Vec<String> {
    query_wrapper_unique_take(paths.map(|path| query_wrapper_package_key(&path)), 6)
}

/// Return the next rg scope candidates for query-wrapper render hints.
#[must_use]
pub fn query_wrapper_rg_scope_next(paths: impl Iterator<Item = String>) -> Vec<String> {
    query_wrapper_unique_take(
        paths
            .map(|path| query_wrapper_package_key(&path))
            .filter(|package| !package.is_empty()),
        3,
    )
}

#[must_use]
pub fn query_wrapper_candidate_matches_term(
    candidate: &QueryWrapperQualityCandidate,
    term: &str,
) -> bool {
    format!("{} {} {}", candidate.path, candidate.symbol, candidate.text)
        .to_ascii_lowercase()
        .contains(term)
}

pub(crate) fn query_wrapper_unique_take(
    values: impl Iterator<Item = String>,
    limit: usize,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}

fn query_wrapper_scope_quality(scopes: &[PathBuf]) -> String {
    if scopes.is_empty()
        || scopes
            .iter()
            .any(|scope| scope.as_os_str().is_empty() || scope.as_path() == Path::new("."))
    {
        "low"
    } else if scopes.len() == 1 {
        "high"
    } else {
        "medium"
    }
    .to_string()
}

fn query_wrapper_clause_coverages(
    clauses: &[QueryWrapperSearchClause],
    candidates: &[QueryWrapperQualityCandidate],
) -> Vec<QueryWrapperClauseCoverage> {
    clauses
        .iter()
        .map(|clause| {
            let matched = clause
                .terms
                .iter()
                .filter(|term| {
                    candidates
                        .iter()
                        .any(|candidate| query_wrapper_candidate_matches_term(candidate, term))
                })
                .cloned()
                .collect::<Vec<_>>();
            let missing = clause
                .terms
                .iter()
                .filter(|term| !matched.iter().any(|matched| matched == *term))
                .cloned()
                .collect::<Vec<_>>();
            QueryWrapperClauseCoverage {
                id: clause.id,
                matched,
                missing,
            }
        })
        .collect()
}

fn query_wrapper_noise_paths(candidates: &[QueryWrapperQualityCandidate]) -> Vec<String> {
    query_wrapper_unique_take(
        candidates
            .iter()
            .filter(|candidate| is_query_wrapper_noise_path(&candidate.path))
            .map(|candidate| {
                let package = query_wrapper_package_key(&candidate.path);
                if package.is_empty() {
                    candidate.path.clone()
                } else {
                    package
                }
            }),
        6,
    )
}

fn is_query_wrapper_noise_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/analyzers/")
        || lower.starts_with("analyzers/")
        || lower.contains("/notebooks/")
        || lower.starts_with("notebooks/")
        || lower.contains("/experiments/")
        || lower.starts_with("experiments/")
}

fn is_query_wrapper_generic_term(term: &str) -> bool {
    matches!(
        term,
        "asp"
            | "rg"
            | "fd"
            | "query"
            | "search"
            | "command"
            | "config"
            | "cache"
            | "provider"
            | "prefix"
            | "scope"
            | "owner"
            | "package"
            | "frontier"
            | "noise"
            | "policy"
            | "stage"
            | "activation"
    )
}
