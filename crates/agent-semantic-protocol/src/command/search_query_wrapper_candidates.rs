//! Candidate collection and finder previews for query wrappers.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::search_config::AspConfig;
use super::search_pipe_model::Candidate;
use super::search_pipe_native_finder::{NativeFinderSurface, collect_native_finder_candidates};
use super::search_query_wrapper_candidate_scan::{
    QUERY_CANDIDATE_LIMIT, QueryCandidateAppend, append_query_candidates,
    augment_package_path_candidates, query_candidate_priority,
};
use super::search_query_wrapper_model::{QueryWrapperClause, QueryWrapperSurface};

pub(super) struct QueryCandidateCollection {
    pub(super) candidates: Vec<Candidate>,
    pub(super) trace_fields: BTreeMap<String, Value>,
}

pub(super) fn collect_query_candidates(
    surface: QueryWrapperSurface,
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
    clauses: &[QueryWrapperClause],
    terms: &[String],
    config: &AspConfig,
) -> Result<Vec<Candidate>, String> {
    collect_query_candidate_collection(
        surface,
        project_root,
        locator_root,
        scopes,
        clauses,
        terms,
        config,
    )
    .map(|collection| collection.candidates)
}

pub(super) fn collect_query_candidate_collection(
    surface: QueryWrapperSurface,
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
    clauses: &[QueryWrapperClause],
    terms: &[String],
    config: &AspConfig,
) -> Result<QueryCandidateCollection, String> {
    if terms.is_empty() {
        return Err(format!(
            "asp {} -query requires non-empty terms",
            surface.label()
        ));
    }
    let roots = if scopes.is_empty() {
        vec![project_root.to_path_buf()]
    } else {
        scopes
            .iter()
            .map(|scope| absolute_scope(locator_root, scope))
            .collect()
    };
    let display_root = if scopes.len() == 1 {
        let root = roots
            .first()
            .cloned()
            .unwrap_or_else(|| locator_root.to_path_buf());
        if root.is_file() {
            locator_root.to_path_buf()
        } else {
            root
        }
    } else {
        locator_root.to_path_buf()
    };
    let native_surface = match surface {
        QueryWrapperSurface::Fd => NativeFinderSurface::Path,
        QueryWrapperSurface::Rg => NativeFinderSurface::Content,
    };
    let axis_terms = query_axis_terms(clauses);
    if let Some(mut collection) = collect_native_finder_candidates(
        native_surface,
        infer_language_id(project_root),
        project_root,
        &display_root,
        &roots,
        terms,
        config,
    )?
    .filter(|collection| !collection.candidates.is_empty())
    {
        collection
            .candidates
            .sort_by_key(|candidate| query_candidate_priority(&candidate.path, terms, &axis_terms));
        let mut candidates = cohesive_query_candidates(collection.candidates, clauses);
        let package_path_augmented_count =
            augment_package_path_candidates(&display_root, &roots, terms, config, &mut candidates)?;
        candidates
            .sort_by_key(|candidate| query_candidate_priority(&candidate.path, terms, &axis_terms));
        let mut trace_fields = collection.provenance.trace_fields(candidates.len());
        if package_path_augmented_count > 0 {
            trace_fields.insert(
                "packagePathAugmented".to_string(),
                Value::from(package_path_augmented_count),
            );
        }
        return Ok(QueryCandidateCollection {
            candidates,
            trace_fields,
        });
    }
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for root in &roots {
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
            break;
        }
        append_query_candidates(QueryCandidateAppend {
            surface,
            locator_root: &display_root,
            path: root,
            terms,
            axis_terms: &axis_terms,
            config,
            seen: &mut seen,
            candidates: &mut candidates,
        })?;
    }
    candidates
        .sort_by_key(|candidate| query_candidate_priority(&candidate.path, terms, &axis_terms));
    let mut candidates = cohesive_query_candidates(candidates, clauses);
    let package_path_augmented_count =
        augment_package_path_candidates(&display_root, &roots, terms, config, &mut candidates)?;
    candidates
        .sort_by_key(|candidate| query_candidate_priority(&candidate.path, terms, &axis_terms));
    let mut trace_fields = BTreeMap::new();
    if package_path_augmented_count > 0 {
        trace_fields.insert(
            "packagePathAugmented".to_string(),
            Value::from(package_path_augmented_count),
        );
    }
    Ok(QueryCandidateCollection {
        candidates,
        trace_fields,
    })
}

pub(super) fn query_clauses(queries: &[String]) -> Vec<QueryWrapperClause> {
    queries
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let terms = query_terms(raw);
            (!terms.is_empty()).then_some(QueryWrapperClause {
                id: index + 1,
                raw: raw.clone(),
                terms,
                axis_terms: query_axis_terms_for_raw(raw),
            })
        })
        .collect()
}

pub(super) fn unique_clause_terms(clauses: &[QueryWrapperClause]) -> Vec<String> {
    clauses
        .iter()
        .flat_map(|clause| clause.terms.iter())
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == term) {
                terms.push(term.clone());
            }
            terms
        })
}

fn cohesive_query_candidates(
    candidates: Vec<Candidate>,
    clauses: &[QueryWrapperClause],
) -> Vec<Candidate> {
    if candidates.is_empty() || clauses.len() <= 1 {
        return candidates;
    }
    let expected = clauses
        .iter()
        .map(|clause| clause.id)
        .collect::<BTreeSet<_>>();
    let mut package_coverage = BTreeMap::<String, BTreeSet<usize>>::new();
    let mut path_coverage = BTreeMap::<String, BTreeSet<usize>>::new();
    for candidate in &candidates {
        let clause_ids = candidate_clause_ids(candidate, clauses);
        if clause_ids.is_empty() {
            continue;
        }
        package_coverage
            .entry(package_key(&candidate.path))
            .or_default()
            .extend(clause_ids.iter().copied());
        path_coverage
            .entry(candidate.path.clone())
            .or_default()
            .extend(clause_ids);
    }
    let cohesive_packages = package_coverage
        .iter()
        .filter(|(_, coverage)| coverage == &&expected)
        .map(|(package, _)| package.clone())
        .collect::<BTreeSet<_>>();
    if !cohesive_packages.is_empty() {
        return candidates
            .into_iter()
            .filter(|candidate| cohesive_packages.contains(&package_key(&candidate.path)))
            .collect();
    }
    let cohesive_paths = path_coverage
        .iter()
        .filter(|(_, coverage)| coverage == &&expected)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if !cohesive_paths.is_empty() {
        return candidates
            .into_iter()
            .filter(|candidate| cohesive_paths.contains(&candidate.path))
            .collect();
    }
    candidates
}

fn candidate_clause_ids(candidate: &Candidate, clauses: &[QueryWrapperClause]) -> BTreeSet<usize> {
    clauses
        .iter()
        .filter(|clause| {
            clause
                .terms
                .iter()
                .any(|term| candidate_matches_term(candidate, term))
        })
        .map(|clause| clause.id)
        .collect()
}

pub(super) fn candidate_matches_term(candidate: &Candidate, term: &str) -> bool {
    let lower =
        format!("{} {} {}", candidate.path, candidate.symbol, candidate.text).to_ascii_lowercase();
    lower.contains(term)
}

fn query_terms(query: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    query
        .split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

fn query_axis_terms(clauses: &[QueryWrapperClause]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    clauses
        .iter()
        .flat_map(|clause| clause.axis_terms.iter().cloned())
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

fn query_axis_terms_for_raw(raw: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    raw.split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .flat_map(expanded_query_terms)
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

fn expanded_query_terms(raw: &str) -> Vec<String> {
    let normalized = raw.to_ascii_lowercase();
    let normalized_filter = normalized.clone();
    std::iter::once(normalized)
        .chain(
            identifier_components(raw)
                .into_iter()
                .filter(move |component| component.len() >= 2 && component != &normalized_filter),
        )
        .collect()
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

pub(super) fn owner_candidates(candidates: &[Candidate]) -> Vec<String> {
    unique_take(candidates.iter().map(|candidate| candidate.path.clone()), 8)
}

pub(super) fn package_clusters(candidates: &[Candidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .map(|candidate| package_key(&candidate.path)),
        6,
    )
}

pub(super) fn rg_scope_next(candidates: &[Candidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .map(|candidate| package_key(&candidate.path))
            .filter(|package| !package.is_empty()),
        3,
    )
}

pub(super) fn package_key(path: &str) -> String {
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

fn unique_take(values: impl Iterator<Item = String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}

pub(super) fn infer_language_id(root: &Path) -> &'static str {
    if root.join("Cargo.toml").exists() {
        "rust"
    } else if root.join("tsconfig.json").exists() || root.join("package.json").exists() {
        "typescript"
    } else if root.join("pyproject.toml").exists() {
        "python"
    } else if root.join("Project.toml").exists() {
        "julia"
    } else {
        "unknown"
    }
}

pub(super) fn absolute_scope(root: &Path, scope: &Path) -> PathBuf {
    if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    }
}
