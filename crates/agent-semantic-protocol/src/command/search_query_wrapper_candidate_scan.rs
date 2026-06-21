//! Filesystem candidate scanning for query-wrapper surfaces.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::search_config::AspConfig;
use super::search_language_files::language_neutral_search_file_spec;
use super::search_pipe_model::Candidate;
use super::search_pipe_owner_roles::{has_strong_secondary_owner_intent, secondary_like_owner};
use super::search_query_wrapper_model::QueryWrapperSurface;

pub(super) const QUERY_CANDIDATE_LIMIT: usize = 256;

pub(super) struct QueryCandidateAppend<'a> {
    pub(super) surface: QueryWrapperSurface,
    pub(super) locator_root: &'a Path,
    pub(super) path: &'a Path,
    pub(super) terms: &'a [String],
    pub(super) axis_terms: &'a [String],
    pub(super) config: &'a AspConfig,
    pub(super) accept_all_files: bool,
    pub(super) seen: &'a mut HashSet<String>,
    pub(super) candidates: &'a mut Vec<Candidate>,
}

pub(super) fn append_query_candidates(input: QueryCandidateAppend<'_>) -> Result<(), String> {
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

pub(super) fn augment_package_path_candidates(
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
            surface: QueryWrapperSurface::Fd,
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
    accept_all_files: bool,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    if !accept_all_files && !supported_query_file(path) {
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

fn candidate_covers_term(candidate: &Candidate, term: &str) -> bool {
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

pub(super) fn query_candidate_priority(
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
