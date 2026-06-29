//! Candidate collection for ASP-owned cheap search frontiers.

use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use agent_semantic_client::{LexicalOverlayDocument, search_lexical_overlay_candidates};
use agent_semantic_provider_transport::byte_text;
use ignore::{DirEntry, WalkBuilder};

use super::search_config::AspConfig;
use super::search_language_files::{LanguageFileSpec, language_file_spec};
use super::search_pipe_model::Candidate;
use super::search_pipe_native_finder::{
    NativeFinderCollectionRequest, NativeFinderSurface, collect_native_finder_candidates,
};

const PIPE_CANDIDATE_LINE_LIMIT: usize = 256;

pub(super) fn read_piped_stdin() -> Result<Vec<u8>, String> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(Vec::new());
    }
    let mut bytes = Vec::new();
    stdin
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read search ingest stdin: {error}"))?;
    Ok(bytes)
}

pub(super) fn parse_ingest_candidates(
    project_root: &Path,
    locator_root: &Path,
    stdin: &[u8],
) -> Vec<Candidate> {
    ingest_candidate_lines(stdin)
        .filter_map(|line| parse_ingest_candidate_line(project_root, locator_root, line))
        .take(PIPE_CANDIDATE_LINE_LIMIT)
        .collect()
}

pub(super) fn collect_candidates(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    query: &str,
    owners: &[PathBuf],
    config: &AspConfig,
) -> Result<Vec<Candidate>, String> {
    let terms = query_terms(query);
    if terms.is_empty() {
        return Err("search pipe requires a non-empty query".to_string());
    }
    let roots = if owners.is_empty() {
        vec![project_root.to_path_buf()]
    } else {
        owners
            .iter()
            .map(|owner| {
                if owner.is_absolute() {
                    owner.clone()
                } else {
                    project_root.join(owner)
                }
            })
            .collect()
    };
    let file_spec = language_file_spec(language_id);
    if terms.iter().all(|term| search_term_looks_like_path(term)) {
        let native_candidates = collect_native_finder_candidates(NativeFinderCollectionRequest {
            surface: native_surface_for_pipe_terms(&terms),
            language_id,
            file_spec_override: None,
            accept_all_files: false,
            project_root,
            locator_root,
            roots: &roots,
            terms: &terms,
            config,
            native_args: &[],
        })?
        .map(|collection| collection.candidates)
        .unwrap_or_default();
        return Ok(native_candidates);
    }
    let search_roots = roots
        .iter()
        .map(|root| sorted_search_root_files(root, config, &file_spec, &terms))
        .collect::<Result<Vec<_>, _>>()?;
    let supplemental_candidates =
        collect_candidates_from_search_roots(locator_root, &file_spec, &terms, &search_roots);
    Ok(supplemental_candidates)
}

fn native_surface_for_pipe_terms(terms: &[String]) -> NativeFinderSurface {
    if matches!(terms, [term] if search_term_looks_like_path(term)) {
        NativeFinderSurface::Path
    } else {
        NativeFinderSurface::Both
    }
}

fn search_term_looks_like_path(term: &str) -> bool {
    term.contains('/') || term.contains('\\') || term.contains('.')
}

fn collect_candidates_from_search_roots(
    locator_root: &Path,
    file_spec: &LanguageFileSpec,
    terms: &[String],
    search_roots: &[Vec<PathBuf>],
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let per_term_limit = per_term_candidate_limit(terms.len());
    let mut seen = HashSet::new();
    let documents = search_roots
        .iter()
        .flat_map(|paths| paths.iter())
        .filter(|path| file_spec.matches(path))
        .filter_map(|path| lexical_overlay_document(locator_root, path))
        .collect::<Vec<_>>();
    if documents.is_empty() {
        return candidates;
    }
    let mut remaining = PIPE_CANDIDATE_LINE_LIMIT;
    for paths in search_roots {
        if remaining == 0 {
            break;
        }
        append_overlay_path_candidates(
            locator_root,
            file_spec,
            terms,
            per_term_limit,
            paths,
            &mut remaining,
            &mut seen,
            &mut candidates,
        );
    }
    for hit in search_lexical_overlay_candidates(terms, &documents, per_term_limit, remaining) {
        if candidates.len() >= PIPE_CANDIDATE_LINE_LIMIT {
            break;
        }
        let candidate = Candidate {
            path: hit.owner_path().to_string(),
            line: 1,
            end_line: 1,
            symbol: hit.symbol().to_string(),
            text: hit.text().to_string(),
            source: "overlay".to_string(),
            confidence: "lexical-overlay".to_string(),
        };
        push_candidate(candidate, &mut seen, &mut candidates);
    }
    candidates
}

fn parse_ingest_candidate_line(
    project_root: &Path,
    locator_root: &Path,
    line: &[u8],
) -> Option<Candidate> {
    if line.is_empty() {
        return None;
    }
    if let Some(candidate) = parse_line_candidate(project_root, locator_root, line) {
        return Some(candidate);
    }
    let path = PathBuf::from(byte_text::lossy_string(line));
    let absolute = resolve_candidate_path(project_root, locator_root, path);
    if !absolute.exists() {
        return None;
    }
    let display = display_path(locator_root, &absolute);
    Some(Candidate {
        symbol: symbol_from_text(&display),
        path: display,
        line: 1,
        end_line: 1,
        text: String::new(),
        source: "ingest".to_string(),
        confidence: "likely".to_string(),
    })
}

fn parse_line_candidate(
    project_root: &Path,
    locator_root: &Path,
    line: &[u8],
) -> Option<Candidate> {
    let path_end = byte_text::find_byte(b':', line)?;
    let raw_path = &line[..path_end];
    let rest = &line[path_end + 1..];
    let line_end = byte_text::find_byte(b':', rest)?;
    let line_number = parse_usize_ascii(&rest[..line_end])?;
    let rest = &rest[line_end + 1..];
    let text = if let Some(column_end) = byte_text::find_byte(b':', rest) {
        if parse_usize_ascii(&rest[..column_end]).is_some() {
            &rest[column_end + 1..]
        } else {
            rest
        }
    } else {
        rest
    };
    let path = PathBuf::from(byte_text::lossy_string(raw_path));
    let absolute = resolve_candidate_path(project_root, locator_root, path);
    Some(Candidate {
        path: display_path(locator_root, &absolute),
        line: line_number,
        end_line: line_number,
        symbol: symbol_from_bytes(text),
        text: byte_text::lossy_string(text),
        source: "ingest".to_string(),
        confidence: "likely".to_string(),
    })
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn append_overlay_path_candidates(
    locator_root: &Path,
    file_spec: &LanguageFileSpec,
    terms: &[String],
    per_term_limit: usize,
    paths: &[PathBuf],
    remaining: &mut usize,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    let mut term_counts = vec![0usize; terms.len()];
    for path in paths {
        if *remaining == 0 {
            break;
        }
        if !file_spec.matches(path) {
            continue;
        }
        let display = display_path(locator_root, path);
        let lower = display.to_ascii_lowercase();
        for (index, term) in terms.iter().enumerate() {
            if term_counts[index] >= per_term_limit || !lower.contains(term) {
                continue;
            }
            let candidate = Candidate {
                path: display.clone(),
                line: 1,
                end_line: 1,
                symbol: term.clone(),
                text: display.clone(),
                source: "overlay-path".to_string(),
                confidence: "path-lexical-overlay".to_string(),
            };
            if push_candidate(candidate, seen, candidates) {
                term_counts[index] += 1;
                *remaining -= 1;
            }
            break;
        }
    }
}

fn lexical_overlay_document(locator_root: &Path, path: &Path) -> Option<LexicalOverlayDocument> {
    let display = display_path(locator_root, path);
    let bytes = fs::read(path).ok()?;
    let source_text = byte_text::lossy_string(&bytes);
    Some(
        LexicalOverlayDocument::new(display.clone(), display.clone(), symbol_from_text(&display))
            .kind("owner")
            .source_hash("workspace-dirty")
            .search_text(source_text),
    )
}

fn push_candidate(
    candidate: Candidate,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) -> bool {
    let key = format!(
        "{}:{}:{}:{}",
        candidate.path, candidate.line, candidate.symbol, candidate.source
    );
    if !seen.insert(key) {
        return false;
    }
    candidates.push(candidate);
    true
}

fn path_search_priority(path: &Path, terms: &[String]) -> (u8, u8, String) {
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
    } else if display.contains("/tests/")
        || display.ends_with("/tests")
        || display.contains("/benches/")
        || display.ends_with("/benches")
        || display.contains("/examples/")
        || display.ends_with("/examples")
    {
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
            name == term
                || name
                    .rsplit_once('.')
                    .map(|(stem, _)| stem == term)
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn sorted_search_root_files(
    root: &Path,
    config: &AspConfig,
    file_spec: &LanguageFileSpec,
    terms: &[String],
) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(root).map_err(|error| {
        format!(
            "failed to inspect search pipe root {}: {error}",
            root.display()
        )
    })?;
    if metadata.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    sorted_search_files(root, config, file_spec, terms)
}

fn sorted_search_files(
    root: &Path,
    config: &AspConfig,
    file_spec: &LanguageFileSpec,
    terms: &[String],
) -> Result<Vec<PathBuf>, String> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.filter_entry(search_entry_filter(config, file_spec.clone()));
    let mut paths = Vec::new();
    for result in builder.build() {
        let entry = result.map_err(|error| {
            format!(
                "failed to walk search pipe root {}: {error}",
                root.display()
            )
        })?;
        if entry.depth() == 0 {
            continue;
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() {
            paths.push(entry.into_path());
        }
    }
    paths.sort_by_key(|path| path_search_priority(path, terms));
    Ok(paths)
}

fn search_entry_filter(
    config: &AspConfig,
    file_spec: LanguageFileSpec,
) -> impl Fn(&DirEntry) -> bool + Send + Sync + 'static {
    let ignore_dirs = config.search.ignore_dirs.clone();
    let include_hidden_dirs = config.search.include_hidden_dirs.clone();
    move |entry| {
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            return file_spec.matches(entry.path());
        }
        !should_skip_walk_dir(entry, &ignore_dirs, &include_hidden_dirs)
    }
}

fn should_skip_walk_dir(
    entry: &DirEntry,
    ignore_dirs: &[String],
    include_hidden_dirs: &[String],
) -> bool {
    if entry.depth() == 0
        || !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
    {
        return false;
    }
    should_skip_dir_name(entry.path(), ignore_dirs, include_hidden_dirs)
}

fn should_skip_dir_name(
    path: &Path,
    ignore_dirs: &[String],
    include_hidden_dirs: &[String],
) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') && !include_hidden_dirs.iter().any(|dir| dir == name) {
        return true;
    }
    ignore_dirs.iter().any(|dir| dir == name)
}

fn per_term_candidate_limit(term_count: usize) -> usize {
    if term_count == 0 {
        return PIPE_CANDIDATE_LINE_LIMIT;
    }
    (PIPE_CANDIDATE_LINE_LIMIT / term_count)
        .clamp(16, 64)
        .min(PIPE_CANDIDATE_LINE_LIMIT)
}

fn resolve_candidate_path(project_root: &Path, locator_root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    let locator_relative = locator_root.join(&path);
    if locator_relative.exists() {
        return locator_relative;
    }
    project_root.join(path)
}

fn symbol_from_text(text: &str) -> String {
    text.split(|character: char| {
        !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
    .find(|part| !part.is_empty())
    .unwrap_or("match")
    .to_lowercase()
}

fn symbol_from_bytes(bytes: &[u8]) -> String {
    symbol_from_text(&byte_text::lossy_string(bytes))
}

fn parse_usize_ascii(bytes: &[u8]) -> Option<usize> {
    std::str::from_utf8(bytes).ok()?.parse::<usize>().ok()
}

fn ingest_candidate_lines(stdin: &[u8]) -> impl Iterator<Item = &[u8]> {
    byte_text::split_lf_or_nul_records(stdin)
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
