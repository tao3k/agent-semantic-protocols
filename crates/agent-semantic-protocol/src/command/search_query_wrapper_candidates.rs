//! Candidate collection and finder previews for query wrappers.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::search_config::AspConfig;
use super::search_pipe_model::Candidate;
use super::search_query_wrapper_model::{FdQueryPreview, QueryWrapperClause, QueryWrapperSurface};

const QUERY_CANDIDATE_LIMIT: usize = 256;
const SUPPORTED_EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "py", "jl"];

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
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
            break;
        }
        append_query_candidates(
            surface,
            &display_root,
            &root,
            terms,
            config,
            &mut seen,
            &mut candidates,
        )?;
    }
    candidates.sort_by_key(|candidate| query_candidate_priority(&candidate.path, terms));
    Ok(cohesive_query_candidates(candidates, clauses))
}

fn append_query_candidates(
    surface: QueryWrapperSurface,
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    config: &AspConfig,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) -> Result<(), String> {
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
    entries.sort_by_key(|entry| path_priority(&entry.path(), terms));
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
            append_query_candidates(
                surface,
                locator_root,
                &path,
                terms,
                config,
                seen,
                candidates,
            )?;
        } else if file_type.is_file() {
            append_file_query_candidates(surface, locator_root, &path, terms, seen, candidates);
        }
    }
    Ok(())
}

fn append_file_query_candidates(
    surface: QueryWrapperSurface,
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return;
    };
    if !SUPPORTED_EXTENSIONS.contains(&extension) {
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

fn path_priority(path: &Path, terms: &[String]) -> (u8, u8, String) {
    let display = path.to_string_lossy().replace('\\', "/");
    let lower = display.to_ascii_lowercase();
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
    (query_priority, layout_priority, display)
}

fn path_basename_matches(lower_path: &str, term: &str) -> bool {
    lower_path
        .rsplit('/')
        .next()
        .map(|name| {
            name.trim_end_matches(".tsx")
                .trim_end_matches(".ts")
                .trim_end_matches(".jsx")
                .trim_end_matches(".js")
                .trim_end_matches(".rs")
                .trim_end_matches(".py")
                .trim_end_matches(".jl")
                == term
        })
        .unwrap_or(false)
}

fn query_candidate_priority(path: &str, terms: &[String]) -> (u8, u8, String) {
    let lower = path.to_ascii_lowercase();
    let query_priority = if terms.iter().any(|term| path_basename_matches(&lower, term)) {
        0
    } else if terms.iter().any(|term| lower.contains(term)) {
        1
    } else {
        2
    };
    let owner_priority = if lower.contains("/internal/") { 1 } else { 0 };
    (query_priority, owner_priority, lower)
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
