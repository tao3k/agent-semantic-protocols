//! Query-wrapper clause, scope, package, and noise quality gates.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::search_pipe_model::Candidate;
use super::search_query_wrapper_candidates::{
    candidate_matches_term, package_clusters, package_key,
};
use super::search_query_wrapper_model::{
    QueryWrapperClause, QueryWrapperClauseCoverage, QueryWrapperQuality,
};

pub(super) fn analyze_query_wrapper_quality(
    scopes: &[PathBuf],
    clauses: &[QueryWrapperClause],
    terms: &[String],
    candidates: &[Candidate],
) -> QueryWrapperQuality {
    let scope_quality = wrapper_scope_quality(scopes);
    let packages = package_clusters(candidates);
    let package_count = candidates
        .iter()
        .map(|candidate| package_key(&candidate.path))
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
    let noise = noise_paths(candidates);
    let single_or_clause = clauses.len() == 1 && terms.len() > 1;
    let generic_count = terms
        .iter()
        .filter(|term| is_generic_query_term(term))
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

fn wrapper_scope_quality(scopes: &[PathBuf]) -> String {
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
    clauses: &[QueryWrapperClause],
    candidates: &[Candidate],
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
                        .any(|candidate| candidate_matches_term(candidate, term))
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

fn noise_paths(candidates: &[Candidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .filter(|candidate| is_noise_path(&candidate.path))
            .map(|candidate| {
                let package = package_key(&candidate.path);
                if package.is_empty() {
                    candidate.path.clone()
                } else {
                    package
                }
            }),
        6,
    )
}

fn unique_take(values: impl Iterator<Item = String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}

fn is_noise_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/analyzers/")
        || lower.starts_with("analyzers/")
        || lower.contains("/notebooks/")
        || lower.starts_with("notebooks/")
        || lower.contains("/experiments/")
        || lower.starts_with("experiments/")
        || lower.contains("/generated/")
        || lower.contains("/vendor/")
        || lower.contains("/vendors/")
        || lower.contains("/dist/")
        || lower.contains("/build/")
        || lower.contains("/node_modules/")
}

fn is_generic_query_term(term: &str) -> bool {
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
