// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-Rust in-process execution engine for admitted ASP Search Playbooks.
//!
//! Evaluates topology membership, lexical `rg` patterns, and Tantivy query
//! scoring directly inside the host process (zero subprocess spawn) and
//! produces a deterministic BLAKE3 generation-bound execution report ready for
//! CAS persistence through [`crate::SearchEngineStoragePort`].

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use agent_semantic_search_playbook::{
    ProgressiveSearchPlaybookRequest, TopologyOwnerMembershipBlock, analyze_native_rg_argv,
    analyze_tantivy_query,
};
use globset::{Glob, GlobMatcher};
use ignore::WalkBuilder;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::{
    SearchGenerationDigest, SearchStoragePortError, SourceIndexRankCandidate,
    SourceIndexRankRequest, TrigramQuery, WorkspaceSearchIndex, compile_regex_trigram_query,
    rank_source_index_report,
};

const MAX_LINE_MATCHES_PER_FILE: usize = 8;
const MAX_LINE_PREVIEW_BYTES: usize = 240;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// One exact line match produced by lexical evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConvergedLineMatch {
    pub line_number: usize,
    pub line_text: String,
}

/// One ranked file hit returned by [`execute_admitted_playbook`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConvergedSearchHit {
    pub path: String,
    pub language: Option<String>,
    pub score: u32,
    pub matched_axes: Vec<String>,
    pub line_matches: Vec<ConvergedLineMatch>,
}

/// Deterministic, generation-bound report returned by pure-Rust playbook execution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConvergedSearchExecutionReport {
    pub generation_digest: SearchGenerationDigest,
    pub execution_mode: String,
    pub scanned_files: usize,
    pub matched_files: usize,
    pub admitted_axes: Vec<String>,
    pub hits: Vec<ConvergedSearchHit>,
}

/// Error returned when pure-Rust playbook execution cannot run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConvergedSearchExecutionError {
    WorkspaceUnavailable(String),
    InvalidPattern(String),
    InvalidGlob(String),
    Storage(SearchStoragePortError),
}

impl fmt::Display for ConvergedSearchExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceUnavailable(msg) => write!(formatter, "workspace unavailable: {msg}"),
            Self::InvalidPattern(msg) => write!(formatter, "invalid search pattern: {msg}"),
            Self::InvalidGlob(msg) => write!(formatter, "invalid topology glob: {msg}"),
            Self::Storage(err) => write!(formatter, "search storage error: {err}"),
        }
    }
}

impl std::error::Error for ConvergedSearchExecutionError {}

impl From<SearchStoragePortError> for ConvergedSearchExecutionError {
    fn from(value: SearchStoragePortError) -> Self {
        Self::Storage(value)
    }
}

struct CompiledTopologyFilter {
    exact_path: Option<String>,
    path_prefix: Option<String>,
    extension: Option<String>,
    glob: Option<GlobMatcher>,
}

struct CompiledRgClause {
    matchers: Vec<Regex>,
    trigram_plan: TrigramQuery,
}

struct CompiledTantivyClause {
    terms: Vec<String>,
}

struct ScannedFile {
    relative_path: String,
    language: Option<String>,
    content_hash: String,
    body: String,
}

/// Execute an admitted search playbook purely in-process over `workspace_root`.
pub fn execute_admitted_playbook(
    workspace_root: &Path,
    request: &ProgressiveSearchPlaybookRequest,
) -> Result<ConvergedSearchExecutionReport, ConvergedSearchExecutionError> {
    if !workspace_root.is_dir() {
        return Err(ConvergedSearchExecutionError::WorkspaceUnavailable(
            workspace_root.display().to_string(),
        ));
    }
    let allowed_extensions = allowed_extensions_from_request(request);
    let topology_filters = compile_topology_filters(&request.topology)?;
    let rg_clauses = compile_rg_clauses(&request.rg)?;
    let tantivy_clauses = compile_tantivy_clauses(&request.tantivy);

    let scanned = scan_workspace_files(workspace_root, &allowed_extensions, &topology_filters);
    let generation_digest = compute_workspace_generation_digest(&scanned)?;
    let admitted_axes = collect_admitted_axes(request);
    let query_hint = build_rank_query_hint(&rg_clauses, &tantivy_clauses, request);

    let mut candidate_hits = Vec::new();
    for file in &scanned {
        if let Some(hit) = evaluate_file(
            file,
            &rg_clauses,
            &tantivy_clauses,
            !topology_filters.is_empty(),
        ) {
            candidate_hits.push(hit);
        }
    }

    let ranked_hits = rank_converged_hits(candidate_hits, &query_hint);
    Ok(ConvergedSearchExecutionReport {
        generation_digest,
        execution_mode: "pure-rust-in-process".to_owned(),
        scanned_files: scanned.len(),
        matched_files: ranked_hits.len(),
        admitted_axes,
        hits: ranked_hits,
    })
}

/// Execute an admitted multi-axis Search Playbook directly against a resident
/// [`WorkspaceSearchIndex`] (zero disk I/O, full HIR trigram query pruning).
pub fn execute_admitted_playbook_with_index(
    index: &WorkspaceSearchIndex,
    request: &ProgressiveSearchPlaybookRequest,
) -> Result<ConvergedSearchExecutionReport, ConvergedSearchExecutionError> {
    let allowed_extensions = allowed_extensions_from_request(request);
    let topology_filters = compile_topology_filters(&request.topology)?;
    let rg_clauses = compile_rg_clauses(&request.rg)?;
    let tantivy_clauses = compile_tantivy_clauses(&request.tantivy);
    let admitted_axes = collect_admitted_axes(request);
    let query_hint = build_rank_query_hint(&rg_clauses, &tantivy_clauses, request);

    let plans: Vec<TrigramQuery> = rg_clauses.iter().map(|c| c.trigram_plan.clone()).collect();
    let candidate_ids = index.evaluate_trigram_plans(&plans);

    let mut scanned_files = 0usize;
    let mut candidate_hits = Vec::new();
    for file_id in candidate_ids {
        let Some((rel_path, ext, lang, body)) = index.live_file_view(file_id) else {
            continue;
        };
        if !matches_topology_filters(rel_path, &topology_filters) {
            continue;
        }
        if let Some(allowed) = &allowed_extensions
            && !allowed.contains(ext)
        {
            continue;
        }
        scanned_files += 1;
        let view = ScannedFile {
            relative_path: rel_path.to_owned(),
            language: lang
                .map(|l| {
                    if l == "scheme" {
                        "gerbil-scheme".to_owned()
                    } else {
                        l.to_owned()
                    }
                })
                .or_else(|| infer_language_from_extension(ext)),
            content_hash: String::new(),
            body: body.to_owned(),
        };
        if let Some(hit) = evaluate_file(
            &view,
            &rg_clauses,
            &tantivy_clauses,
            !topology_filters.is_empty(),
        ) {
            candidate_hits.push(hit);
        }
    }

    let ranked_hits = rank_converged_hits(candidate_hits, &query_hint);
    Ok(ConvergedSearchExecutionReport {
        generation_digest: index.generation_digest().clone(),
        execution_mode: "pure-rust-resident-index".to_owned(),
        scanned_files,
        matched_files: ranked_hits.len(),
        admitted_axes,
        hits: ranked_hits,
    })
}

fn allowed_extensions_from_request(
    request: &ProgressiveSearchPlaybookRequest,
) -> Option<BTreeSet<&'static str>> {
    let mut exts = BTreeSet::new();
    if let Some(langs) = &request.language {
        for lang in langs.split('|').map(str::trim).filter(|s| !s.is_empty()) {
            for ext in extensions_for_language(lang) {
                exts.insert(*ext);
            }
        }
    }
    if let Some(docs) = &request.documents {
        for doc in docs.split('|').map(str::trim).filter(|s| !s.is_empty()) {
            for ext in extensions_for_document(doc) {
                exts.insert(*ext);
            }
        }
    }
    if exts.is_empty() { None } else { Some(exts) }
}

fn extensions_for_language(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["rs"],
        "typescript" => &["ts", "tsx", "mts", "cts"],
        "javascript" => &["js", "jsx", "mjs", "cjs"],
        "python" => &["py", "pyi"],
        "julia" => &["jl"],
        "gerbil-scheme" | "scheme" => &["ss", "scm"],
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

fn infer_language_from_extension(ext: &str) -> Option<String> {
    let lang = match ext {
        "rs" => "rust",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "jl" => "julia",
        "ss" | "scm" => "gerbil-scheme",
        "org" => "org",
        "md" | "markdown" => "markdown",
        _ => return None,
    };
    Some(lang.to_owned())
}

fn compile_topology_filters(
    blocks: &[TopologyOwnerMembershipBlock],
) -> Result<Vec<CompiledTopologyFilter>, ConvergedSearchExecutionError> {
    blocks
        .iter()
        .map(|block| {
            let glob = match &block.path_glob {
                Some(pattern) => Some(
                    Glob::new(pattern)
                        .map_err(|err| ConvergedSearchExecutionError::InvalidGlob(err.to_string()))?
                        .compile_matcher(),
                ),
                None => None,
            };
            Ok(CompiledTopologyFilter {
                exact_path: block.exact_path.clone(),
                path_prefix: block.path_prefix.clone(),
                extension: block.extension.clone(),
                glob,
            })
        })
        .collect()
}

fn matches_topology_filters(rel_path: &str, filters: &[CompiledTopologyFilter]) -> bool {
    filters.iter().all(|filter| {
        if let Some(exact) = &filter.exact_path
            && rel_path != exact
        {
            return false;
        }
        if let Some(prefix) = &filter.path_prefix
            && !rel_path.starts_with(prefix)
        {
            return false;
        }
        if let Some(ext) = &filter.extension {
            let actual = Path::new(rel_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if !actual.eq_ignore_ascii_case(ext.trim_start_matches('.')) {
                return false;
            }
        }
        if let Some(matcher) = &filter.glob
            && !matcher.is_match(rel_path)
        {
            return false;
        }
        true
    })
}

fn compile_rg_clauses(
    rg_blocks: &[Vec<String>],
) -> Result<Vec<CompiledRgClause>, ConvergedSearchExecutionError> {
    rg_blocks
        .iter()
        .map(|argv| {
            let analysis = analyze_native_rg_argv(argv);
            let ignore_case = analysis
                .options
                .iter()
                .any(|opt| opt.option == "-i" || opt.option == "--ignore-case");
            let fixed_strings = analysis
                .options
                .iter()
                .any(|opt| opt.option == "-F" || opt.option == "--fixed-strings");
            let mut matchers = Vec::with_capacity(analysis.patterns.len());
            let mut branch_plans = Vec::with_capacity(analysis.patterns.len());
            for pattern in &analysis.patterns {
                let source = if fixed_strings {
                    regex::escape(&pattern.value)
                } else {
                    pattern.value.clone()
                };
                let regex = RegexBuilder::new(&source)
                    .case_insensitive(ignore_case)
                    .build()
                    .map_err(|err| {
                        ConvergedSearchExecutionError::InvalidPattern(err.to_string())
                    })?;
                branch_plans.push(compile_regex_trigram_query(&source, ignore_case));
                matchers.push(regex);
            }
            let trigram_plan = if branch_plans.is_empty() {
                TrigramQuery::All
            } else {
                TrigramQuery::or(branch_plans)
            };
            Ok(CompiledRgClause {
                matchers,
                trigram_plan,
            })
        })
        .collect()
}

fn compile_tantivy_clauses(tantivy_blocks: &[Vec<String>]) -> Vec<CompiledTantivyClause> {
    tantivy_blocks
        .iter()
        .filter_map(|argv| argv.first())
        .map(|expr| {
            let analysis = analyze_tantivy_query(expr);
            let mut terms = BTreeSet::new();
            for selector in analysis.selector_queries {
                for token in tokenize_search_terms(&selector) {
                    terms.insert(token);
                }
            }
            for token in tokenize_search_terms(&analysis.expression) {
                if !matches!(
                    token.as_str(),
                    "and" | "or" | "not" | "title" | "body" | "path" | "symbol"
                ) {
                    terms.insert(token);
                }
            }
            CompiledTantivyClause {
                terms: terms.into_iter().collect(),
            }
        })
        .collect()
}

fn tokenize_search_terms(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .map(|tok| tok.trim().to_ascii_lowercase())
        .filter(|tok| tok.len() >= 2 && !tok.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

fn scan_workspace_files(
    workspace_root: &Path,
    allowed_extensions: &Option<BTreeSet<&'static str>>,
    topology_filters: &[CompiledTopologyFilter],
) -> Vec<ScannedFile> {
    let mut files = Vec::new();
    let walker = WalkBuilder::new(workspace_root)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(workspace_root) else {
            continue;
        };
        let rel_path = rel.to_string_lossy().replace('\\', "/");
        if !matches_topology_filters(&rel_path, topology_filters) {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if let Some(allowed) = allowed_extensions
            && !allowed.contains(ext)
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(path) else {
            continue;
        };
        let content_hash = blake3::hash(body.as_bytes()).to_hex().to_string();
        files.push(ScannedFile {
            relative_path: rel_path,
            language: infer_language_from_extension(ext),
            content_hash,
            body,
        });
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

fn compute_workspace_generation_digest(
    scanned: &[ScannedFile],
) -> Result<SearchGenerationDigest, ConvergedSearchExecutionError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.converged-search.generation.v1\n");
    for file in scanned {
        hasher.update(file.relative_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(file.content_hash.as_bytes());
        hasher.update(b"\n");
    }
    let digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    Ok(SearchGenerationDigest::new(digest)?)
}

fn evaluate_file(
    file: &ScannedFile,
    rg_clauses: &[CompiledRgClause],
    tantivy_clauses: &[CompiledTantivyClause],
    has_topology: bool,
) -> Option<ConvergedSearchHit> {
    let mut matched_axes = Vec::new();
    let mut line_matches = Vec::new();
    let mut bonus_score = 0u32;

    if has_topology {
        matched_axes.push("topology".to_owned());
        bonus_score = bonus_score.saturating_add(10);
    }

    if !rg_clauses.is_empty() {
        let matched = evaluate_rg_clauses(&file.body, rg_clauses, &mut line_matches);
        if !matched {
            return None;
        }
        matched_axes.push("rg".to_owned());
        bonus_score = bonus_score.saturating_add(50 + (line_matches.len() as u32) * 5);
    }

    if !tantivy_clauses.is_empty() {
        let lower_body = file.body.to_ascii_lowercase();
        let lower_path = file.relative_path.to_ascii_lowercase();
        let mut tantivy_hits = 0u32;
        for clause in tantivy_clauses {
            let clause_hits = clause
                .terms
                .iter()
                .filter(|term| {
                    lower_body.contains(term.as_str()) || lower_path.contains(term.as_str())
                })
                .count() as u32;
            if clause_hits == 0 && !clause.terms.is_empty() {
                return None;
            }
            tantivy_hits = tantivy_hits.saturating_add(clause_hits);
        }
        matched_axes.push("tantivy".to_owned());
        bonus_score = bonus_score.saturating_add(25 + tantivy_hits * 8);
    }

    Some(ConvergedSearchHit {
        path: file.relative_path.clone(),
        language: file.language.clone(),
        score: bonus_score,
        matched_axes,
        line_matches,
    })
}

fn evaluate_rg_clauses(
    body: &str,
    rg_clauses: &[CompiledRgClause],
    out_matches: &mut Vec<ConvergedLineMatch>,
) -> bool {
    for clause in rg_clauses {
        if clause.matchers.is_empty() {
            continue;
        }
        let mut clause_matched = false;
        for (idx, line) in body.lines().enumerate() {
            if clause.matchers.iter().any(|re| re.is_match(line)) {
                clause_matched = true;
                if out_matches.len() < MAX_LINE_MATCHES_PER_FILE {
                    out_matches.push(ConvergedLineMatch {
                        line_number: idx + 1,
                        line_text: truncate_preview(line),
                    });
                }
            }
        }
        if !clause_matched {
            return false;
        }
    }
    true
}

fn truncate_preview(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= MAX_LINE_PREVIEW_BYTES {
        return trimmed.to_owned();
    }
    let mut end = MAX_LINE_PREVIEW_BYTES;
    while !trimmed.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    trimmed[..end].to_owned()
}

fn collect_admitted_axes(request: &ProgressiveSearchPlaybookRequest) -> Vec<String> {
    let mut axes = Vec::new();
    if !request.topology.is_empty() {
        axes.push("topology".to_owned());
    }
    if !request.rg.is_empty() {
        axes.push("rg".to_owned());
    }
    if !request.tantivy.is_empty() {
        axes.push("tantivy".to_owned());
    }
    if !request.syntax.is_empty() || !request.native_syntax.is_empty() {
        axes.push("syntax".to_owned());
    }
    if !request.graph.is_empty() {
        axes.push("graph".to_owned());
    }
    axes
}

fn build_rank_query_hint(
    rg_clauses: &[CompiledRgClause],
    tantivy_clauses: &[CompiledTantivyClause],
    request: &ProgressiveSearchPlaybookRequest,
) -> String {
    let mut tokens = Vec::new();
    for clause in rg_clauses {
        for matcher in &clause.matchers {
            tokens.push(matcher.as_str().to_owned());
        }
    }
    for clause in tantivy_clauses {
        tokens.extend(clause.terms.iter().cloned());
    }
    if tokens.is_empty()
        && let Some(block) = request.topology.first()
        && let Some(prefix) = &block.path_prefix
    {
        tokens.push(prefix.clone());
    }
    tokens.join(" ")
}

fn rank_converged_hits(
    mut hits: Vec<ConvergedSearchHit>,
    query_hint: &str,
) -> Vec<ConvergedSearchHit> {
    if hits.is_empty() {
        return hits;
    }
    let candidates: Vec<SourceIndexRankCandidate> = hits
        .iter()
        .enumerate()
        .map(|(ordinal, hit)| SourceIndexRankCandidate {
            ordinal,
            path: hit.path.clone(),
            query_keys: hit.matched_axes.clone(),
        })
        .collect();
    let report = rank_source_index_report(SourceIndexRankRequest {
        query: query_hint.to_owned(),
        candidates,
    });
    for ranked in report.ranked_candidates {
        if let Some(hit) = hits.get_mut(ranked.candidate.ordinal) {
            hit.score = hit.score.saturating_add(u32::from(ranked.score.total));
        }
    }
    hits.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    hits
}
