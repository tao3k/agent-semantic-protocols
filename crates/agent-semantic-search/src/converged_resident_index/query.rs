// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! 4-axis converged query execution, `rayon` data-parallel candidate verification,
//! topology filtering, and RAM context extraction for [`super::WorkspaceSearchIndex`].

use std::collections::BTreeSet;
use std::time::Instant;

use memchr::memmem::Finder;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use regex::{Regex, RegexBuilder};
use rustc_hash::FxHashMap;

use crate::ConvergedLineMatch;
use crate::file_locator::{FileLocatorMatchKind, FileLocatorQuery};
use crate::trigram_query::compile_regex_trigram_query;

#[cfg(feature = "tantivy-accelerator")]
use crate::tantivy_ram_engine::TantivyRamEngine;

use super::build::ascii_lower_bytes;
use super::index::{
    IndexedFile, WorkspaceContextLine, WorkspaceSearchHit, WorkspaceSearchIndex,
    WorkspaceSearchQuery, WorkspaceSearchReport,
};
use super::symbols::{
    WorkspaceSymbolMatch, nearest_symbols_for_line, run_graph_axis, run_symbol_axis,
};

const MAX_QUERY_PATTERN_BYTES: usize = 1024;
const PARALLEL_VERIFY_THRESHOLD: usize = 64;

pub(super) struct FusedHit {
    pub(super) file_id: u32,
    pub(super) score: u32,
    pub(super) axes: Vec<String>,
    pub(super) line_matches: Vec<ConvergedLineMatch>,
    pub(super) focus_line: Option<usize>,
    pub(super) symbols: Vec<WorkspaceSymbolMatch>,
    pub(super) graph_relations: Vec<String>,
}

impl FusedHit {
    pub(super) fn new(file_id: u32) -> Self {
        Self {
            file_id,
            score: 0,
            axes: Vec::new(),
            line_matches: Vec::new(),
            focus_line: None,
            symbols: Vec::new(),
            graph_relations: Vec::new(),
        }
    }

    pub(super) fn add_axis(&mut self, axis: &str) {
        if !self.axes.iter().any(|existing| existing == axis) {
            self.axes.push(axis.to_owned());
        }
    }

    pub(super) fn add_symbol(&mut self, sym: WorkspaceSymbolMatch) {
        if !self
            .symbols
            .iter()
            .any(|existing| existing.name == sym.name && existing.line_number == sym.line_number)
        {
            self.symbols.push(sym);
        }
    }

    pub(super) fn add_relation(&mut self, relation: String) {
        if !self
            .graph_relations
            .iter()
            .any(|existing| existing == &relation)
        {
            self.graph_relations.push(relation);
        }
    }
}

pub(super) struct CompiledFilters {
    allowed_exts: Option<BTreeSet<&'static str>>,
    prefix_lower: Option<String>,
    ext_filter: Option<String>,
    has_topology: bool,
}

impl CompiledFilters {
    pub(super) fn from_parts(
        languages: &[String],
        documents: &[String],
        path_prefix: Option<&str>,
        extension: Option<&str>,
    ) -> Self {
        let allowed_exts = allowed_extensions(languages, documents);
        let prefix_lower = path_prefix
            .filter(|p| !p.is_empty())
            .map(str::to_ascii_lowercase);
        let ext_filter = extension
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

    fn from_query(query: &WorkspaceSearchQuery<'_>) -> Self {
        Self::from_parts(
            query.languages,
            query.documents,
            query.path_prefix,
            query.extension,
        )
    }

    pub(super) fn matches(&self, file: &IndexedFile) -> bool {
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

pub(super) fn execute_converged_query(
    index: &WorkspaceSearchIndex,
    query: &WorkspaceSearchQuery<'_>,
) -> WorkspaceSearchReport {
    let started = Instant::now();
    let max_hits = query.max_hits.max(1);
    let filters = CompiledFilters::from_query(query);
    let raw_pattern = bound_utf8_prefix(query.pattern.trim(), MAX_QUERY_PATTERN_BYTES);
    let symbol_filter = query.symbol.map(str::trim).filter(|s| !s.is_empty());
    let graph_filter = query.graph_target.map(str::trim).filter(|s| !s.is_empty());

    if raw_pattern.is_empty() && symbol_filter.is_none() && graph_filter.is_none() {
        let hits = list_files_matching_filters(index, &filters, max_hits, query.context_radius);
        let matched = hits.len();
        return WorkspaceSearchReport {
            generation_digest: index.generation_digest.clone(),
            indexed_files: index.live_file_count,
            candidate_files: index.live_file_count,
            matched_files: matched,
            elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
            hits,
        };
    }

    let mut fused: FxHashMap<u32, FusedHit> = FxHashMap::default();
    let mut candidate_count = 0usize;

    let mut structural_scope: Option<rustc_hash::FxHashSet<u32>> = None;
    if let Some(sym) = symbol_filter {
        let mut sym_fused: FxHashMap<u32, FusedHit> = FxHashMap::default();
        candidate_count += run_symbol_axis(index, sym, &filters, &mut sym_fused);
        let ids: rustc_hash::FxHashSet<u32> = sym_fused.keys().copied().collect();
        merge_fused(&mut fused, sym_fused);
        structural_scope = Some(ids);
    }
    if let Some(target) = graph_filter {
        let mut graph_fused: FxHashMap<u32, FusedHit> = FxHashMap::default();
        candidate_count += run_graph_axis(index, target, &filters, &mut graph_fused);
        let ids: rustc_hash::FxHashSet<u32> = graph_fused.keys().copied().collect();
        merge_fused(&mut fused, graph_fused);
        structural_scope = Some(match structural_scope {
            Some(prev) => prev.intersection(&ids).copied().collect(),
            None => ids,
        });
    }

    if !raw_pattern.is_empty() {
        let mut lexical_fused: FxHashMap<u32, FusedHit> = FxHashMap::default();
        let regex_expr = extract_regex_expression(raw_pattern, index.config.enable_rg_regex);
        if let Some(expr) = regex_expr {
            candidate_count +=
                run_resident_rg_regex(index, expr, &filters, max_hits, &mut lexical_fused);
        } else {
            candidate_count +=
                run_resident_rg_literal(index, raw_pattern, &filters, max_hits, &mut lexical_fused);

            if index.config.enable_file_locator {
                run_file_locator_and_fuzzy_paths(
                    index,
                    raw_pattern,
                    &filters,
                    max_hits,
                    &mut lexical_fused,
                );
            }

            #[cfg(feature = "tantivy-accelerator")]
            if let Some(tantivy) = index.tantivy.as_ref() {
                run_tantivy_bm25_and_fuzzy(
                    index,
                    tantivy,
                    raw_pattern,
                    &filters,
                    max_hits,
                    &mut lexical_fused,
                );
            }
        }
        if let Some(scope) = structural_scope.as_ref() {
            lexical_fused.retain(|fid, _| scope.contains(fid));
            fused.retain(|fid, _| lexical_fused.contains_key(fid));
            merge_fused(&mut fused, lexical_fused);
        } else {
            fused = lexical_fused;
        }
    } else if let Some(scope) = structural_scope.as_ref() {
        fused.retain(|fid, _| scope.contains(fid));
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
        indexed_files: index.live_file_count,
        candidate_files: candidate_count.max(matched_files),
        matched_files,
        elapsed_micros: u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        hits,
    }
}

fn merge_fused(dest: &mut FxHashMap<u32, FusedHit>, src: FxHashMap<u32, FusedHit>) {
    for (file_id, incoming) in src {
        match dest.get_mut(&file_id) {
            Some(existing) => {
                existing.score = existing.score.saturating_add(incoming.score);
                for axis in incoming.axes {
                    existing.add_axis(&axis);
                }
                for sym in incoming.symbols {
                    existing.add_symbol(sym);
                }
                for rel in incoming.graph_relations {
                    existing.add_relation(rel);
                }
                if existing.line_matches.is_empty() && !incoming.line_matches.is_empty() {
                    existing.line_matches = incoming.line_matches;
                }
                if existing.focus_line.is_none() {
                    existing.focus_line = incoming.focus_line;
                }
            }
            None => {
                dest.insert(file_id, incoming);
            }
        }
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

fn verify_regex_candidate(
    file_id: u32,
    file: &IndexedFile,
    regex: &Regex,
    max_matches: usize,
    max_chars: usize,
) -> Option<(u32, u32, Vec<ConvergedLineMatch>, Option<usize>)> {
    let line_matches = collect_regex_line_matches(file, regex, max_matches, max_chars);
    let path_hit = regex.is_match(&file.relative_path);
    if line_matches.is_empty() && !path_hit {
        return None;
    }
    let mut score = 85u32 + (line_matches.len() as u32) * 10;
    if path_hit {
        score = score.saturating_add(60);
    }
    let focus_line = line_matches.first().map(|m| m.line_number);
    Some((file_id, score, line_matches, focus_line))
}

fn run_resident_rg_regex(
    index: &WorkspaceSearchIndex,
    expr: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut FxHashMap<u32, FusedHit>,
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

    let plan = compile_regex_trigram_query(expr, smart_case_insensitive);
    let candidates = index.trigrams.evaluate_query(&plan, &index.tombstones);
    let count = candidates.len();
    let max_matches = index.config.max_line_matches_per_hit.max(1);
    let max_chars = index.config.max_line_preview_chars.max(16);

    let verified: Vec<(u32, u32, Vec<ConvergedLineMatch>, Option<usize>)> = if candidates.len()
        > PARALLEL_VERIFY_THRESHOLD
    {
        candidates
            .par_iter()
            .filter_map(|&file_id| {
                if index.is_tombstoned(file_id) {
                    return None;
                }
                let file = index.files.get(file_id as usize)?;
                if !filters.matches(file) {
                    return None;
                }
                verify_regex_candidate(file_id, file, &regex, max_matches, max_chars)
            })
            .collect()
    } else {
        let mut out = Vec::new();
        for &file_id in &candidates {
            if index.is_tombstoned(file_id) {
                continue;
            }
            let Some(file) = index.files.get(file_id as usize) else {
                continue;
            };
            if !filters.matches(file) {
                continue;
            }
            if let Some(hit) = verify_regex_candidate(file_id, file, &regex, max_matches, max_chars)
            {
                out.push(hit);
                if out.len() >= max_hits * 3 {
                    break;
                }
            }
        }
        out
    };

    for (file_id, score, line_matches, focus_line) in verified {
        let entry = fused
            .entry(file_id)
            .or_insert_with(|| FusedHit::new(file_id));
        entry.score = entry.score.saturating_add(score);
        entry.add_axis("rg-regex");
        if entry.line_matches.is_empty() {
            entry.line_matches = line_matches;
            entry.focus_line = focus_line;
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

struct LiteralVerifyOutcome {
    file_id: u32,
    score: u32,
    path_match: bool,
    line_matches: Vec<ConvergedLineMatch>,
    focus_line: Option<usize>,
}

fn verify_literal_candidate(
    file_id: u32,
    file: &IndexedFile,
    finder: &Finder<'_>,
    exact_finder: Option<&Finder<'_>>,
    max_matches: usize,
    max_chars: usize,
) -> Option<LiteralVerifyOutcome> {
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
            && let Some(exact) = exact_finder
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
        return None;
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
    Some(LiteralVerifyOutcome {
        file_id,
        score,
        path_match,
        line_matches,
        focus_line,
    })
}

fn run_resident_rg_literal(
    index: &WorkspaceSearchIndex,
    pattern: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut FxHashMap<u32, FusedHit>,
) -> usize {
    let needle_lower = ascii_lower_bytes(pattern.as_bytes());
    let candidates = index
        .trigrams
        .query_literal(&needle_lower, &index.tombstones);
    let candidate_count = candidates.len();
    let finder = Finder::new(&needle_lower);
    let exact_finder =
        (pattern.as_bytes() != needle_lower.as_slice()).then(|| Finder::new(pattern));
    let max_matches = index.config.max_line_matches_per_hit.max(1);
    let max_chars = index.config.max_line_preview_chars.max(16);

    let verified: Vec<LiteralVerifyOutcome> =
        if candidates.len() > PARALLEL_VERIFY_THRESHOLD && needle_lower.len() >= 3 {
            candidates
                .par_iter()
                .filter_map(|&file_id| {
                    if index.is_tombstoned(file_id) {
                        return None;
                    }
                    let file = index.files.get(file_id as usize)?;
                    if !filters.matches(file) {
                        return None;
                    }
                    verify_literal_candidate(
                        file_id,
                        file,
                        &finder,
                        exact_finder.as_ref(),
                        max_matches,
                        max_chars,
                    )
                })
                .collect()
        } else {
            let mut out = Vec::new();
            for &file_id in &candidates {
                if index.is_tombstoned(file_id) {
                    continue;
                }
                let Some(file) = index.files.get(file_id as usize) else {
                    continue;
                };
                if !filters.matches(file) {
                    continue;
                }
                if let Some(outcome) = verify_literal_candidate(
                    file_id,
                    file,
                    &finder,
                    exact_finder.as_ref(),
                    max_matches,
                    max_chars,
                ) {
                    out.push(outcome);
                    if needle_lower.len() < 3 && out.len() >= max_hits * 4 {
                        break;
                    }
                }
            }
            out
        };

    for outcome in verified {
        let entry = fused
            .entry(outcome.file_id)
            .or_insert_with(|| FusedHit::new(outcome.file_id));
        entry.score = entry.score.saturating_add(outcome.score);
        if outcome.path_match {
            entry.add_axis("path");
        }
        if !outcome.line_matches.is_empty() {
            entry.add_axis("rg");
            entry.line_matches = outcome.line_matches;
            entry.focus_line = outcome.focus_line;
        }
    }
    candidate_count
}

fn run_file_locator_and_fuzzy_paths(
    index: &WorkspaceSearchIndex,
    pattern: &str,
    filters: &CompiledFilters,
    max_hits: usize,
    fused: &mut FxHashMap<u32, FusedHit>,
) {
    if let Some(locator) = index.file_locator.as_ref() {
        let query = FileLocatorQuery::new(pattern).with_limit(max_hits);
        for cand in locator.locate(&query) {
            let Some(&file_id) = index.path_to_id.get(&cand.workspace_relative_path) else {
                continue;
            };
            if index.is_tombstoned(file_id) {
                continue;
            }
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
            let entry = fused
                .entry(file_id)
                .or_insert_with(|| FusedHit::new(file_id));
            entry.score = entry.score.saturating_add(bonus);
            entry.add_axis(axis);
        }
    }

    let clean_needle = ascii_lower_bytes(pattern.as_bytes());
    if clean_needle.len() >= 3 && !clean_needle.contains(&b' ') && fused.len() < max_hits {
        let mut subseq_hits: Vec<(u32, u32)> = Vec::new();
        for (file_id, file) in index.live_files() {
            if fused.contains_key(&file_id) || !filters.matches(file) {
                continue;
            }
            if let Some(sub_score) = subsequence_fuzzy_score(&file.path_lower, &clean_needle) {
                subseq_hits.push((file_id, sub_score));
            }
        }
        subseq_hits.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.1));
        for (file_id, sub_score) in subseq_hits.into_iter().take(max_hits / 2) {
            let entry = fused
                .entry(file_id)
                .or_insert_with(|| FusedHit::new(file_id));
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
    fused: &mut FxHashMap<u32, FusedHit>,
) {
    let tantivy_hits = tantivy.search(pattern, max_hits * 2, index.config.fuzzy_distance);
    let max_chars = index.config.max_line_preview_chars.max(16);
    let query_terms: Vec<Vec<u8>> = pattern
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|tok| tok.len() >= 2)
        .map(|tok| ascii_lower_bytes(tok.as_bytes()))
        .collect();

    for th in tantivy_hits {
        if index.is_tombstoned(th.file_id) {
            continue;
        }
        let Some(file) = index.files.get(th.file_id as usize) else {
            continue;
        };
        if !filters.matches(file) {
            continue;
        }
        let bm25_bonus = ((th.bm25_score * 14.0).round() as u32).clamp(25, 160);
        let axis = if th.is_fuzzy { "fuzzy" } else { "bm25" };
        let entry = fused
            .entry(th.file_id)
            .or_insert_with(|| FusedHit::new(th.file_id));
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
    if index.is_tombstoned(entry.file_id) {
        return None;
    }
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
    let symbols = if entry.symbols.is_empty() {
        nearest_symbols_for_line(file, first_line.unwrap_or(1))
    } else {
        entry.symbols
    };

    Some(WorkspaceSearchHit {
        path: file.relative_path.clone(),
        language: file.language.map(str::to_owned),
        score,
        matched_axes: entry.axes,
        first_line,
        first_line_text,
        line_matches: entry.line_matches,
        context_lines,
        symbols,
        graph_relations: entry.graph_relations,
    })
}

fn list_files_matching_filters(
    index: &WorkspaceSearchIndex,
    filters: &CompiledFilters,
    max_hits: usize,
    context_radius: usize,
) -> Vec<WorkspaceSearchHit> {
    let mut live: Vec<&IndexedFile> = index
        .live_files()
        .map(|(_, f)| f)
        .filter(|f| filters.matches(f))
        .collect();
    live.sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let mut hits = Vec::with_capacity(max_hits.min(live.len()));
    let max_preview = index.config.max_line_preview_chars.max(16);
    for file in live.into_iter().take(max_hits) {
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
            symbols: nearest_symbols_for_line(file, 1),
            graph_relations: Vec::new(),
        });
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

pub(super) fn extract_line_slice(file: &IndexedFile, line_num: usize) -> &str {
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

pub(super) fn byte_offset_to_line(line_offsets: &[u32], byte_offset: usize) -> usize {
    let target = u32::try_from(byte_offset).unwrap_or(u32::MAX);
    line_offsets
        .partition_point(|&start| start <= target)
        .max(1)
}

pub(super) fn truncate_line(line: &str, max_chars: usize) -> String {
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
