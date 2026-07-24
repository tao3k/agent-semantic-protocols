//! Gerbil distribution dependency index core.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Default maximum number of Gerbil dependency index exports returned by one search.
pub const DEFAULT_GERBIL_DEPS_SEARCH_LIMIT: usize = 80;

/// Validated Gerbil distribution module identifier, such as `:std/srfi/13`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GerbilDepsModuleId(String);

impl GerbilDepsModuleId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for GerbilDepsModuleId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for GerbilDepsModuleId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// Search request for exports in an active Gerbil distribution module.
pub struct GerbilDepsSearchRequest {
    /// Gerbil module identifier, such as `:std/srfi/13`.
    pub module_id: String,
    /// Raw query string supplied by the caller.
    pub query: String,
    /// Lowercased query terms used by the index matcher.
    pub terms: Vec<String>,
    /// Maximum number of exports to return.
    pub limit: usize,
}

/// Search result for a Gerbil distribution module export query.
pub struct GerbilDepsSearchResult {
    /// Gerbil module identifier that was searched.
    pub module_id: String,
    /// Coarse distribution scope for agent-facing receipts.
    pub scope: &'static str,
    /// Raw query string supplied by the caller.
    pub query: String,
    /// Export names that matched the query.
    pub exports: Vec<String>,
}

/// Query request for a specific Gerbil export selector.
pub struct GerbilDepsQueryRequest {
    /// Stable selector, such as `gerbil:/std/srfi/13#export/string-prefix?`.
    pub selector: String,
    /// Gerbil module identifier derived from the selector.
    pub module_id: String,
    /// Export name derived from the selector.
    pub export_name: String,
}

/// Source window for a Gerbil export selector.
pub struct GerbilDepsQueryResult {
    /// Stable selector that was queried.
    pub selector: String,
    /// Gerbil module identifier that owns the export.
    pub module_id: String,
    /// Export name that was queried.
    pub export_name: String,
    /// Source file that contains the returned window.
    pub source_path: PathBuf,
    /// One-based source line for the definition when available.
    pub source_line: Option<usize>,
    /// Smallest useful source window for the export.
    pub source_text: String,
}

struct GerbilInstall {
    home: PathBuf,
}

struct Definition {
    path: PathBuf,
    line: usize,
    text: String,
}

/// Search exports in the active Gerbil distribution resolved from `gxi` on `PATH`.
pub fn gerbil_deps_search_exports(
    request: &GerbilDepsSearchRequest,
) -> Result<GerbilDepsSearchResult, String> {
    let install = resolve_active_gerbil_install()?;
    search_exports_with_home(&install.home, request)
}

fn search_exports_with_home(
    gerbil_home: &Path,
    request: &GerbilDepsSearchRequest,
) -> Result<GerbilDepsSearchResult, String> {
    let source = module_source_path(gerbil_home, &request.module_id);
    let source_text = fs::read_to_string(&source).map_err(|error| {
        format!(
            "failed to read Gerbil module `{}` at {}: {error}",
            request.module_id,
            source.display()
        )
    })?;
    let exports = filter_exports_for_query(extract_exported_symbols(&source_text), &request.terms)
        .into_iter()
        .take(request.limit)
        .collect::<Vec<_>>();
    Ok(GerbilDepsSearchResult {
        module_id: request.module_id.clone(),
        scope: module_scope(&request.module_id),
        query: request.query.clone(),
        exports,
    })
}

/// Query an export definition from the active Gerbil distribution.
pub fn gerbil_deps_query_export(
    request: &GerbilDepsQueryRequest,
) -> Result<GerbilDepsQueryResult, String> {
    let install = resolve_active_gerbil_install()?;
    query_export_with_home(&install.home, request)
}

fn query_export_with_home(
    gerbil_home: &Path,
    request: &GerbilDepsQueryRequest,
) -> Result<GerbilDepsQueryResult, String> {
    let source = module_source_path(gerbil_home, &request.module_id);
    let mut paths = vec![source.clone()];
    paths.extend(included_source_paths(&source)?);

    if let Some(definition) = find_definition(&paths, &request.export_name)? {
        return Ok(GerbilDepsQueryResult {
            selector: request.selector.clone(),
            module_id: request.module_id.clone(),
            export_name: request.export_name.clone(),
            source_path: definition.path,
            source_line: Some(definition.line),
            source_text: definition.text,
        });
    }

    let text = fs::read_to_string(&source).map_err(|error| {
        format!(
            "failed to read Gerbil module `{}` at {}: {error}",
            request.module_id,
            source.display()
        )
    })?;
    let export_form = extract_head_form(&text, "export").ok_or_else(|| {
        format!(
            "Gerbil module `{}` does not contain an export form at {}",
            request.module_id,
            source.display()
        )
    })?;
    Ok(GerbilDepsQueryResult {
        selector: request.selector.clone(),
        module_id: request.module_id.clone(),
        export_name: request.export_name.clone(),
        source_path: source,
        source_line: None,
        source_text: export_form.to_string(),
    })
}

/// Validate that a Gerbil module identifier is specific enough for deps search.
pub fn gerbil_deps_validate_module_id(
    module_id: impl Into<GerbilDepsModuleId>,
) -> Result<GerbilDepsModuleId, String> {
    let module_id = module_id.into();
    let module_id_str = module_id.as_str();
    let Some(path) = module_id_str.strip_prefix(':') else {
        return Err("module-id-required".to_string());
    };
    if path.is_empty()
        || path.contains("..")
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('*')
        || path.contains('?')
        || !path.contains('/')
    {
        return Err("specific-module-id-required".to_string());
    }
    Ok(module_id)
}

/// Validate that a selector export name cannot escape the module source tree.
pub fn gerbil_deps_validate_symbol(symbol: &str) -> Result<(), String> {
    if symbol.contains('/') || symbol.contains("..") || symbol.contains('#') || symbol.is_empty() {
        return Err(format!(
            "invalid Gerbil export selector symbol `{symbol}`; use gerbil:/std/srfi/13#export/string-prefix?"
        ));
    }
    Ok(())
}

/// Split a raw Gerbil deps query into normalized matcher terms.
pub fn gerbil_deps_query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '|' | ','))
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Build the compact `only-in` import form for matched Gerbil exports.
pub fn gerbil_deps_minimal_import(
    module_id: impl Into<GerbilDepsModuleId>,
    names: &[String],
) -> String {
    let module_id = module_id.into();
    format!(
        "(import (only-in {} {}))",
        module_id.as_str(),
        names.join(" ")
    )
}

/// Build the stable selector for an export in a Gerbil module.
pub fn gerbil_deps_selector_for(module_id: impl Into<GerbilDepsModuleId>, name: &str) -> String {
    let module_id = module_id.into();
    format!(
        "gerbil:/{}#export/{name}",
        module_id.as_str().trim_start_matches(':')
    )
}

fn resolve_active_gerbil_install() -> Result<GerbilInstall, String> {
    let gxi = which::which("gxi").map_err(|error| {
        format!(
            "failed to locate active `gxi` for Gerbil deps index: {error}; ensure gxi is on PATH"
        )
    })?;
    let gxi = fs::canonicalize(&gxi).unwrap_or(gxi);
    let mut seen = HashSet::new();
    for candidate in gerbil_home_candidates_from_gxi(&gxi) {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if candidate.join("src").is_dir() {
            return Ok(GerbilInstall { home: candidate });
        }
    }
    Err(format!(
        "failed to locate Gerbil source tree from active gxi {}; expected <gerbil-home>/src",
        gxi.display()
    ))
}

fn gerbil_home_candidates_from_gxi(gxi: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(bin_dir) = gxi.parent()
        && let Some(prefix) = bin_dir.parent()
    {
        candidates.push(prefix.to_path_buf());
        if let Ok(entries) = fs::read_dir(prefix) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    candidates.push(path);
                }
            }
        }
    }
    candidates
}

fn module_source_path(gerbil_home: &Path, module_id: &str) -> PathBuf {
    gerbil_home
        .join("src")
        .join(module_id.trim_start_matches(':'))
        .with_extension("ss")
}

fn included_source_paths(module_source: &Path) -> Result<Vec<PathBuf>, String> {
    let source_text = fs::read_to_string(module_source).map_err(|error| {
        format!(
            "failed to read Gerbil module source {}: {error}",
            module_source.display()
        )
    })?;
    let source_dir = module_source
        .parent()
        .ok_or_else(|| format!("module source has no parent: {}", module_source.display()))?;
    Ok(extract_include_paths(&source_text)
        .into_iter()
        .map(|include| source_dir.join(include))
        .collect())
}

fn extract_exported_symbols(text: &str) -> Vec<String> {
    let Some(form) = extract_head_form(text, "export") else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    tokens(form)
        .into_iter()
        .skip(1)
        .filter(|token| token != "." && !token.ends_with(':'))
        .filter(|token| seen.insert(token.clone()))
        .collect()
}

fn extract_include_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut rest = text;
    while let Some(offset) = rest.find("(include") {
        rest = &rest[offset + "(include".len()..];
        let Some(start_quote) = rest.find('"') else {
            break;
        };
        let after_quote = &rest[start_quote + 1..];
        let Some(end_quote) = after_quote.find('"') else {
            break;
        };
        let path = &after_quote[..end_quote];
        if !path.contains("..") && !path.starts_with('/') {
            paths.push(path.to_string());
        }
        rest = &after_quote[end_quote + 1..];
    }
    paths
}

fn find_definition(paths: &[PathBuf], name: &str) -> Result<Option<Definition>, String> {
    for path in paths {
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if let Some((line, text)) = definition_window(&text, name) {
            return Ok(Some(Definition {
                path: path.clone(),
                line,
                text,
            }));
        }
    }
    Ok(None)
}

fn definition_window(text: &str, name: &str) -> Option<(usize, String)> {
    let offsets = line_offsets(text);
    let lines = text.lines().collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !is_definition_line(trimmed, name) {
            continue;
        }
        let indent = line.len() - trimmed.len();
        let form_start = offsets[index] + indent;
        let form_end = find_matching_paren(text, form_start)?;
        let window_start_line = leading_comment_start(&lines, index);
        let window_start = offsets[window_start_line];
        let mut window = text[window_start..form_end].to_string();
        if text[form_end..].starts_with('\n') {
            window.push('\n');
        }
        return Some((index + 1, window));
    }
    None
}

fn line_offsets(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.bytes().enumerate().filter_map(|(index, byte)| {
            (byte == b'\n' && index + 1 < text.len()).then_some(index + 1)
        }))
        .collect()
}

fn leading_comment_start(lines: &[&str], definition_line: usize) -> usize {
    let mut start = definition_line;
    while start > 0 {
        let previous = lines[start - 1].trim_start();
        if previous.starts_with(";;") || previous.is_empty() {
            start -= 1;
        } else {
            break;
        }
    }
    start
}

fn is_definition_line(line: &str, name: &str) -> bool {
    if let Some(rest) = line.strip_prefix("(def (") {
        return rest.starts_with(name) && token_boundary(rest, name.len());
    }
    if let Some(rest) = line.strip_prefix("(def ") {
        return rest.starts_with(name) && token_boundary(rest, name.len());
    }
    if let Some(rest) = line.strip_prefix("(defsyntax ") {
        return rest.starts_with(name) && token_boundary(rest, name.len());
    }
    if let Some(rest) = line.strip_prefix("(defrules ") {
        return rest.starts_with(name) && token_boundary(rest, name.len());
    }
    false
}

fn token_boundary(text: &str, index: usize) -> bool {
    text[index..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_ascii_whitespace() || ch == ')' || ch == '(')
}

fn extract_head_form<'a>(text: &'a str, head: &str) -> Option<&'a str> {
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    for (index, ch) in text.char_indices() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            ';' => in_line_comment = true,
            '"' => in_string = true,
            '(' => {
                let after_open = skip_ascii_ws(&text[index + 1..]);
                let token_start = index + 1 + after_open;
                let token = read_token(&text[token_start..]);
                if token == head {
                    let end = find_matching_paren(text, index)?;
                    return Some(&text[index..end]);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_matching_paren(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    for (relative, ch) in text[start..].char_indices() {
        let index = start + relative;
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            ';' => in_line_comment = true,
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    for ch in text.chars() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            ';' => {
                push_token(&mut tokens, &mut current);
                in_line_comment = true;
            }
            '"' => {
                push_token(&mut tokens, &mut current);
                in_string = true;
            }
            '(' | ')' | '[' | ']' | '\'' | '`' | ',' => {
                push_token(&mut tokens, &mut current);
            }
            ch if ch.is_ascii_whitespace() => {
                push_token(&mut tokens, &mut current);
            }
            _ => current.push(ch),
        }
    }
    push_token(&mut tokens, &mut current);
    tokens
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn skip_ascii_ws(text: &str) -> usize {
    text.bytes()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count()
}

fn read_token(text: &str) -> &str {
    let end = text
        .char_indices()
        .find_map(|(index, ch)| {
            if ch.is_ascii_whitespace() || matches!(ch, '(' | ')' | '[' | ']') {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(text.len());
    &text[..end]
}

fn filter_exports_for_query(exports: Vec<String>, terms: &[String]) -> Vec<String> {
    let precise = exports
        .iter()
        .filter(|name| symbol_precisely_matches_query(name, terms))
        .cloned()
        .collect::<Vec<_>>();
    if precise.is_empty() {
        exports
            .into_iter()
            .filter(|name| symbol_contains_query(name, terms))
            .collect()
    } else {
        precise
    }
}

fn symbol_precisely_matches_query(name: &str, terms: &[String]) -> bool {
    let normalized = normalize_symbol_family(name);
    terms.iter().any(|term| normalized == *term)
}

fn normalize_symbol_family(name: &str) -> String {
    let mut normalized = name
        .trim_end_matches(|ch| matches!(ch, '?' | '!'))
        .to_ascii_lowercase();
    if let Some(base) = normalized.strip_suffix("-ci") {
        normalized = base.to_string();
    }
    normalized
}

fn symbol_contains_query(name: &str, terms: &[String]) -> bool {
    let name = name.to_ascii_lowercase();
    terms.iter().any(|term| name.contains(term))
}

fn module_scope(module_id: &str) -> &'static str {
    if module_id.starts_with(":std/srfi/") {
        "standard-library/srfi"
    } else if module_id.starts_with(":std/") {
        "standard-library"
    } else if module_id.starts_with(":gerbil/compiler/") {
        "compiler"
    } else if module_id.starts_with(":gerbil/expander/") {
        "expander"
    } else if module_id == ":gerbil/gambit" {
        "runtime-bridge"
    } else if module_id.starts_with(":gerbil/") {
        "gerbil-core"
    } else {
        "gerbil-distribution"
    }
}
