//! Dynamic search candidate projection.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_provider_transport::byte_text;
use ignore::{DirEntry, WalkBuilder};

use crate::{LexicalOverlayDocument, search_lexical_overlay_candidates};

/// Candidate projected from a high-churn dynamic search overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicSearchCandidate {
    /// Owner path relative to the caller's locator root when possible.
    pub path: String,
    /// Display start line. Dynamic overlay candidates do not make line ranges
    /// executable identity.
    pub line: usize,
    /// Display end line.
    pub end_line: usize,
    /// Symbol or query term that led to the candidate.
    pub symbol: String,
    /// Candidate text used by downstream renderers.
    pub text: String,
    /// Candidate source label.
    pub source: String,
    /// Candidate confidence label.
    pub confidence: String,
}

/// Candidate projected from a pipe ingest stream before protocol rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestSearchCandidate {
    /// Owner path relative to the caller's locator root when possible.
    pub path: String,
    /// Display start line. Ingest line ranges remain display metadata.
    pub line: usize,
    /// Display end line.
    pub end_line: usize,
    /// Symbol or text token derived from the ingest record.
    pub symbol: String,
    /// Candidate text used by downstream renderers.
    pub text: String,
    /// Candidate source label.
    pub source: String,
    /// Candidate confidence label.
    pub confidence: String,
}

/// Request for projecting lexical overlay candidates from selected roots.
pub struct DynamicSearchCandidateRequest<'a> {
    /// Root used for display path normalization.
    pub locator_root: &'a Path,
    /// Query terms normalized by the command/parser layer.
    pub terms: &'a [String],
    /// Search roots whose paths were selected by the caller.
    pub search_roots: &'a [Vec<PathBuf>],
    /// Maximum candidates returned.
    pub limit: usize,
}

/// Request for dynamic overlay candidate collection from roots.
pub struct DynamicSearchRootCandidateRequest<'a> {
    /// Root used to resolve relative owner inputs.
    pub project_root: &'a Path,
    /// Root used for display path normalization.
    pub locator_root: &'a Path,
    /// Query terms normalized by the caller.
    pub terms: &'a [String],
    /// Owner roots selected by the caller. Empty means `project_root`.
    pub owners: &'a [PathBuf],
    /// Ignored directory names from the search config.
    pub ignore_dirs: &'a [String],
    /// Hidden directory names that should still be walked.
    pub include_hidden_dirs: &'a [String],
    /// Language/provider-owned file predicate.
    pub file_matches: &'a dyn Fn(&Path) -> bool,
    /// Maximum candidates returned.
    pub limit: usize,
}

/// Project newline/NUL-delimited pipe ingest records into compact candidates.
#[must_use]
pub fn collect_ingest_search_candidates(
    project_root: &Path,
    locator_root: &Path,
    stdin: &[u8],
    limit: usize,
) -> Vec<IngestSearchCandidate> {
    if limit == 0 {
        return Vec::new();
    }
    byte_text::split_lf_or_nul_records(stdin)
        .filter_map(|line| parse_ingest_candidate_line(project_root, locator_root, line))
        .take(limit)
        .collect()
}

/// Collect dynamic overlay candidates from owner roots.
///
/// This keeps the expensive workspace walk and overlay projection in the
/// search core while letting language providers own file matching.
pub fn collect_dynamic_lexical_overlay_candidates_from_roots(
    request: DynamicSearchRootCandidateRequest<'_>,
) -> Result<Vec<DynamicSearchCandidate>, String> {
    if request.limit == 0 || request.terms.is_empty() {
        return Ok(Vec::new());
    }

    let roots = resolved_owner_roots(request.project_root, request.owners);
    let search_roots = roots
        .iter()
        .map(|root| sorted_search_root_files(root, &request))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(collect_dynamic_lexical_overlay_candidates(
        DynamicSearchCandidateRequest {
            locator_root: request.locator_root,
            terms: request.terms,
            search_roots: &search_roots,
            limit: request.limit,
        },
    ))
}

fn parse_ingest_candidate_line(
    project_root: &Path,
    locator_root: &Path,
    line: &[u8],
) -> Option<IngestSearchCandidate> {
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
    Some(IngestSearchCandidate {
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
) -> Option<IngestSearchCandidate> {
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
    Some(IngestSearchCandidate {
        path: display_path(locator_root, &absolute),
        line: line_number,
        end_line: line_number,
        symbol: symbol_from_bytes(text),
        text: byte_text::lossy_string(text),
        source: "ingest".to_string(),
        confidence: "likely".to_string(),
    })
}

/// Project path and file-content lexical overlay evidence into compact candidates.
#[must_use]
pub fn collect_dynamic_lexical_overlay_candidates(
    request: DynamicSearchCandidateRequest<'_>,
) -> Vec<DynamicSearchCandidate> {
    if request.limit == 0 || request.terms.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let documents = request
        .search_roots
        .iter()
        .flat_map(|paths| paths.iter())
        .filter_map(|path| lexical_overlay_document(request.locator_root, path))
        .collect::<Vec<_>>();

    let mut remaining = request.limit;
    let per_term_limit = per_term_candidate_limit(request.terms.len(), request.limit);
    for paths in request.search_roots {
        if remaining == 0 {
            break;
        }
        append_overlay_path_candidates(
            request.locator_root,
            request.terms,
            per_term_limit,
            paths,
            &mut remaining,
            &mut seen,
            &mut candidates,
        );
    }

    if remaining == 0 || documents.is_empty() {
        return candidates;
    }

    for hit in
        search_lexical_overlay_candidates(request.terms, &documents, per_term_limit, remaining)
    {
        if candidates.len() >= request.limit {
            break;
        }
        let candidate = DynamicSearchCandidate {
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

fn resolved_owner_roots(project_root: &Path, owners: &[PathBuf]) -> Vec<PathBuf> {
    if owners.is_empty() {
        return vec![project_root.to_path_buf()];
    }
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
}

fn sorted_search_root_files(
    root: &Path,
    request: &DynamicSearchRootCandidateRequest<'_>,
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
    sorted_search_files(root, request)
}

fn sorted_search_files(
    root: &Path,
    request: &DynamicSearchRootCandidateRequest<'_>,
) -> Result<Vec<PathBuf>, String> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.filter_entry(search_entry_filter(
        request.ignore_dirs.to_vec(),
        request.include_hidden_dirs.to_vec(),
    ));
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
        if file_type.is_file() && (request.file_matches)(entry.path()) {
            paths.push(entry.into_path());
        }
    }
    paths.sort_by_key(|path| path_search_priority(path, request.terms));
    Ok(paths)
}

fn search_entry_filter(
    ignore_dirs: Vec<String>,
    include_hidden_dirs: Vec<String>,
) -> impl Fn(&DirEntry) -> bool + Send + Sync + 'static {
    move |entry| {
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            return true;
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

fn append_overlay_path_candidates(
    locator_root: &Path,
    terms: &[String],
    per_term_limit: usize,
    paths: &[PathBuf],
    remaining: &mut usize,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<DynamicSearchCandidate>,
) {
    let mut term_counts = vec![0usize; terms.len()];
    for path in paths {
        if *remaining == 0 {
            break;
        }
        let display = display_path(locator_root, path);
        let lower = display.to_ascii_lowercase();
        for (index, term) in terms.iter().enumerate() {
            if term_counts[index] >= per_term_limit || !lower.contains(term) {
                continue;
            }
            let candidate = DynamicSearchCandidate {
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
    let source_text = String::from_utf8_lossy(&bytes).into_owned();
    Some(
        LexicalOverlayDocument::new(display.clone(), display.clone(), symbol_from_text(&display))
            .kind("owner")
            .source_hash("workspace-dirty")
            .search_text(source_text),
    )
}

fn push_candidate(
    candidate: DynamicSearchCandidate,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<DynamicSearchCandidate>,
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

fn per_term_candidate_limit(term_count: usize, total_limit: usize) -> usize {
    if term_count == 0 {
        return total_limit;
    }
    (total_limit / term_count).clamp(16, 64).min(total_limit)
}

fn symbol_from_text(text: &str) -> String {
    text.split(|character: char| {
        !(character == '_' || character == '-' || character.is_ascii_alphanumeric())
    })
    .find(|part| !part.is_empty())
    .unwrap_or("match")
    .to_lowercase()
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn symbol_from_bytes(bytes: &[u8]) -> String {
    symbol_from_text(&byte_text::lossy_string(bytes))
}

fn parse_usize_ascii(bytes: &[u8]) -> Option<usize> {
    std::str::from_utf8(bytes).ok()?.parse::<usize>().ok()
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
