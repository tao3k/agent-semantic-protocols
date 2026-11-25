// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Pure-Rust resident symbol extraction and 1-hop structural import/reference graph
//! queries for [`super::WorkspaceSearchIndex`].
//!
//! Runs with zero external subprocesses during both initial build / zero-copy `mmap`
//! snapshot assembly and incremental `apply_file_delta` updates.

use memchr::memmem::Finder;
use rustc_hash::{FxHashMap, FxHashSet};

use super::build::ascii_lower_bytes;
use super::index::{IndexedFile, WorkspaceSearchIndex};
use super::query::{CompiledFilters, FusedHit, extract_line_slice, truncate_line};
use crate::ConvergedLineMatch;

/// Structural category of a symbol extracted into the resident index.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WorkspaceSymbolKind {
    Function,
    Type,
    Trait,
    Module,
    Constant,
    Macro,
    Class,
    Interface,
    Heading,
}

impl WorkspaceSymbolKind {
    /// Stable lowercase identifier for UI badges and protocol projections.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "fn",
            Self::Type => "type",
            Self::Trait => "trait",
            Self::Module => "mod",
            Self::Constant => "const",
            Self::Macro => "macro",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Heading => "heading",
        }
    }
}

/// One matched or enclosing symbol returned on [`super::WorkspaceSearchHit`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSymbolMatch {
    pub name: String,
    pub kind: WorkspaceSymbolKind,
    pub line_number: usize,
}

#[derive(Clone, Debug)]
pub(super) struct IndexedSymbol {
    pub(super) name: String,
    pub(super) name_lower: String,
    pub(super) kind: WorkspaceSymbolKind,
    pub(super) line_number: usize,
}

pub(super) struct FileSemantics {
    pub(super) symbols: Vec<IndexedSymbol>,
    pub(super) imports: Vec<String>,
}

/// Extract structural symbol definitions and lightweight import targets in a single pass.
pub(super) fn extract_file_semantics(language: Option<&str>, body: &str) -> FileSemantics {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let mut seen_imports: FxHashSet<String> = FxHashSet::default();

    for (idx, raw_line) in body.lines().enumerate() {
        let line_number = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match language {
            Some("rust") => scan_rust_line(
                trimmed,
                line_number,
                &mut symbols,
                &mut imports,
                &mut seen_imports,
            ),
            Some("python") => scan_python_line(
                trimmed,
                line_number,
                &mut symbols,
                &mut imports,
                &mut seen_imports,
            ),
            Some("typescript" | "javascript") => scan_ts_line(
                trimmed,
                line_number,
                &mut symbols,
                &mut imports,
                &mut seen_imports,
            ),
            Some("scheme") => scan_scheme_line(
                trimmed,
                line_number,
                &mut symbols,
                &mut imports,
                &mut seen_imports,
            ),
            Some("julia") => scan_julia_line(
                trimmed,
                line_number,
                &mut symbols,
                &mut imports,
                &mut seen_imports,
            ),
            Some("sysml" | "quint") => scan_model_line(
                trimmed,
                line_number,
                &mut symbols,
                &mut imports,
                &mut seen_imports,
            ),
            Some("org" | "markdown") => scan_doc_heading(trimmed, line_number, &mut symbols),
            _ => {}
        }
    }

    FileSemantics { symbols, imports }
}

/// Find the closest defining symbol at or above `focus_line` (or first symbol in file).
pub(super) fn nearest_symbols_for_line(
    file: &IndexedFile,
    focus_line: usize,
) -> Vec<WorkspaceSymbolMatch> {
    if file.symbols.is_empty() {
        return Vec::new();
    }
    let enclosing = file
        .symbols
        .iter()
        .rev()
        .find(|sym| sym.line_number <= focus_line)
        .or_else(|| file.symbols.first());
    enclosing
        .map(|sym| {
            vec![WorkspaceSymbolMatch {
                name: sym.name.clone(),
                kind: sym.kind,
                line_number: sym.line_number,
            }]
        })
        .unwrap_or_default()
}

/// Execute `@sym:<needle>` resident symbol-definition search across live files.
pub(super) fn run_symbol_axis(
    index: &WorkspaceSearchIndex,
    symbol_needle: &str,
    filters: &CompiledFilters,
    fused: &mut FxHashMap<u32, FusedHit>,
) -> usize {
    let clean = symbol_needle.trim();
    if clean.is_empty() {
        return 0;
    }
    let needle_lower = clean.to_ascii_lowercase();
    let max_chars = index.config.max_line_preview_chars.max(16);
    let mut matched_count = 0usize;

    for (file_id, file) in index.live_files() {
        if !filters.matches(file) || file.symbols.is_empty() {
            continue;
        }
        let mut best_score = 0u32;
        let mut matches = Vec::new();
        for sym in &file.symbols {
            let score = if sym.name == clean {
                220u32
            } else if sym.name_lower == needle_lower {
                200u32
            } else if sym.name_lower.starts_with(&needle_lower) {
                160u32
            } else if sym.name_lower.contains(&needle_lower) {
                120u32
            } else {
                0u32
            };
            if score > 0 {
                best_score = best_score.max(score);
                matches.push(WorkspaceSymbolMatch {
                    name: sym.name.clone(),
                    kind: sym.kind,
                    line_number: sym.line_number,
                });
            }
        }
        if matches.is_empty() {
            continue;
        }
        matched_count += 1;
        let focus_line = matches[0].line_number;
        let line_match = ConvergedLineMatch {
            line_number: focus_line,
            line_text: truncate_line(extract_line_slice(file, focus_line), max_chars),
        };
        let entry = fused
            .entry(file_id)
            .or_insert_with(|| FusedHit::new(file_id));
        entry.score = entry.score.saturating_add(best_score);
        entry.add_axis("symbol");
        if entry.focus_line.is_none() {
            entry.focus_line = Some(focus_line);
        }
        if entry.line_matches.is_empty() {
            entry.line_matches.push(line_match);
        }
        for sym in matches.into_iter().take(4) {
            entry.add_symbol(sym);
        }
    }

    matched_count
}

/// Execute `@graph:<target>` 1-hop structural neighborhood search (definers, importers,
/// trigram-accelerated callers/referencers, and outgoing dependencies).
pub(super) fn run_graph_axis(
    index: &WorkspaceSearchIndex,
    graph_target: &str,
    filters: &CompiledFilters,
    fused: &mut FxHashMap<u32, FusedHit>,
) -> usize {
    let clean = graph_target.trim();
    if clean.is_empty() {
        return 0;
    }
    let target_lower = clean.to_ascii_lowercase();
    let max_chars = index.config.max_line_preview_chars.max(16);
    let mut definer_ids: FxHashSet<u32> = FxHashSet::default();
    let mut definer_stems: FxHashSet<String> = FxHashSet::default();
    let mut outgoing_imports: FxHashSet<String> = FxHashSet::default();
    definer_stems.insert(target_lower.clone());

    // 1. Locate defining files (by symbol name or file/module path stem).
    for (file_id, file) in index.live_files() {
        let stem = file_stem_lower(&file.relative_path);
        let path_matches = file.relative_path.eq_ignore_ascii_case(clean) || stem == target_lower;
        let sym_hit = file
            .symbols
            .iter()
            .find(|sym| sym.name_lower == target_lower || sym.name_lower.contains(&target_lower));

        if !path_matches && sym_hit.is_none() {
            continue;
        }
        definer_ids.insert(file_id);
        if !stem.is_empty() && stem != "mod" && stem != "lib" && stem != "index" {
            definer_stems.insert(stem);
        }
        for imp in &file.imports {
            outgoing_imports.insert(imp.clone());
        }

        if !filters.matches(file) {
            continue;
        }
        let entry = fused
            .entry(file_id)
            .or_insert_with(|| FusedHit::new(file_id));
        entry.score = entry.score.saturating_add(210);
        entry.add_axis("graph");
        if let Some(sym) = sym_hit {
            entry.add_axis("symbol");
            entry.add_relation(format!("defines:{}", sym.name));
            entry.add_symbol(WorkspaceSymbolMatch {
                name: sym.name.clone(),
                kind: sym.kind,
                line_number: sym.line_number,
            });
            if entry.focus_line.is_none() {
                entry.focus_line = Some(sym.line_number);
            }
            if entry.line_matches.is_empty() {
                entry.line_matches.push(ConvergedLineMatch {
                    line_number: sym.line_number,
                    line_text: truncate_line(extract_line_slice(file, sym.line_number), max_chars),
                });
            }
        } else {
            entry.add_relation(format!("node:{clean}"));
        }
    }

    // 2. Locate importers and outgoing dependency targets across live files.
    for (file_id, file) in index.live_files() {
        if !filters.matches(file) {
            continue;
        }
        if !definer_ids.contains(&file_id)
            && let Some(imp) = file
                .imports
                .iter()
                .find(|imp| definer_stems.contains(imp.as_str()))
        {
            let entry = fused
                .entry(file_id)
                .or_insert_with(|| FusedHit::new(file_id));
            entry.score = entry.score.saturating_add(145);
            entry.add_axis("graph");
            entry.add_relation(format!("imports:{imp}"));
        }
        if !definer_ids.contains(&file_id) && !outgoing_imports.is_empty() {
            let stem = file_stem_lower(&file.relative_path);
            let defines_dep = file
                .symbols
                .iter()
                .find(|sym| outgoing_imports.contains(sym.name_lower.as_str()));
            if let Some(dep_sym) = defines_dep {
                let entry = fused
                    .entry(file_id)
                    .or_insert_with(|| FusedHit::new(file_id));
                entry.score = entry.score.saturating_add(120);
                entry.add_axis("graph");
                entry.add_relation(format!("dep-defines:{}", dep_sym.name));
                entry.add_symbol(WorkspaceSymbolMatch {
                    name: dep_sym.name.clone(),
                    kind: dep_sym.kind,
                    line_number: dep_sym.line_number,
                });
            } else if !stem.is_empty() && outgoing_imports.contains(&stem) {
                let entry = fused
                    .entry(file_id)
                    .or_insert_with(|| FusedHit::new(file_id));
                entry.score = entry.score.saturating_add(115);
                entry.add_axis("graph");
                entry.add_relation(format!("dep-module:{stem}"));
            }
        }
    }

    // 3. Trigram-accelerated callers / referencers for `target_lower`.
    let needle_bytes = ascii_lower_bytes(clean.as_bytes());
    let ref_candidates = index
        .trigrams
        .query_literal(&needle_bytes, &index.tombstones);
    let finder = Finder::new(&needle_bytes);
    for file_id in ref_candidates {
        if definer_ids.contains(&file_id) || index.is_tombstoned(file_id) {
            continue;
        }
        let Some(file) = index.files.get(file_id as usize) else {
            continue;
        };
        if !filters.matches(file) {
            continue;
        }
        if let Some(byte_pos) = finder.find(&file.body_lower) {
            let line_num = super::query::byte_offset_to_line(&file.line_offsets, byte_pos);
            let entry = fused
                .entry(file_id)
                .or_insert_with(|| FusedHit::new(file_id));
            entry.score = entry.score.saturating_add(130);
            entry.add_axis("graph");
            entry.add_relation(format!("references:{clean}"));
            if entry.focus_line.is_none() {
                entry.focus_line = Some(line_num);
            }
            if entry.line_matches.is_empty() {
                entry.line_matches.push(ConvergedLineMatch {
                    line_number: line_num,
                    line_text: truncate_line(extract_line_slice(file, line_num), max_chars),
                });
            }
        }
    }

    fused.len()
}

fn push_symbol(
    symbols: &mut Vec<IndexedSymbol>,
    name: &str,
    kind: WorkspaceSymbolKind,
    line_number: usize,
) {
    let clean = name.trim();
    if clean.len() < 2 {
        return;
    }
    symbols.push(IndexedSymbol {
        name: clean.to_owned(),
        name_lower: clean.to_ascii_lowercase(),
        kind,
        line_number,
    });
}

fn push_import(imports: &mut Vec<String>, seen: &mut FxHashSet<String>, raw: &str) {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.len() < 2
        || matches!(
            lower.as_str(),
            "self" | "super" | "crate" | "std" | "core" | "alloc" | "as" | "pub" | "from"
        )
    {
        return;
    }
    if seen.insert(lower.clone()) {
        imports.push(lower);
    }
}

fn take_ident(rest: &str) -> Option<&str> {
    let trimmed = rest.trim_start();
    let end = trimmed
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '!'))
        .unwrap_or(trimmed.len());
    let ident = trimmed[..end].trim_end_matches('!');
    (!ident.is_empty()).then_some(ident)
}

fn strip_rust_visibility(line: &str) -> &str {
    let mut s = line;
    if let Some(rest) = s.strip_prefix("pub") {
        if let Some(after_paren) = rest.strip_prefix('(')
            && let Some((_, after)) = after_paren.split_once(')')
        {
            s = after.trim_start();
        } else if rest.starts_with(char::is_whitespace) {
            s = rest.trim_start();
        }
    }
    for kw in ["default ", "async ", "unsafe ", "const ", "extern \"C\" "] {
        if let Some(rest) = s.strip_prefix(kw) {
            s = rest.trim_start();
        }
    }
    s
}

fn scan_rust_line(
    trimmed: &str,
    line_number: usize,
    symbols: &mut Vec<IndexedSymbol>,
    imports: &mut Vec<String>,
    seen_imports: &mut FxHashSet<String>,
) {
    if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*') {
        return;
    }
    let body = strip_rust_visibility(trimmed);
    if let Some(rest) = body.strip_prefix("use ") {
        for token in rest.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
            push_import(imports, seen_imports, token);
        }
        return;
    }
    if let Some(rest) = body.strip_prefix("fn ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Function, line_number);
        }
    } else if let Some(rest) = body
        .strip_prefix("struct ")
        .or_else(|| body.strip_prefix("enum "))
        .or_else(|| body.strip_prefix("type "))
    {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Type, line_number);
        }
    } else if let Some(rest) = body.strip_prefix("trait ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Trait, line_number);
        }
    } else if let Some(rest) = body.strip_prefix("mod ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Module, line_number);
            if trimmed.ends_with(';') {
                push_import(imports, seen_imports, name);
            }
        }
    } else if let Some(rest) = body
        .strip_prefix("const ")
        .or_else(|| body.strip_prefix("static "))
    {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Constant, line_number);
        }
    } else if let Some(rest) = body.strip_prefix("macro_rules! ")
        && let Some(name) = take_ident(rest)
    {
        push_symbol(symbols, name, WorkspaceSymbolKind::Macro, line_number);
    }
}

fn scan_python_line(
    trimmed: &str,
    line_number: usize,
    symbols: &mut Vec<IndexedSymbol>,
    imports: &mut Vec<String>,
    seen_imports: &mut FxHashSet<String>,
) {
    if trimmed.starts_with('#') {
        return;
    }
    if let Some(rest) = trimmed
        .strip_prefix("def ")
        .or_else(|| trimmed.strip_prefix("async def "))
    {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Function, line_number);
        }
    } else if let Some(rest) = trimmed.strip_prefix("class ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Class, line_number);
        }
    } else if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
        for token in trimmed.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
            if token != "import" && token != "from" {
                push_import(imports, seen_imports, token);
            }
        }
    }
}

fn scan_ts_line(
    trimmed: &str,
    line_number: usize,
    symbols: &mut Vec<IndexedSymbol>,
    imports: &mut Vec<String>,
    seen_imports: &mut FxHashSet<String>,
) {
    if trimmed.starts_with("//") || trimmed.starts_with("/*") {
        return;
    }
    if trimmed.starts_with("import ") {
        for token in
            trimmed.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        {
            if token != "import" && token != "from" && token != "type" {
                push_import(imports, seen_imports, token);
            }
        }
        return;
    }
    let body = trimmed
        .strip_prefix("export ")
        .unwrap_or(trimmed)
        .strip_prefix("default ")
        .unwrap_or(trimmed.strip_prefix("export ").unwrap_or(trimmed));
    let body = body.strip_prefix("async ").unwrap_or(body);
    if let Some(rest) = body.strip_prefix("function ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Function, line_number);
        }
    } else if let Some(rest) = body.strip_prefix("class ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Class, line_number);
        }
    } else if let Some(rest) = body.strip_prefix("interface ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Interface, line_number);
        }
    } else if let Some(rest) = body
        .strip_prefix("type ")
        .or_else(|| body.strip_prefix("enum "))
    {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Type, line_number);
        }
    } else if trimmed.starts_with("export const ")
        && let Some(rest) = trimmed.strip_prefix("export const ")
        && let Some(name) = take_ident(rest)
    {
        push_symbol(symbols, name, WorkspaceSymbolKind::Constant, line_number);
    }
}

fn scan_scheme_line(
    trimmed: &str,
    line_number: usize,
    symbols: &mut Vec<IndexedSymbol>,
    imports: &mut Vec<String>,
    seen_imports: &mut FxHashSet<String>,
) {
    if trimmed.starts_with(';') {
        return;
    }
    if trimmed.starts_with("(import ") {
        for token in trimmed
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '/' | ':')))
        {
            if let Some(last) = token.rsplit('/').next()
                && last != "import"
            {
                push_import(imports, seen_imports, last.trim_start_matches(':'));
            }
        }
        return;
    }
    for (prefix, kind) in [
        ("(def (", WorkspaceSymbolKind::Function),
        ("(def ", WorkspaceSymbolKind::Constant),
        ("(defstruct ", WorkspaceSymbolKind::Type),
        ("(defclass ", WorkspaceSymbolKind::Class),
        ("(defmethod ", WorkspaceSymbolKind::Function),
        ("(defrule ", WorkspaceSymbolKind::Macro),
        ("(defsyntax ", WorkspaceSymbolKind::Macro),
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let clean = rest.trim_start_matches('(');
            if let Some(name) = take_ident(clean) {
                push_symbol(symbols, name, kind, line_number);
            }
            return;
        }
    }
}

fn scan_julia_line(
    trimmed: &str,
    line_number: usize,
    symbols: &mut Vec<IndexedSymbol>,
    imports: &mut Vec<String>,
    seen_imports: &mut FxHashSet<String>,
) {
    if trimmed.starts_with('#') {
        return;
    }
    if let Some(rest) = trimmed.strip_prefix("function ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Function, line_number);
        }
    } else if let Some(rest) = trimmed
        .strip_prefix("struct ")
        .or_else(|| trimmed.strip_prefix("mutable struct "))
    {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Type, line_number);
        }
    } else if let Some(rest) = trimmed.strip_prefix("module ") {
        if let Some(name) = take_ident(rest) {
            push_symbol(symbols, name, WorkspaceSymbolKind::Module, line_number);
        }
    } else if let Some(rest) = trimmed
        .strip_prefix("using ")
        .or_else(|| trimmed.strip_prefix("import "))
    {
        for token in rest.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
            push_import(imports, seen_imports, token);
        }
    }
}

fn scan_model_line(
    trimmed: &str,
    line_number: usize,
    symbols: &mut Vec<IndexedSymbol>,
    imports: &mut Vec<String>,
    seen_imports: &mut FxHashSet<String>,
) {
    if trimmed.starts_with("//") {
        return;
    }
    if trimmed.contains("import ") {
        for token in trimmed.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
            if token != "import" && token != "private" && token != "public" {
                push_import(imports, seen_imports, token);
            }
        }
        return;
    }
    for (prefix, kind) in [
        ("package ", WorkspaceSymbolKind::Module),
        ("module ", WorkspaceSymbolKind::Module),
        ("part def ", WorkspaceSymbolKind::Type),
        ("attribute def ", WorkspaceSymbolKind::Type),
        ("enum def ", WorkspaceSymbolKind::Type),
        ("state def ", WorkspaceSymbolKind::Type),
        ("action def ", WorkspaceSymbolKind::Function),
        ("action ", WorkspaceSymbolKind::Function),
        ("def ", WorkspaceSymbolKind::Function),
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix)
            && let Some(name) = take_ident(rest)
        {
            push_symbol(symbols, name, kind, line_number);
            return;
        }
    }
}

fn scan_doc_heading(trimmed: &str, line_number: usize, symbols: &mut Vec<IndexedSymbol>) {
    let heading = if let Some(rest) = trimmed.strip_prefix('*') {
        let rest = rest.trim_start_matches('*');
        rest.starts_with(' ').then(|| rest.trim())
    } else if let Some(rest) = trimmed.strip_prefix('#') {
        let rest = rest.trim_start_matches('#');
        rest.starts_with(' ').then(|| rest.trim())
    } else {
        None
    };
    if let Some(title) = heading
        && !title.is_empty()
    {
        push_symbol(symbols, title, WorkspaceSymbolKind::Heading, line_number);
    }
}

fn file_stem_lower(relative_path: &str) -> String {
    let basename = relative_path.rsplit('/').next().unwrap_or(relative_path);
    let stem = basename.rsplit_once('.').map_or(basename, |(stem, _)| stem);
    stem.to_ascii_lowercase()
}
