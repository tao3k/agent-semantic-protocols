//! Owner-local query-set frontier rendering.

use std::cmp::Reverse;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use agent_semantic_provider_transport::byte_text;

use super::search_language_files::language_file_spec;
use super::search_pipe_rust_item::rust_item_symbol_for_line;

pub(super) fn render_owner_query_frontier(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    owner: &Path,
    query: &str,
) -> String {
    let owner_path = if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        project_root.join(owner)
    };
    let display_owner = display_path(locator_root, &owner_path);
    let item_matches = find_owner_query_matches(language_id, &owner_path, query);
    let mut rendered = String::from(
        "[search-reasoning] q=owner-query alg=asp-fast-owner-query-v1\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,Q=query,T=test,O=owner,I=item}\n",
    );
    let _ = writeln!(
        rendered,
        "Q=query:term({query})!query;T=test:path({display_owner})!tests;O=owner:path({display_owner})!owner;"
    );
    for (index, item_match) in item_matches.iter().enumerate() {
        let item_id = numbered_id("I", index);
        let selector = format!("{display_owner}:{}:{}", item_match.start, item_match.end);
        let _ = writeln!(
            rendered,
            "{item_id}=item:symbol({})@{selector}!syntax;",
            item_match.term
        );
        if let Some(pattern) =
            owner_query_tree_sitter_pattern(language_id, item_match.kind, &item_match.term)
        {
            let _ = writeln!(
                rendered,
                "syntax {item_id} selector={selector} pattern='{pattern}'"
            );
        }
    }
    let mut edge_targets = vec![
        "Q:matches".to_string(),
        "T:covers".to_string(),
        "O:selects".to_string(),
    ];
    edge_targets.extend(
        numbered_ids("I", item_matches.len())
            .into_iter()
            .map(|id| format!("{id}:contains")),
    );
    let _ = writeln!(rendered, "G>{{{}}}", edge_targets.join(","));
    let mut rank = vec!["Q".to_string(), "T".to_string(), "O".to_string()];
    rank.extend(numbered_ids("I", item_matches.len()));
    let frontier = rank
        .iter()
        .map(|id| match id.as_str() {
            "Q" => "Q.query".to_string(),
            "T" => "T.tests".to_string(),
            "O" => "O.owner".to_string(),
            other => format!("{other}.syntax"),
        })
        .collect::<Vec<_>>();
    let _ = writeln!(
        rendered,
        "rank={} frontier={}",
        rank.join(","),
        frontier.join(",")
    );
    if item_matches.is_empty() {
        let _ = writeln!(rendered, "recommendedNext=scoped-rg-query");
        let _ = writeln!(
            rendered,
            "nextCommand=asp rg -query {} {}",
            shell_arg(query),
            shell_arg(&display_owner),
        );
        rendered.push_str("reason=no-owner-item-match\n");
    } else if let Some(item_match) = item_matches.first() {
        let selector = format!("{display_owner}:{}:{}", item_match.start, item_match.end);
        let _ = writeln!(rendered, "recommendedNext=query-selector");
        let _ = writeln!(
            rendered,
            "nextCommand=asp {language_id} query --selector {selector} --workspace . --code"
        );
        rendered.push_str("reason=owner-item-selector-ready\n");
    }
    rendered.push_str("entries=owner-query(O,Q=>items+tests+dependency-usage)\n");
    rendered
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        shell_quote(value)
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn owner_query_tree_sitter_pattern(language_id: &str, kind: &str, query: &str) -> Option<String> {
    let escaped_query = query.replace('\\', "\\\\").replace('"', "\\\"");
    match language_id {
        "rust" => rust_tree_sitter_pattern(kind, &escaped_query),
        "python" => python_tree_sitter_pattern(kind, &escaped_query),
        "typescript" | "javascript" => typescript_tree_sitter_pattern(kind, &escaped_query),
        _ => None,
    }
}

fn rust_tree_sitter_pattern(kind: &str, escaped_query: &str) -> Option<String> {
    let (node, capture) = match kind {
        "struct" => ("struct_item", "type.name"),
        "enum" => ("enum_item", "type.name"),
        "trait" => ("trait_item", "type.name"),
        "type" => ("type_item", "type.name"),
        "mod" => ("mod_item", "module.name"),
        "const" => ("const_item", "constant.name"),
        "static" => ("static_item", "constant.name"),
        "fn" => ("function_item", "function.name"),
        _ => return None,
    };
    Some(format!(
        "(({node} name: (_) @{capture}) (#eq? @{capture} \"{escaped_query}\"))"
    ))
}

fn python_tree_sitter_pattern(kind: &str, escaped_query: &str) -> Option<String> {
    let (node, capture) = match kind {
        "class" => ("class_definition", "class.name"),
        "function" => ("function_definition", "function.name"),
        _ => return None,
    };
    Some(format!(
        "(({node} name: (identifier) @{capture}) (#eq? @{capture} \"{escaped_query}\"))"
    ))
}

fn typescript_tree_sitter_pattern(kind: &str, escaped_query: &str) -> Option<String> {
    let pattern = match kind {
        "class" => {
            format!(
                "((class_declaration name: (type_identifier) @class.name) (#eq? @class.name \"{escaped_query}\"))"
            )
        }
        "interface" => {
            format!(
                "((interface_declaration name: (type_identifier) @interface.name) (#eq? @interface.name \"{escaped_query}\"))"
            )
        }
        "type" => {
            format!(
                "((type_alias_declaration name: (type_identifier) @type.name) (#eq? @type.name \"{escaped_query}\"))"
            )
        }
        "const" => {
            format!(
                "((lexical_declaration (variable_declarator name: (identifier) @constant.name)) (#eq? @constant.name \"{escaped_query}\"))"
            )
        }
        "function" => {
            format!(
                "((function_declaration name: (identifier) @function.name) (#eq? @function.name \"{escaped_query}\"))"
            )
        }
        _ => return None,
    };
    Some(pattern)
}

struct OwnerQueryMatch {
    start: usize,
    end: usize,
    kind: &'static str,
    term: String,
    match_rank: u8,
    axis_coverage: usize,
}

fn find_owner_query_matches(language_id: &str, path: &Path, query: &str) -> Vec<OwnerQueryMatch> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let terms = query_terms(query);
    let lines = byte_text::line_slices(&bytes);
    let mut matches = Vec::new();
    'line_scan: for (index, line) in lines.iter().enumerate() {
        let Some(kind) = item_kind_for_line(path, line) else {
            continue;
        };
        let symbol = item_symbol_for_line(path, line);
        let lower = byte_text::lowercase_lossy_string(line);
        for term in &terms {
            if !lower.contains(&term.lower)
                && !symbol
                    .as_ref()
                    .is_some_and(|symbol| symbol.to_lowercase().contains(&term.lower))
            {
                continue;
            }
            let term_display = symbol.as_ref().unwrap_or(&term.display);
            let match_rank = symbol
                .as_ref()
                .map(|symbol| {
                    if symbol.to_lowercase().starts_with(&term.lower) {
                        0
                    } else {
                        1
                    }
                })
                .unwrap_or(0);
            if matches.iter().any(|item: &OwnerQueryMatch| {
                item.start == index + 1 && item.term.eq_ignore_ascii_case(term_display)
            }) {
                continue;
            }
            let start = index + 1;
            let end = rust_block_end(path, &lines, index)
                .or_else(|| python_block_end(path, &lines, index))
                .or_else(|| typescript_block_end(path, &lines, index))
                .or_else(|| scheme_block_end(path, &lines, index))
                .unwrap_or(start + 1);
            matches.push(OwnerQueryMatch {
                start,
                end,
                kind,
                term: term_display.clone(),
                match_rank,
                axis_coverage: selector_axis_coverage(
                    &lines,
                    start,
                    end,
                    &terms,
                    symbol.as_deref(),
                ),
            });
            if matches.len() >= 16 {
                break 'line_scan;
            }
        }
    }
    let best_declaration_coverage = matches
        .iter()
        .map(|item| item.axis_coverage)
        .max()
        .unwrap_or(0);
    matches.extend(query_axis_window_matches(
        path,
        &terms,
        &lines,
        best_declaration_coverage,
    ));
    sort_owner_query_matches(&mut matches);
    deduplicate_owner_query_matches(&mut matches);
    matches.truncate(8);
    if matches.is_empty()
        && let Some(barrel_match) = typescript_barrel_owner_query_match(path, &terms, &lines)
    {
        matches.push(barrel_match);
    }
    if matches.is_empty()
        && let Some(config_match) = config_owner_query_match(language_id, path, &terms, &lines)
    {
        matches.push(config_match);
    }
    matches
}

fn sort_owner_query_matches(matches: &mut [OwnerQueryMatch]) {
    matches.sort_by_key(|item| {
        (
            Reverse(item.axis_coverage),
            item.match_rank,
            item_kind_priority(item.kind),
            item.end.saturating_sub(item.start),
            item.start,
        )
    });
}

fn deduplicate_owner_query_matches(matches: &mut Vec<OwnerQueryMatch>) {
    let mut unique = Vec::with_capacity(matches.len());
    for item in matches.drain(..) {
        if unique.iter().any(|existing: &OwnerQueryMatch| {
            existing.start == item.start && existing.end == item.end
        }) {
            continue;
        }
        unique.push(item);
    }
    *matches = unique;
}

fn query_axis_window_matches(
    path: &Path,
    terms: &[QueryTerm],
    lines: &[&[u8]],
    best_declaration_coverage: usize,
) -> Vec<OwnerQueryMatch> {
    if terms.len() < 2 || lines.is_empty() {
        return Vec::new();
    }
    let required_coverage = query_axis_window_required_coverage(terms.len());
    let mut matches = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line_matches_any_query_axis(line, terms) {
            continue;
        }
        let (start, end) = query_axis_window_range(path, lines, index);
        let axis_coverage = selector_axis_coverage(lines, start, end, terms, None);
        if axis_coverage < required_coverage || axis_coverage <= best_declaration_coverage {
            continue;
        }
        matches.push(OwnerQueryMatch {
            start,
            end,
            kind: "context",
            term: query_axis_window_label(lines, start, end, terms),
            match_rank: 3,
            axis_coverage,
        });
    }
    sort_owner_query_matches(&mut matches);
    deduplicate_owner_query_matches(&mut matches);
    matches.truncate(4);
    matches
}

fn query_axis_window_required_coverage(term_count: usize) -> usize {
    match term_count {
        0 | 1 => term_count,
        2 | 3 => 2,
        _ => (term_count * 2).div_ceil(3),
    }
}

fn line_matches_any_query_axis(line: &[u8], terms: &[QueryTerm]) -> bool {
    let lower = byte_text::lowercase_lossy_string(line);
    terms.iter().any(|term| lower.contains(&term.lower))
}

fn query_axis_window_range(path: &Path, lines: &[&[u8]], index: usize) -> (usize, usize) {
    if let Some(range) = enclosing_item_range(path, lines, index) {
        return range;
    }
    let start = index.saturating_sub(3) + 1;
    let end = (index + 4).min(lines.len());
    (start, end.max(start))
}

fn enclosing_item_range(path: &Path, lines: &[&[u8]], index: usize) -> Option<(usize, usize)> {
    let lower_bound = index.saturating_sub(200);
    for start_index in (lower_bound..=index).rev() {
        if item_kind_for_line(path, lines[start_index]).is_none() {
            continue;
        }
        let start = start_index + 1;
        let end = rust_block_end(path, lines, start_index)
            .or_else(|| python_block_end(path, lines, start_index))
            .or_else(|| typescript_block_end(path, lines, start_index))
            .or_else(|| scheme_block_end(path, lines, start_index))
            .unwrap_or(start);
        if end > index {
            return Some((start, end));
        }
    }
    None
}

fn query_axis_window_label(
    lines: &[&[u8]],
    start: usize,
    end: usize,
    terms: &[QueryTerm],
) -> String {
    let mut evidence = String::new();
    let start_index = start.saturating_sub(1);
    let end_index = end.min(lines.len());
    for line in &lines[start_index..end_index] {
        evidence.push_str(&byte_text::lowercase_lossy_string(line));
        evidence.push('\n');
    }
    let matched = terms
        .iter()
        .filter(|term| evidence.contains(&term.lower))
        .take(3)
        .map(|term| term.display.as_str())
        .collect::<Vec<_>>();
    if matched.is_empty() {
        "query-axis".to_string()
    } else {
        format!("query-axis:{}", matched.join("+"))
    }
}

fn selector_axis_coverage(
    lines: &[&[u8]],
    start: usize,
    end: usize,
    terms: &[QueryTerm],
    symbol: Option<&str>,
) -> usize {
    let mut evidence = String::new();
    if let Some(symbol) = symbol {
        evidence.push_str(&symbol.to_lowercase());
        evidence.push('\n');
    }
    let start_index = start.saturating_sub(1);
    let end_index = end.min(lines.len());
    for line in &lines[start_index..end_index] {
        evidence.push_str(&byte_text::lowercase_lossy_string(line));
        evidence.push('\n');
    }
    terms
        .iter()
        .filter(|term| evidence.contains(&term.lower))
        .count()
}

fn item_kind_priority(kind: &str) -> u8 {
    match kind {
        "interface" | "class" | "struct" | "enum" | "trait" => 0,
        "type" => 1,
        "function" | "fn" => 2,
        "const" | "static" => 3,
        "config" => 4,
        _ => 4,
    }
}

fn config_owner_query_match(
    language_id: &str,
    path: &Path,
    terms: &[QueryTerm],
    lines: &[&[u8]],
) -> Option<OwnerQueryMatch> {
    let file_spec = language_file_spec(language_id);
    if !file_spec.is_config_path(path) {
        return None;
    }
    let filename = path.file_name().and_then(|name| name.to_str())?;
    let filename_lower = filename.to_ascii_lowercase();
    if terms
        .iter()
        .any(|term| filename_lower.contains(&term.lower))
    {
        return Some(OwnerQueryMatch {
            start: 1,
            end: 1,
            kind: "config",
            term: filename.to_string(),
            match_rank: 0,
            axis_coverage: selector_axis_coverage(lines, 1, 1, terms, Some(filename)),
        });
    }
    for (index, line) in lines.iter().enumerate() {
        let lower = byte_text::lowercase_lossy_string(line);
        if let Some(term) = terms.iter().find(|term| lower.contains(&term.lower)) {
            return Some(OwnerQueryMatch {
                start: index + 1,
                end: index + 1,
                kind: "config",
                term: term.display.clone(),
                match_rank: 1,
                axis_coverage: selector_axis_coverage(lines, index + 1, index + 1, terms, None),
            });
        }
    }
    None
}

fn item_kind_for_line(path: &Path, line: &[u8]) -> Option<&'static str> {
    let lower = byte_text::lowercase_lossy_string(line);
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => rust_item_kind_for_line(&lower),
        Some("py") => python_item_kind_for_line(&lower),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs") => {
            typescript_item_kind_for_line(&lower)
        }
        Some("ss" | "ssi" | "scm" | "sld") => scheme_item_kind_for_line(&lower),
        _ => None,
    }
}

fn item_symbol_for_line(path: &Path, line: &[u8]) -> Option<String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("ss" | "ssi" | "scm" | "sld") => scheme_item_symbol_for_line(line),
        Some("rs") => rust_item_symbol_for_line(line),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs") => None,
        _ => None,
    }
}

fn rust_item_kind_for_line(line: &str) -> Option<&'static str> {
    if line.contains(" struct ") || line.trim_start().starts_with("struct ") {
        Some("struct")
    } else if line.contains(" enum ") || line.trim_start().starts_with("enum ") {
        Some("enum")
    } else if line.contains(" trait ") || line.trim_start().starts_with("trait ") {
        Some("trait")
    } else if line.contains(" type ") || line.trim_start().starts_with("type ") {
        Some("type")
    } else if line.contains(" mod ") || line.trim_start().starts_with("mod ") {
        Some("mod")
    } else if line.contains(" const ") || line.trim_start().starts_with("const ") {
        Some("const")
    } else if line.contains(" static ") || line.trim_start().starts_with("static ") {
        Some("static")
    } else if line.contains(" fn ") || line.trim_start().starts_with("fn ") {
        Some("fn")
    } else {
        None
    }
}

fn python_item_kind_for_line(line: &str) -> Option<&'static str> {
    if line.trim_start().starts_with("class ") {
        Some("class")
    } else if line.trim_start().starts_with("def ") || line.trim_start().starts_with("async def ") {
        Some("function")
    } else {
        None
    }
}

fn typescript_item_kind_for_line(line: &str) -> Option<&'static str> {
    let line = line.trim_start();
    if line.starts_with("export * ") || line.starts_with("export {") {
        return Some("export");
    }
    let declaration = line
        .strip_prefix("export ")
        .or_else(|| line.strip_prefix("declare "))
        .unwrap_or(line)
        .trim_start();
    if declaration.starts_with("default ") {
        Some("default-export")
    } else if declaration.starts_with("interface ") {
        Some("interface")
    } else if declaration.starts_with("class ") || declaration.starts_with("abstract class ") {
        Some("class")
    } else if declaration.starts_with("type ") {
        Some("type")
    } else if declaration.starts_with("const ") {
        Some("const")
    } else if declaration.starts_with("function ") || declaration.starts_with("async function ") {
        Some("function")
    } else {
        None
    }
}

fn typescript_barrel_owner_query_match(
    path: &Path,
    terms: &[QueryTerm],
    lines: &[&[u8]],
) -> Option<OwnerQueryMatch> {
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs")
    ) {
        return None;
    }
    let path_lower = path.to_string_lossy().to_lowercase();
    let term = terms.iter().find(|term| path_lower.contains(&term.lower))?;
    for (index, line) in lines.iter().enumerate() {
        let lower = byte_text::lowercase_lossy_string(line);
        if lower.trim_start().starts_with("export * ") || lower.trim_start().starts_with("export {")
        {
            let start = index + 1;
            let end = typescript_block_end(path, lines, index).unwrap_or(start);
            return Some(OwnerQueryMatch {
                start,
                end,
                kind: "export",
                term: term.display.clone(),
                match_rank: 2,
                axis_coverage: selector_axis_coverage(
                    lines,
                    start,
                    end,
                    terms,
                    path.file_name().and_then(|name| name.to_str()),
                ),
            });
        }
    }
    None
}

fn scheme_item_kind_for_line(line: &str) -> Option<&'static str> {
    let line = line.trim_start();
    if line.starts_with("(defstruct ") {
        Some("struct")
    } else if line.starts_with("(define-type ") {
        Some("type")
    } else if line.starts_with("(def ") || line.starts_with("(def* ") {
        Some("function")
    } else {
        None
    }
}

fn scheme_item_symbol_for_line(line: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(line);
    let line = line.trim_start();
    let rest = line
        .strip_prefix("(defstruct ")
        .or_else(|| line.strip_prefix("(define-type "))
        .or_else(|| line.strip_prefix("(def* "))
        .or_else(|| line.strip_prefix("(def "))?
        .trim_start();
    let rest = rest.strip_prefix('(').unwrap_or(rest);
    let symbol = rest
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')'))
        .next()
        .unwrap_or_default();
    if symbol.is_empty() {
        None
    } else {
        Some(symbol.to_string())
    }
}

fn rust_block_end(path: &Path, lines: &[&[u8]], start_index: usize) -> Option<usize> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return None;
    }
    let mut saw_open = false;
    let mut brace_depth = 0isize;
    for (line_index, line) in lines.iter().enumerate().skip(start_index) {
        for byte in *line {
            match byte {
                b'{' => {
                    saw_open = true;
                    brace_depth += 1;
                }
                b'}' if saw_open => {
                    brace_depth -= 1;
                }
                _ => {}
            }
        }
        if saw_open && brace_depth <= 0 {
            let end = line_index + 1;
            return Some(if end == start_index + 1 { end + 1 } else { end });
        }
    }
    None
}

fn python_block_end(path: &Path, lines: &[&[u8]], start_index: usize) -> Option<usize> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("py") {
        return None;
    }
    let base_indent = leading_spaces(lines.get(start_index)?);
    for (line_index, line) in lines.iter().enumerate().skip(start_index + 1) {
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        let indent = leading_spaces(line);
        if indent <= base_indent {
            return Some(line_index);
        }
    }
    Some(lines.len())
}

fn typescript_block_end(path: &Path, lines: &[&[u8]], start_index: usize) -> Option<usize> {
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs")
    ) {
        return None;
    }
    let mut saw_open = false;
    let mut brace_depth = 0isize;
    for (line_index, line) in lines.iter().enumerate().skip(start_index) {
        for byte in *line {
            match byte {
                b'{' => {
                    saw_open = true;
                    brace_depth += 1;
                }
                b'}' if saw_open => {
                    brace_depth -= 1;
                }
                b';' if !saw_open => return Some(line_index + 1),
                _ => {}
            }
        }
        if saw_open && brace_depth <= 0 {
            let end = line_index + 1;
            return Some(if end == start_index + 1 { end + 1 } else { end });
        }
    }
    Some(lines.len())
}

fn scheme_block_end(path: &Path, lines: &[&[u8]], start_index: usize) -> Option<usize> {
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ss" | "ssi" | "scm" | "sld")
    ) {
        return None;
    }
    let mut depth = 0isize;
    let mut saw_open = false;
    for (line_index, line) in lines.iter().enumerate().skip(start_index) {
        for byte in *line {
            match byte {
                b'(' => {
                    saw_open = true;
                    depth += 1;
                }
                b')' if saw_open => {
                    depth -= 1;
                }
                _ => {}
            }
        }
        if saw_open && depth <= 0 {
            return Some(line_index + 1);
        }
    }
    Some(lines.len())
}

fn leading_spaces(line: &[u8]) -> usize {
    line.iter().take_while(|byte| **byte == b' ').count()
}

struct QueryTerm {
    display: String,
    lower: String,
}

fn query_terms(query: &str) -> Vec<QueryTerm> {
    query
        .split([',', '|'])
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| QueryTerm {
            display: term.to_string(),
            lower: term.to_lowercase(),
        })
        .collect()
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn numbered_ids(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|index| numbered_id(prefix, index)).collect()
}

fn numbered_id(prefix: &str, index: usize) -> String {
    if index == 0 {
        prefix.to_string()
    } else {
        format!("{prefix}{}", index + 1)
    }
}
