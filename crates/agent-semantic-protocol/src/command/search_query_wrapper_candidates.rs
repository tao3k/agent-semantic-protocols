//! Candidate collection and finder previews for query wrappers.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::search_config::AspConfig;
use super::search_pipe_model::Candidate;
use super::search_pipe_native_finder::{NativeFinderSurface, collect_native_finder_candidates};
use super::search_pipe_owner_roles::{has_strong_secondary_owner_intent, secondary_like_owner};
use super::search_query_wrapper_model::{FdQueryPreview, QueryWrapperClause, QueryWrapperSurface};

const QUERY_CANDIDATE_LIMIT: usize = 256;
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "jl", "ss", "ssi", "scm", "sld",
];
const SUPPORTED_CONFIG_FILENAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "pnpm-workspace.yaml",
    "pyproject.toml",
    "Project.toml",
    "gerbil.pkg",
    "build.ss",
];

pub(super) struct QueryCandidateCollection {
    pub(super) candidates: Vec<Candidate>,
    pub(super) trace_fields: BTreeMap<String, Value>,
}

pub(super) fn fd_query_preview(
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
    query: &str,
) -> Option<FdQueryPreview> {
    let config = AspConfig::load(locator_root, project_root);
    let queries = vec![query.to_string()];
    let clauses = query_clauses(&queries);
    let terms = unique_clause_terms(&clauses);
    let preview_scopes = scopes
        .iter()
        .map(|scope| {
            if scope.is_absolute() {
                scope.clone()
            } else {
                project_root.join(scope)
            }
        })
        .collect::<Vec<_>>();
    let candidates = collect_query_candidates(
        QueryWrapperSurface::Fd,
        project_root,
        project_root,
        &preview_scopes,
        &clauses,
        &terms,
        &config,
    )
    .ok()?;
    let preview = FdQueryPreview {
        owner_candidates: owner_candidates(&candidates).into_iter().take(4).collect(),
        package_clusters: package_clusters(&candidates).into_iter().take(1).collect(),
        rg_scope_next: rg_scope_next(&candidates).into_iter().take(1).collect(),
    };
    (!preview.is_empty()).then_some(preview)
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
        roots
            .first()
            .cloned()
            .unwrap_or_else(|| locator_root.to_path_buf())
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

struct QueryCandidateAppend<'a> {
    surface: QueryWrapperSurface,
    locator_root: &'a Path,
    path: &'a Path,
    terms: &'a [String],
    axis_terms: &'a [String],
    config: &'a AspConfig,
    seen: &'a mut HashSet<String>,
    candidates: &'a mut Vec<Candidate>,
}

fn append_query_candidates(input: QueryCandidateAppend<'_>) -> Result<(), String> {
    let QueryCandidateAppend {
        surface,
        locator_root,
        path,
        terms,
        axis_terms,
        config,
        seen,
        candidates,
    } = input;
    if candidates.len() >= QUERY_CANDIDATE_LIMIT || !path.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect query wrapper path {}: {error}",
            path.display()
        )
    })?;
    if metadata.is_file() {
        append_file_query_candidates(surface, locator_root, path, terms, seen, candidates);
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
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
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
                seen,
                candidates,
            })?;
        } else if file_type.is_file() {
            append_file_query_candidates(surface, locator_root, &path, terms, seen, candidates);
        }
    }
    Ok(())
}

fn augment_package_path_candidates(
    locator_root: &Path,
    roots: &[PathBuf],
    terms: &[String],
    config: &AspConfig,
    candidates: &mut Vec<Candidate>,
) -> Result<usize, String> {
    let package_terms = terms
        .iter()
        .filter(|term| term.contains('_'))
        .cloned()
        .collect::<Vec<_>>();
    if package_terms.is_empty() {
        return Ok(0);
    }
    let mut package_candidates = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        append_query_candidates(QueryCandidateAppend {
            surface: QueryWrapperSurface::Fd,
            locator_root,
            path: root,
            terms: &package_terms,
            axis_terms: &package_terms,
            config,
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
            candidates.push(Candidate {
                source: "package-path-query".to_string(),
                confidence: "package-path".to_string(),
                ..candidate
            });
            added += 1;
        }
    }
    Ok(added)
}

fn append_file_query_candidates(
    surface: QueryWrapperSurface,
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    if !supported_query_file(path) {
        return;
    }
    match surface {
        QueryWrapperSurface::Fd => {
            append_path_candidate(locator_root, path, terms, seen, candidates)
        }
        QueryWrapperSurface::Rg => {
            append_content_candidates(locator_root, path, terms, seen, candidates)
        }
    }
}

fn supported_query_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SUPPORTED_EXTENSIONS.contains(&extension))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| SUPPORTED_CONFIG_FILENAMES.contains(&name))
}

fn append_path_candidate(
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
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
    candidates.push(Candidate {
        path: display.clone(),
        line: 1,
        end_line: 1,
        symbol: term.clone(),
        text: display,
        source: "fd-query".to_string(),
        confidence: "path".to_string(),
    });
}

fn append_content_candidates(
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
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
        candidates.push(Candidate {
            path: display,
            line: line_number,
            end_line: line_number,
            symbol: term.clone(),
            text: line.to_string(),
            source: "rg-query".to_string(),
            confidence: "content".to_string(),
        });
    }
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

fn path_priority(
    path: &Path,
    terms: &[String],
    axis_terms: &[String],
) -> (u8, Reverse<usize>, u8, u8, String) {
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
        secondary_priority,
        Reverse(axis_coverage),
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
                    .trim_end_matches(".tsx")
                    .trim_end_matches(".ts")
                    .trim_end_matches(".jsx")
                    .trim_end_matches(".js")
                    .trim_end_matches(".rs")
                    .trim_end_matches(".py")
                    .trim_end_matches(".jl")
                    .trim_end_matches(".ss")
                    .trim_end_matches(".ssi")
                    .trim_end_matches(".scm")
                    .trim_end_matches(".sld")
                    == term
        })
        .unwrap_or(false)
}

fn query_candidate_priority(
    path: &str,
    terms: &[String],
    axis_terms: &[String],
) -> (u8, Reverse<usize>, u8, u8, u8, String) {
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
        secondary_priority,
        Reverse(axis_coverage),
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

fn should_skip_dir(path: &Path, config: &AspConfig) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.')
        && !config
            .search
            .include_hidden_dirs
            .iter()
            .any(|dir| dir == name)
    {
        return true;
    }
    config.search.ignore_dirs.iter().any(|dir| dir == name)
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
