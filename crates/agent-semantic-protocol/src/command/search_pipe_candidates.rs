//! Candidate collection for ASP-owned cheap search frontiers.

use std::collections::HashSet;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use agent_semantic_provider_transport::byte_text;
use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use ignore::{DirEntry, WalkBuilder};

use super::search_config::AspConfig;
use super::search_language_files::{LanguageFileSpec, language_file_spec};
use super::search_pipe_model::Candidate;
use super::search_pipe_native_finder::{NativeFinderSurface, collect_native_finder_candidates};

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
    if let Some(collection) = collect_native_finder_candidates(
        native_surface_for_pipe_terms(&terms),
        language_id,
        project_root,
        locator_root,
        &roots,
        &terms,
        config,
        &[],
    )?
    .filter(|collection| !collection.candidates.is_empty())
    {
        return Ok(collection.candidates);
    }
    let file_spec = language_file_spec(language_id);
    let term_matcher = CandidateTermMatcher::new(&terms)?;
    let search_roots = roots
        .iter()
        .map(|root| sorted_search_root_files(root, config, file_spec, &terms))
        .collect::<Result<Vec<_>, _>>()?;
    collect_candidates_from_search_roots(
        locator_root,
        file_spec,
        &terms,
        &term_matcher,
        &search_roots,
    )
}

fn native_surface_for_pipe_terms(terms: &[String]) -> NativeFinderSurface {
    if matches!(terms, [term] if search_term_looks_like_path(term)) {
        NativeFinderSurface::Path
    } else {
        NativeFinderSurface::Both
    }
}

fn search_term_looks_like_path(term: &str) -> bool {
    term.contains('/')
        || term.contains('\\')
        || term.contains('.')
        || path_basename_matches(term, term)
}

fn collect_candidates_from_search_roots(
    locator_root: &Path,
    file_spec: LanguageFileSpec,
    terms: &[String],
    term_matcher: &CandidateTermMatcher,
    search_roots: &[Vec<PathBuf>],
) -> Result<Vec<Candidate>, String> {
    let mut candidates = Vec::new();
    let mut remaining = PIPE_CANDIDATE_LINE_LIMIT;
    let per_term_limit = per_term_candidate_limit(terms.len());
    let mut term_counts = vec![0usize; terms.len()];
    let mut seen = HashSet::new();
    let mut collector = CandidateCollector {
        locator_root,
        file_spec,
        terms,
        term_matcher,
        per_term_limit,
        term_counts: &mut term_counts,
        candidates: &mut candidates,
        remaining: &mut remaining,
        seen: &mut seen,
    };
    for paths in search_roots {
        if collector.is_done() {
            break;
        }
        collector.append_path_candidates(paths);
    }
    for paths in search_roots {
        if collector.is_done() {
            break;
        }
        collector.append_candidates(paths)?;
    }
    Ok(candidates)
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

struct CandidateCollector<'a> {
    locator_root: &'a Path,
    file_spec: LanguageFileSpec,
    terms: &'a [String],
    term_matcher: &'a CandidateTermMatcher,
    per_term_limit: usize,
    term_counts: &'a mut [usize],
    candidates: &'a mut Vec<Candidate>,
    remaining: &'a mut usize,
    seen: &'a mut HashSet<String>,
}

impl CandidateCollector<'_> {
    fn is_done(&self) -> bool {
        *self.remaining == 0
    }

    fn append_path_candidates(&mut self, paths: &[PathBuf]) {
        for path in paths {
            if self.is_done() {
                break;
            }
            self.append_path_candidate(path);
        }
    }

    fn append_candidates(&mut self, paths: &[PathBuf]) -> Result<(), String> {
        for path in paths {
            if self.is_done() {
                break;
            }
            self.append_file_candidates(path)?;
        }
        Ok(())
    }

    fn append_file_candidates(&mut self, path: &Path) -> Result<(), String> {
        if !self.file_spec.matches(path) {
            return Ok(());
        }
        self.append_path_candidate(path);
        let Ok(bytes) = fs::read(path) else {
            return Ok(());
        };
        for (index, line) in file_candidate_lines(&bytes).enumerate() {
            if self.is_done() {
                break;
            }
            let Some((candidate, term_index)) = self.line_candidate(path, line, index + 1) else {
                continue;
            };
            if self.push_candidate(candidate) {
                self.term_counts[term_index] += 1;
                *self.remaining -= 1;
            }
        }
        Ok(())
    }

    fn append_path_candidate(&mut self, path: &Path) {
        if self.is_done() {
            return;
        }
        if !self.file_spec.matches(path) {
            return;
        }
        let display = display_path(self.locator_root, path);
        let Some(term_index) = self.term_matcher.find_available_str(
            &display,
            self.terms,
            self.per_term_limit,
            self.term_counts,
        ) else {
            return;
        };
        let candidate = Candidate {
            path: display.clone(),
            line: 1,
            end_line: 1,
            symbol: self.terms[term_index].clone(),
            text: display,
            source: "finder-path".to_string(),
            confidence: "path-exact".to_string(),
        };
        if self.push_candidate(candidate) {
            self.term_counts[term_index] += 1;
            *self.remaining -= 1;
        }
    }

    fn push_candidate(&mut self, candidate: Candidate) -> bool {
        let key = format!(
            "{}:{}:{}:{}",
            candidate.path, candidate.line, candidate.symbol, candidate.source
        );
        if !self.seen.insert(key) {
            return false;
        }
        self.candidates.push(candidate);
        true
    }

    fn line_candidate(
        &self,
        path: &Path,
        line: &[u8],
        line_number: usize,
    ) -> Option<(Candidate, usize)> {
        let term_index = self.term_matcher.find_available_bytes(
            line,
            self.terms,
            self.per_term_limit,
            self.term_counts,
        )?;
        Some((
            Candidate {
                path: display_path(self.locator_root, path),
                line: line_number,
                end_line: line_number,
                symbol: self.terms[term_index].clone(),
                text: byte_text::lossy_string(line),
                source: "finder".to_string(),
                confidence: "heuristic".to_string(),
            },
            term_index,
        ))
    }
}

enum CandidateTermMatcher {
    Ascii(AhoCorasick),
    UnicodeFallback,
}

impl CandidateTermMatcher {
    fn new(terms: &[String]) -> Result<Self, String> {
        if !terms.iter().all(|term| term.is_ascii()) {
            return Ok(Self::UnicodeFallback);
        }
        AhoCorasickBuilder::new()
            .ascii_case_insensitive(true)
            .build(terms)
            .map(Self::Ascii)
            .map_err(|error| format!("failed to compile search pipe term matcher: {error}"))
    }

    fn find_available_str(
        &self,
        haystack: &str,
        terms: &[String],
        per_term_limit: usize,
        term_counts: &[usize],
    ) -> Option<usize> {
        self.find_available_bytes(haystack.as_bytes(), terms, per_term_limit, term_counts)
    }

    fn find_available_bytes(
        &self,
        haystack: &[u8],
        terms: &[String],
        per_term_limit: usize,
        term_counts: &[usize],
    ) -> Option<usize> {
        match self {
            Self::Ascii(matcher) => matcher
                .find_overlapping_iter(haystack)
                .map(|mat| mat.pattern().as_usize())
                .filter(|index| term_available(*index, per_term_limit, term_counts))
                .min(),
            Self::UnicodeFallback => {
                let lower = byte_text::lowercase_lossy_string(haystack);
                terms
                    .iter()
                    .enumerate()
                    .find(|(index, term)| {
                        term_available(*index, per_term_limit, term_counts)
                            && lower.contains(term.as_str())
                    })
                    .map(|(index, _)| index)
            }
        }
    }
}

fn term_available(index: usize, per_term_limit: usize, term_counts: &[usize]) -> bool {
    term_counts.get(index).copied().unwrap_or(0) < per_term_limit
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
            name.trim_end_matches(".tsx")
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

fn sorted_search_root_files(
    root: &Path,
    config: &AspConfig,
    file_spec: LanguageFileSpec,
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
    file_spec: LanguageFileSpec,
    terms: &[String],
) -> Result<Vec<PathBuf>, String> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.filter_entry(search_entry_filter(config, file_spec));
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

fn file_candidate_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    byte_text::split_lf_lines(bytes)
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
