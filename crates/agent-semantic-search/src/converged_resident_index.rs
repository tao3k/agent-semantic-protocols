// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! High-performance sub-millisecond converged 4-axis resident workspace search engine.
//!
//! Owned by ASP (`agent-semantic-search`) and governed declaratively by caller-supplied
//! [`WorkspaceIndexConfig`] (e.g., Marlin's Scheme `HelmIndexPolicy.`):
//! 1. **Built-in Resident `rg` Engine**: 3-byte ASCII trigram posting-list intersection +
//!    SIMD `memchr::memmem` for literal queries, plus full `regex::Regex` matching
//!    (`re:...`, `rg:...`, or regex metacharacters) directly in RAM.
//! 2. **Built-in ASP `FileLocatorIndex` + Fuzzy Subsequence Path Matcher**: instant
//!    exact, basename, stem, segment, glob, trigram `FuzzyFilename`, and ordered
//!    character subsequence path matching.
//! 3. **In-RAM Tantivy BM25 + `FuzzyTermQuery`** (feature `tantivy-accelerator`):
//!    real `tantivy::Index::create_in_ram` BM25 relevance ranking, field expressions,
//!    and Levenshtein edit-distance 1–2 typo-tolerant fuzzy search.
//! 4. **O(1) Topology Filtering**: `@lang:`, `@doc:`, `@prefix:`, `@ext:` scoping.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ignore::WalkBuilder;
use memchr::memmem::Finder;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use regex::{Regex, RegexBuilder};

use crate::file_locator::{FileLocatorIndex, FileLocatorMatchKind, FileLocatorQuery};
use crate::{ConvergedLineMatch, SearchGenerationDigest};

#[cfg(feature = "tantivy-accelerator")]
use crate::tantivy_ram_engine::TantivyRamEngine;

const DEFAULT_MAX_INDEXED_FILE_BYTES: u64 = 2 * 1024 * 1024;
const DEFAULT_MAX_LINE_MATCHES_PER_HIT: usize = 6;
const DEFAULT_MAX_LINE_PREVIEW_CHARS: usize = 220;
const MAX_QUERY_PATTERN_BYTES: usize = 1024;

/// Declarative indexing policy supplied by the host/config plane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceIndexConfig {
    pub skip_dirs: Vec<String>,
    pub max_file_bytes: u64,
    pub max_line_matches_per_hit: usize,
    pub max_line_preview_chars: usize,
    pub fuzzy_distance: u8,
    pub enable_tantivy: bool,
    pub enable_file_locator: bool,
    pub enable_rg_regex: bool,
}

impl Default for WorkspaceIndexConfig {
    fn default() -> Self {
        Self {
            skip_dirs: vec![".git".to_owned()],
            max_file_bytes: DEFAULT_MAX_INDEXED_FILE_BYTES,
            max_line_matches_per_hit: DEFAULT_MAX_LINE_MATCHES_PER_HIT,
            max_line_preview_chars: DEFAULT_MAX_LINE_PREVIEW_CHARS,
            fuzzy_distance: 1,
            enable_tantivy: true,
            enable_file_locator: true,
            enable_rg_regex: true,
        }
    }
}

/// One line of surrounding source context extracted directly from RAM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceContextLine {
    pub line_number: usize,
    pub text: String,
    pub is_match: bool,
}

/// One ranked converged workspace hit produced by [`WorkspaceSearchIndex::query`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchHit {
    pub path: String,
    pub language: Option<String>,
    pub score: u32,
    pub matched_axes: Vec<String>,
    pub first_line: Option<usize>,
    pub first_line_text: String,
    pub line_matches: Vec<ConvergedLineMatch>,
    pub context_lines: Vec<WorkspaceContextLine>,
}

/// Query parameters for sub-millisecond converged resident search execution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceSearchQuery<'a> {
    pub pattern: &'a str,
    pub languages: &'a [String],
    pub documents: &'a [String],
    pub path_prefix: Option<&'a str>,
    pub extension: Option<&'a str>,
    pub max_hits: usize,
    pub context_radius: usize,
}

/// Sub-millisecond converged query report with exact microsecond timing and generation digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSearchReport {
    pub generation_digest: SearchGenerationDigest,
    pub indexed_files: usize,
    pub candidate_files: usize,
    pub matched_files: usize,
    pub elapsed_micros: u64,
    pub hits: Vec<WorkspaceSearchHit>,
}

struct IndexedFile {
    relative_path: String,
    path_lower: Vec<u8>,
    ext: String,
    language: Option<&'static str>,
    body: String,
    body_lower: Vec<u8>,
    line_offsets: Vec<u32>,
}

struct BuiltFileRecord {
    file: IndexedFile,
    hash: [u8; 32],
    trigrams: Vec<u32>,
}

struct FusedHit {
    file_id: u32,
    score: u32,
    axes: Vec<String>,
    line_matches: Vec<ConvergedLineMatch>,
    focus_line: Option<usize>,
}

impl FusedHit {
    fn add_axis(&mut self, axis: &str) {
        if !self.axes.iter().any(|existing| existing == axis) {
            self.axes.push(axis.to_owned());
        }
    }
}

/// Resident, generation-stamped 4-axis search index owned by ASP.
pub struct WorkspaceSearchIndex {
    root: PathBuf,
    config: WorkspaceIndexConfig,
    files: Vec<IndexedFile>,
    path_to_id: HashMap<String, u32>,
    trigrams: HashMap<u32, Vec<u32>>,
    file_locator: Option<FileLocatorIndex>,
    #[cfg(feature = "tantivy-accelerator")]
    tantivy: Option<TantivyRamEngine>,
    generation_digest: SearchGenerationDigest,
    build_micros: u64,
}

impl WorkspaceSearchIndex {
    /// Build a fresh resident index for `root` using default minimal config.
    #[must_use]
    pub fn build(root: &Path) -> Self {
        Self::build_with_config(root, WorkspaceIndexConfig::default())
    }

    /// Build a fresh resident index for `root` governed by caller-supplied `config`.
    #[must_use]
    pub fn build_with_config(root: &Path, config: WorkspaceIndexConfig) -> Self {
        let started = Instant::now();
        let entries = collect_workspace_entries(root, &config);
        let records: Vec<BuiltFileRecord> = entries
            .into_par_iter()
            .filter_map(|(abs_path, rel_path)| build_file_record_from_disk(&abs_path, rel_path))
            .collect();
        assemble_index(root.to_path_buf(), config, records, started)
    }

    /// Build a resident index directly from in-memory `(relative_path, body_bytes)` pairs
    /// (ideal for deterministic property tests, fuzzing, and virtual workspaces).
    #[must_use]
    pub fn build_from_memory<I, P, B>(root: PathBuf, files: I, config: WorkspaceIndexConfig) -> Self
    where
        I: IntoIterator<Item = (P, B)>,
        P: Into<String>,
        B: Into<Vec<u8>>,
    {
        let started = Instant::now();
        let max_bytes = usize::try_from(config.max_file_bytes.max(1)).unwrap_or(usize::MAX);
        let records: Vec<BuiltFileRecord> = files
            .into_iter()
            .filter_map(|(rel, bytes)| {
                let raw = bytes.into();
                if raw.is_empty() || raw.len() > max_bytes {
                    return None;
                }
                build_file_record_from_bytes(rel.into(), raw)
            })
            .collect();
        assemble_index(root, config, records, started)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn build_micros(&self) -> u64 {
        self.build_micros
    }

    #[must_use]
    pub fn generation_digest(&self) -> &SearchGenerationDigest {
        &self.generation_digest
    }

    /// Execute a sub-millisecond converged query across resident `rg`, ASP
    /// `FileLocatorIndex`, fuzzy path matching, and in-RAM Tantivy BM25/Fuzzy.
    #[must_use]
    pub fn query(&self, query: &WorkspaceSearchQuery<'_>) -> WorkspaceSearchReport {
        execute_converged_query(self, query)
    }
}

fn assemble_index(
    root: PathBuf,
    config: WorkspaceIndexConfig,
    mut records: Vec<BuiltFileRecord>,
    started: Instant,
) -> WorkspaceSearchIndex {
    records.sort_unstable_by(|a, b| a.file.relative_path.cmp(&b.file.relative_path));
    records.dedup_by(|a, b| a.file.relative_path == b.file.relative_path);

    let generation_digest = compute_generation_digest(&records);
    let (files, path_to_id, trigrams) = finalize_inverted_index(records);

    let file_locator = config.enable_file_locator.then(|| {
        FileLocatorIndex::build(files.iter().map(|file| PathBuf::from(&file.relative_path)))
    });

    #[cfg(feature = "tantivy-accelerator")]
    let tantivy = if config.enable_tantivy && !files.is_empty() {
        let docs: Vec<(u32, &str, &str)> = files
            .iter()
            .enumerate()
            .filter_map(|(idx, file)| {
                Some((
                    u32::try_from(idx).ok()?,
                    file.relative_path.as_str(),
                    file.body.as_str(),
                ))
            })
            .collect();
        TantivyRamEngine::build(&docs)
    } else {
        None
    };

    let build_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    WorkspaceSearchIndex {
        root,
        config,
        files,
        path_to_id,
        trigrams,
        file_locator,
        #[cfg(feature = "tantivy-accelerator")]
        tantivy,
        generation_digest,
        build_micros,
    }
}

fn collect_workspace_entries(root: &Path, config: &WorkspaceIndexConfig) -> Vec<(PathBuf, String)> {
    let mut entries = Vec::new();
    let skip_dirs = config.skip_dirs.clone();
    let max_file_bytes = config.max_file_bytes.max(1);
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let name = entry.file_name().to_str().unwrap_or("");
                return !skip_dirs.iter().any(|skip| skip == name);
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() == 0 || meta.len() > max_file_bytes {
            continue;
        }
        let path = entry.into_path();
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel_path = rel.to_string_lossy().replace('\\', "/");
        entries.push((path, rel_path));
    }
    entries
}

fn build_file_record_from_disk(abs_path: &Path, relative_path: String) -> Option<BuiltFileRecord> {
    let bytes = std::fs::read(abs_path).ok()?;
    build_file_record_from_bytes(relative_path, bytes)
}

fn build_file_record_from_bytes(relative_path: String, bytes: Vec<u8>) -> Option<BuiltFileRecord> {
    let clean_path = relative_path.replace('\\', "/");
    if clean_path.is_empty()
        || bytes.is_empty()
        || memchr::memchr(0, &bytes[..bytes.len().min(4096)]).is_some()
    {
        return None;
    }
    let body = String::from_utf8(bytes).ok()?;
    let hash = *blake3::hash(body.as_bytes()).as_bytes();
    let ext = Path::new(&clean_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let language = infer_language(&ext);
    let path_lower = ascii_lower_bytes(clean_path.as_bytes());
    let body_lower = ascii_lower_bytes(body.as_bytes());
    let line_offsets = build_line_offsets(body.as_bytes());
    let trigrams = extract_sorted_trigrams(&path_lower, &body_lower);

    Some(BuiltFileRecord {
        file: IndexedFile {
            relative_path: clean_path,
            path_lower,
            ext,
            language,
            body,
            body_lower,
            line_offsets,
        },
        hash,
        trigrams,
    })
}

fn compute_generation_digest(records: &[BuiltFileRecord]) -> SearchGenerationDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.converged-resident-index.v2\n");
    for rec in records {
        hasher.update(rec.file.relative_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(&rec.hash);
        hasher.update(b"\n");
    }
    let digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    SearchGenerationDigest::new(digest).expect("valid blake3-256 generation digest")
}

type FinalizedIndex = (
    Vec<IndexedFile>,
    HashMap<String, u32>,
    HashMap<u32, Vec<u32>>,
);

fn finalize_inverted_index(records: Vec<BuiltFileRecord>) -> FinalizedIndex {
    let mut files = Vec::with_capacity(records.len());
    let mut path_to_id = HashMap::with_capacity(records.len());
    let mut trigrams: HashMap<u32, Vec<u32>> = HashMap::with_capacity(16_384);
    for (idx, rec) in records.into_iter().enumerate() {
        let file_id = u32::try_from(idx).unwrap_or(u32::MAX);
        path_to_id.insert(rec.file.relative_path.clone(), file_id);
        for tri in rec.trigrams {
            trigrams.entry(tri).or_default().push(file_id);
        }
        files.push(rec.file);
    }
    (files, path_to_id, trigrams)
}

struct CompiledFilters {
    allowed_exts: Option<BTreeSet<&'static str>>,
    prefix_lower: Option<String>,
    ext_filter: Option<String>,
    has_topology: bool,
}

impl CompiledFilters {
    fn from_query(query: &WorkspaceSearchQuery<'_>) -> Self {
        let allowed_exts = allowed_extensions(query.languages, query.documents);
        let prefix_lower = query
            .path_prefix
            .filter(|p| !p.is_empty())
            .map(str::to_ascii_lowercase);
        let ext_filter = query
            .extension
            .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
            .filter(|e| !e.is_empty());
        let has_topology = prefix_lower.is_some() || ext_filter.is_some();
        Self {
            allowed_exts,
            prefix_lower,
            ext_filter,
            has_topology,
        }
    }

    fn matches(&self, file: &IndexedFile) -> bool {
        if let Some(prefix) = self.prefix_lower.as_deref()
            && !file.path_lower.starts_with(prefix.as_bytes())
        {
            return false;
        }
        if let Some(ext) = self.ext_filter.as_deref()
            && file.ext != ext
        {
            return false;
        }
        if let Some(allowed) = &self.allowed_exts
            && !allowed.contains(file.ext.as_str())
        {
            return false;
        }
        true
    }
}

fn bound_utf8_prefix(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }
    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

fn execute_converged_query(
    index: &WorkspaceSearchIndex,
    query: &WorkspaceSearchQuery<'_>,
) -> WorkspaceSearchReport {
    let started = Instant::now();
    let max_hits = query.max_hits.max(1);
    let filters = CompiledFilters::from_query(query);
    let raw_pattern = bound_utf8_prefix(query.pattern.trim(), MAX_QUERY_PATTERN_BYTES);

    if raw_pattern.is_empty() {
        let hits = list_files_matching_filters(index, &filters, max_hits, query.context_radius);
        let matched = hits.len();
        return WorkspaceSearchReport {
            generation_digest: index.generation_digest.clone(),
            indexed_files: index.files.len(),
            candidate_files: index.files.len(),
            matched_files: matched,
            elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            hits,
        };
    }

    let mut fused: HashMap<u32, FusedHit> = HashMap::new();
    let mut candidate_count = 0usize;

    let regex_expr = extract_regex_expression(raw_pattern, index.config.enable_rg_regex);
    if let Some(expr) = regex_expr {
        candidate_count += run_resident_rg_regex(index, expr, &filters, max_hits, &mut fused);
    } else {
        candidate_count +=
            run_resident_rg_literal(index, raw_pattern, &filters, max_hits, &mut fused);

        if index.config.enable_file_locator {
            run_file_locator_and_fuzzy_paths(index, raw_pattern, &filters, max_hits, &mut fused);
        }

        #[cfg(feature = "tantivy-accelerator")]
        if let Some(tantivy) = index.tantivy.as_ref() {
            run_tantivy_bm25_and_fuzzy(index, tantivy, raw_pattern, &filters, max_hits, &mut fused);
        }
    }

    let mut hits: Vec<WorkspaceSearchHit> = fused
        .into_values()
        .filter_map(|entry| {
            finalize_fused_hit(index, entry, filters.has_topology, query.context_radius)
        })
        .collect();

    hits.sort_unstable_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    let matched_files = hits.len();
    hits.truncate(max_hits);

    WorkspaceSearchReport {
        generation_digest: index.generation_digest.clone(),
        indexed_files: index.files.len(),
        candidate_files: candidate_count.max(matched_files),
        matched_files,
        elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        hits,
    }
}

fn extract_regex_expression(raw: &str, enable_rg_regex: bool) -> Option<&str> {
    if !enable_rg_regex {
        return None;
    }
    if let Some(rest) = raw.strip_prefix("re:").or_else(|| raw.strip_prefix("rg:")) {
        let trimmed = rest.trim();
        return (!trimmed.is_empty()).then_some(trimmed);
    }
    let has_regex_meta = raw.contains(".*")
        || raw.contains(".+")
        || raw.contains("\\s")
        || raw.contains("\\w")
        || raw.contains("\\d")
        || raw.contains("\\b")
        || raw.starts_with('^')
        || raw.ends_with('$')
        || (raw.contains('|') && !raw.contains(" OR "))
        || (raw.contains('[') && raw.contains(']'))
        || (raw.contains('(') && raw.contains(')'));
    has_regex_meta.then_some(raw)
}

fn longest_ascii_literal_run(expr: &str) -> Option<Vec<u8>> {
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut cur_start = 0usize;
    let mut cur_len = 0usize;
    let bytes = expr.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' {
            if cur_len > best_len {
                best_start = cur_start;
                best_len = cur_len;
            }
            cur_len = 0;
            i += 2;
            continue;
        }
        if b.is_ascii_alphanumeric() || b == b'_' {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
        } else {
            if cur_len > best_len {
                best_start = cur_start;
                best_len = cur_len;
            }
            cur_len = 0;
        }
        i += 1;
    }
    if cur_len > best_len {
        best_start = cur_start;
        best_len = cur_len;
    }
    (best_len >= 3).then(|| ascii_lower_bytes(&bytes[best_start..best_start + best_len]))
}

fn run_resident_rg_regex(
    index: &WorkspaceSearchIndex,
    expr: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut HashMap<u32, FusedHit>,
) -> usize {
    let smart_case_insensitive = !expr.chars().any(char::is_uppercase);
    let Ok(regex) = RegexBuilder::new(expr)
        .case_insensitive(smart_case_insensitive)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(2 * 1024 * 1024)
        .build()
    else {
        return 0;
    };

    let candidates = match longest_ascii_literal_run(expr) {
        Some(literal) if !expr.contains('|') && !expr.contains('?') && !expr.contains('*') => {
            select_candidate_file_ids(index, &literal)
        }
        _ => (0..index.files.len())
            .filter_map(|i| u32::try_from(i).ok())
            .collect(),
    };
    let count = candidates.len();
    let max_matches = index.config.max_line_matches_per_hit.max(1);
    let max_chars = index.config.max_line_preview_chars.max(16);

    for file_id in candidates {
        let Some(file) = index.files.get(file_id as usize) else {
            continue;
        };
        if !filters.matches(file) {
            continue;
        }
        let line_matches = collect_regex_line_matches(file, &regex, max_matches, max_chars);
        let path_hit = regex.is_match(&file.relative_path);
        if line_matches.is_empty() && !path_hit {
            continue;
        }
        let mut score = 85u32 + (line_matches.len() as u32) * 10;
        if path_hit {
            score = score.saturating_add(60);
        }
        let focus_line = line_matches.first().map(|m| m.line_number);
        let entry = fused.entry(file_id).or_insert_with(|| FusedHit {
            file_id,
            score: 0,
            axes: Vec::new(),
            line_matches: Vec::new(),
            focus_line: None,
        });
        entry.score = entry.score.saturating_add(score);
        entry.add_axis("rg-regex");
        if entry.line_matches.is_empty() {
            entry.line_matches = line_matches;
            entry.focus_line = focus_line;
        }
        if fused.len() >= max_hits * 3 {
            break;
        }
    }
    count
}

fn collect_regex_line_matches(
    file: &IndexedFile,
    regex: &Regex,
    max_matches: usize,
    max_chars: usize,
) -> Vec<ConvergedLineMatch> {
    let mut matches = Vec::new();
    let mut last_line = 0usize;
    for m in regex.find_iter(&file.body) {
        let line_num = byte_offset_to_line(&file.line_offsets, m.start());
        if line_num == last_line {
            continue;
        }
        last_line = line_num;
        matches.push(ConvergedLineMatch {
            line_number: line_num,
            line_text: truncate_line(extract_line_slice(file, line_num), max_chars),
        });
        if matches.len() >= max_matches {
            break;
        }
    }
    matches
}

fn run_resident_rg_literal(
    index: &WorkspaceSearchIndex,
    pattern: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut HashMap<u32, FusedHit>,
) -> usize {
    let needle_lower = ascii_lower_bytes(pattern.as_bytes());
    let candidates = select_candidate_file_ids(index, &needle_lower);
    let candidate_count = candidates.len();
    let finder = Finder::new(&needle_lower);
    let exact_finder =
        (pattern.as_bytes() != needle_lower.as_slice()).then(|| Finder::new(pattern));
    let max_matches = index.config.max_line_matches_per_hit.max(1);
    let max_chars = index.config.max_line_preview_chars.max(16);

    for file_id in candidates {
        let Some(file) = index.files.get(file_id as usize) else {
            continue;
        };
        if !filters.matches(file) {
            continue;
        }
        let path_match = finder.find(&file.path_lower).is_some();
        let mut line_matches = Vec::new();
        let mut last_line_num = 0usize;
        let mut exact_case = false;

        for byte_offset in finder.find_iter(&file.body_lower) {
            let line_num = byte_offset_to_line(&file.line_offsets, byte_offset);
            if line_num == last_line_num {
                continue;
            }
            last_line_num = line_num;
            let line_text = extract_line_slice(file, line_num);
            if !exact_case
                && let Some(exact) = exact_finder.as_ref()
                && exact.find(line_text.as_bytes()).is_some()
            {
                exact_case = true;
            }
            line_matches.push(ConvergedLineMatch {
                line_number: line_num,
                line_text: truncate_line(line_text, max_chars),
            });
            if line_matches.len() >= max_matches {
                break;
            }
        }

        if !path_match && line_matches.is_empty() {
            continue;
        }

        let mut score = 50u32;
        if path_match {
            score = score.saturating_add(95);
            if let Some(basename) = file.relative_path.rsplit('/').next()
                && finder
                    .find(&ascii_lower_bytes(basename.as_bytes()))
                    .is_some()
            {
                score = score.saturating_add(50);
            }
        }
        if !line_matches.is_empty() {
            score = score.saturating_add(70 + (line_matches.len() as u32) * 8);
        }
        if exact_case {
            score = score.saturating_add(25);
        }

        let focus_line = line_matches.first().map(|m| m.line_number);
        let entry = fused.entry(file_id).or_insert_with(|| FusedHit {
            file_id,
            score: 0,
            axes: Vec::new(),
            line_matches: Vec::new(),
            focus_line: None,
        });
        entry.score = entry.score.saturating_add(score);
        if path_match {
            entry.add_axis("path");
        }
        if !line_matches.is_empty() {
            entry.add_axis("rg");
            entry.line_matches = line_matches;
            entry.focus_line = focus_line;
        }

        if needle_lower.len() < 3 && fused.len() >= max_hits * 4 {
            break;
        }
    }
    candidate_count
}

fn run_file_locator_and_fuzzy_paths(
    index: &WorkspaceSearchIndex,
    pattern: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut HashMap<u32, FusedHit>,
) {
    if let Some(locator) = index.file_locator.as_ref() {
        let query = FileLocatorQuery::new(pattern).with_limit(max_hits);
        for cand in locator.locate(&query) {
            let Some(&file_id) = index.path_to_id.get(&cand.workspace_relative_path) else {
                continue;
            };
            let Some(file) = index.files.get(file_id as usize) else {
                continue;
            };
            if !filters.matches(file) {
                continue;
            }
            let axis = match cand.match_kind {
                FileLocatorMatchKind::FuzzyFilename => "fuzzy-path",
                _ => "path",
            };
            let bonus = u32::from(cand.score / 5).max(30);
            let entry = fused.entry(file_id).or_insert_with(|| FusedHit {
                file_id,
                score: 0,
                axes: Vec::new(),
                line_matches: Vec::new(),
                focus_line: None,
            });
            entry.score = entry.score.saturating_add(bonus);
            entry.add_axis(axis);
        }
    }

    let clean_needle = ascii_lower_bytes(pattern.as_bytes());
    if clean_needle.len() >= 3 && !clean_needle.contains(&b' ') && fused.len() < max_hits {
        let mut subseq_hits: Vec<(u32, u32)> = Vec::new();
        for (idx, file) in index.files.iter().enumerate() {
            let Ok(file_id) = u32::try_from(idx) else {
                continue;
            };
            if fused.contains_key(&file_id) || !filters.matches(file) {
                continue;
            }
            if let Some(sub_score) = subsequence_fuzzy_score(&file.path_lower, &clean_needle) {
                subseq_hits.push((file_id, sub_score));
            }
        }
        subseq_hits.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.1));
        for (file_id, sub_score) in subseq_hits.into_iter().take(max_hits / 2) {
            let entry = fused.entry(file_id).or_insert_with(|| FusedHit {
                file_id,
                score: 0,
                axes: Vec::new(),
                line_matches: Vec::new(),
                focus_line: None,
            });
            entry.score = entry.score.saturating_add(sub_score);
            entry.add_axis("fuzzy-path");
        }
    }
}

fn subsequence_fuzzy_score(haystack: &[u8], needle: &[u8]) -> Option<u32> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    let mut n_idx = 0usize;
    let mut score = 30u32;
    let mut consecutive = 0u32;
    for (h_idx, &b) in haystack.iter().enumerate() {
        if b == needle[n_idx] {
            let boundary =
                h_idx == 0 || matches!(haystack[h_idx - 1], b'/' | b'-' | b'_' | b'.' | b' ');
            if boundary {
                score = score.saturating_add(16);
            }
            if consecutive > 0 {
                score = score.saturating_add(8);
            }
            consecutive += 1;
            n_idx += 1;
            if n_idx == needle.len() {
                let len_penalty = u32::try_from(haystack.len() / 6).unwrap_or(20).min(20);
                return Some(score.saturating_sub(len_penalty).max(20));
            }
        } else {
            consecutive = 0;
        }
    }
    None
}

#[cfg(feature = "tantivy-accelerator")]
fn run_tantivy_bm25_and_fuzzy(
    index: &WorkspaceSearchIndex,
    tantivy: &TantivyRamEngine,
    pattern: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut HashMap<u32, FusedHit>,
) {
    let tantivy_hits = tantivy.search(pattern, max_hits * 2, index.config.fuzzy_distance);
    let max_chars = index.config.max_line_preview_chars.max(16);
    let query_terms: Vec<Vec<u8>> = pattern
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|tok| tok.len() >= 2)
        .map(|tok| ascii_lower_bytes(tok.as_bytes()))
        .collect();

    for th in tantivy_hits {
        let Some(file) = index.files.get(th.file_id as usize) else {
            continue;
        };
        if !filters.matches(file) {
            continue;
        }
        let bm25_bonus = ((th.bm25_score * 14.0).round() as u32).clamp(25, 160);
        let axis = if th.is_fuzzy { "fuzzy" } else { "bm25" };
        let entry = fused.entry(th.file_id).or_insert_with(|| FusedHit {
            file_id: th.file_id,
            score: 0,
            axes: Vec::new(),
            line_matches: Vec::new(),
            focus_line: None,
        });
        entry.score = entry.score.saturating_add(if th.is_fuzzy {
            bm25_bonus / 2 + 20
        } else {
            bm25_bonus
        });
        entry.add_axis(axis);

        if entry.line_matches.is_empty()
            && let Some(line_match) = best_term_line_in_file(file, &query_terms, max_chars)
        {
            entry.focus_line = Some(line_match.line_number);
            entry.line_matches.push(line_match);
        }
    }
}

#[cfg(feature = "tantivy-accelerator")]
fn best_term_line_in_file(
    file: &IndexedFile,
    terms: &[Vec<u8>],
    max_chars: usize,
) -> Option<ConvergedLineMatch> {
    for term in terms {
        if let Some(pos) = Finder::new(term).find(&file.body_lower) {
            let line_num = byte_offset_to_line(&file.line_offsets, pos);
            return Some(ConvergedLineMatch {
                line_number: line_num,
                line_text: truncate_line(extract_line_slice(file, line_num), max_chars),
            });
        }
    }
    for term in terms {
        if term.len() >= 3
            && let Some(pos) = Finder::new(&term[..3]).find(&file.body_lower)
        {
            let line_num = byte_offset_to_line(&file.line_offsets, pos);
            return Some(ConvergedLineMatch {
                line_number: line_num,
                line_text: truncate_line(extract_line_slice(file, line_num), max_chars),
            });
        }
    }
    None
}

fn finalize_fused_hit(
    index: &WorkspaceSearchIndex,
    mut entry: FusedHit,
    has_topology: bool,
    context_radius: usize,
) -> Option<WorkspaceSearchHit> {
    let file = index.files.get(entry.file_id as usize)?;
    if has_topology {
        entry.add_axis("topology");
        entry.score = entry.score.saturating_add(10);
    }
    let path_penalty = u32::try_from(file.relative_path.len() / 8)
        .unwrap_or(20)
        .min(20);
    let score = entry.score.saturating_sub(path_penalty);
    let max_preview_chars = index.config.max_line_preview_chars.max(16);
    let first_line = entry.focus_line.or(Some(1));
    let first_line_text = entry
        .line_matches
        .first()
        .map(|m| m.line_text.clone())
        .unwrap_or_else(|| truncate_line(extract_line_slice(file, 1), max_preview_chars));
    let context_lines = extract_context_lines(
        file,
        first_line.unwrap_or(1),
        context_radius,
        max_preview_chars,
    );

    Some(WorkspaceSearchHit {
        path: file.relative_path.clone(),
        language: file.language.map(str::to_owned),
        score,
        matched_axes: entry.axes,
        first_line,
        first_line_text,
        line_matches: entry.line_matches,
        context_lines,
    })
}

fn select_candidate_file_ids(index: &WorkspaceSearchIndex, needle_lower: &[u8]) -> Vec<u32> {
    if needle_lower.len() < 3 {
        return (0..index.files.len())
            .filter_map(|i| u32::try_from(i).ok())
            .collect();
    }
    let mut query_trigrams = Vec::with_capacity(needle_lower.len().saturating_sub(2));
    for win in needle_lower.windows(3) {
        query_trigrams.push(pack_trigram(win[0], win[1], win[2]));
    }
    query_trigrams.sort_unstable();
    query_trigrams.dedup();

    let mut posting_lists: Vec<&[u32]> = Vec::with_capacity(query_trigrams.len());
    for tri in query_trigrams {
        let Some(list) = index.trigrams.get(&tri) else {
            return Vec::new();
        };
        posting_lists.push(list.as_slice());
    }
    posting_lists.sort_unstable_by_key(|list| list.len());

    let mut acc = posting_lists[0].to_vec();
    for list in &posting_lists[1..] {
        acc = intersect_sorted(&acc, list);
        if acc.is_empty() {
            break;
        }
    }
    acc
}

fn intersect_sorted(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    if a.len().saturating_mul(8) < b.len() {
        for &val in a {
            if b.binary_search(&val).is_ok() {
                out.push(val);
            }
        }
        return out;
    }
    let (mut i, mut j) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }
    out
}

fn list_files_matching_filters(
    index: &WorkspaceSearchIndex,
    filters: &CompiledFilters,
    max_hits: usize,
    context_radius: usize,
) -> Vec<WorkspaceSearchHit> {
    let mut hits = Vec::new();
    let max_preview = index.config.max_line_preview_chars.max(16);
    for file in &index.files {
        if !filters.matches(file) {
            continue;
        }
        let context_lines = extract_context_lines(file, 1, context_radius, max_preview);
        let first_line_text = context_lines
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();
        let axes = if filters.has_topology {
            vec!["topology".to_owned()]
        } else {
            vec!["index".to_owned()]
        };
        hits.push(WorkspaceSearchHit {
            path: file.relative_path.clone(),
            language: file.language.map(str::to_owned),
            score: 50,
            matched_axes: axes,
            first_line: Some(1),
            first_line_text,
            line_matches: Vec::new(),
            context_lines,
        });
        if hits.len() >= max_hits {
            break;
        }
    }
    hits
}

fn extract_context_lines(
    file: &IndexedFile,
    focus_line: usize,
    radius: usize,
    max_preview_chars: usize,
) -> Vec<WorkspaceContextLine> {
    let total_lines = file.line_offsets.len();
    if total_lines == 0 {
        return Vec::new();
    }
    let focus = focus_line.clamp(1, total_lines);
    let start = focus.saturating_sub(radius).max(1);
    let end = focus.saturating_add(radius).min(total_lines);
    let mut out = Vec::with_capacity(end - start + 1);
    for line_num in start..=end {
        out.push(WorkspaceContextLine {
            line_number: line_num,
            text: truncate_line(extract_line_slice(file, line_num), max_preview_chars),
            is_match: line_num == focus,
        });
    }
    out
}

fn extract_line_slice(file: &IndexedFile, line_num: usize) -> &str {
    let Some(&start_u32) = file.line_offsets.get(line_num.saturating_sub(1)) else {
        return "";
    };
    let start = start_u32 as usize;
    let end = file
        .line_offsets
        .get(line_num)
        .map_or(file.body.len(), |&next| next as usize);
    file.body
        .get(start..end)
        .unwrap_or("")
        .trim_end_matches(['\r', '\n'])
}

fn byte_offset_to_line(line_offsets: &[u32], byte_offset: usize) -> usize {
    let target = u32::try_from(byte_offset).unwrap_or(u32::MAX);
    line_offsets
        .partition_point(|&start| start <= target)
        .max(1)
}

fn build_line_offsets(bytes: &[u8]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(64);
    offsets.push(0);
    for pos in memchr::memchr_iter(b'\n', bytes) {
        if let Some(next) = pos.checked_add(1)
            && next < bytes.len()
            && let Ok(off) = u32::try_from(next)
        {
            offsets.push(off);
        }
    }
    offsets
}

fn ascii_lower_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        out.push(b.to_ascii_lowercase());
    }
    out
}

fn extract_sorted_trigrams(path_lower: &[u8], body_lower: &[u8]) -> Vec<u32> {
    let mut tris = Vec::with_capacity(
        path_lower
            .len()
            .saturating_add(body_lower.len().min(65_536)),
    );
    for win in path_lower.windows(3) {
        tris.push(pack_trigram(win[0], win[1], win[2]));
    }
    for win in body_lower.windows(3) {
        tris.push(pack_trigram(win[0], win[1], win[2]));
    }
    tris.sort_unstable();
    tris.dedup();
    tris
}

#[inline]
fn pack_trigram(b0: u8, b1: u8, b2: u8) -> u32 {
    (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2)
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in line.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        if ch == '\t' {
            out.push_str("  ");
        } else if !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

fn allowed_extensions(
    languages: &[String],
    documents: &[String],
) -> Option<BTreeSet<&'static str>> {
    let mut exts = BTreeSet::new();
    for lang in languages {
        for ext in extensions_for_language(lang.as_str()) {
            exts.insert(*ext);
        }
    }
    for doc in documents {
        for ext in extensions_for_document(doc.as_str()) {
            exts.insert(*ext);
        }
    }
    if exts.is_empty() { None } else { Some(exts) }
}

fn extensions_for_language(language: &str) -> &'static [&'static str] {
    match language {
        "rust" | "rs" => &["rs"],
        "typescript" | "ts" => &["ts", "tsx", "mts", "cts"],
        "javascript" | "js" => &["js", "jsx", "mjs", "cjs"],
        "python" | "py" => &["py", "pyi"],
        "julia" | "jl" => &["jl"],
        "gerbil-scheme" | "scheme" | "ss" => &["ss", "scm"],
        "sysml" => &["sysml"],
        "quint" | "qnt" => &["qnt"],
        _ => &[],
    }
}

fn extensions_for_document(document: &str) -> &'static [&'static str] {
    match document {
        "org" => &["org"],
        "markdown" | "md" => &["md", "markdown"],
        "toml" => &["toml"],
        "json" => &["json"],
        _ => &[],
    }
}

fn infer_language(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "ts" | "tsx" | "mts" | "cts" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" | "pyi" => Some("python"),
        "jl" => Some("julia"),
        "ss" | "scm" => Some("scheme"),
        "org" => Some("org"),
        "md" | "markdown" => Some("markdown"),
        "toml" => Some("toml"),
        "sysml" => Some("sysml"),
        "qnt" => Some("quint"),
        _ => None,
    }
}
