//! Gerbil Scheme exact-owner owner-items fast path.

use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct GerbilOwnerItem {
    name: String,
    kind: &'static str,
    start_line: usize,
    end_line: usize,
    language_kind: &'static str,
}

pub(super) fn run_inline_gerbil_owner_items_query(
    language_id: &str,
    owner: &Path,
    query: &str,
    project_root: &Path,
) -> Result<bool, String> {
    if language_id != "gerbil-scheme" {
        return Ok(false);
    }
    let Some((owner_path, owner_display)) = inline_gerbil_owner_path(project_root, owner) else {
        return Ok(false);
    };
    let source = fs::read_to_string(&owner_path)
        .map_err(|error| format!("failed to read {}: {error}", owner_path.display()))?;
    let items = collect_gerbil_owner_items(&source);
    print!(
        "{}",
        render_inline_gerbil_owner_items(&owner_display, query, &items)
    );
    Ok(true)
}

fn inline_gerbil_owner_path(project_root: &Path, owner: &Path) -> Option<(PathBuf, String)> {
    let owner_path = if owner.is_absolute() {
        normalize_path(owner)
    } else {
        normalize_path(&project_root.join(owner))
    };
    if !is_gerbil_owner_path(&owner_path) || !owner_path.is_file() {
        return None;
    }
    let root = project_root.canonicalize().ok()?;
    let owner_path = owner_path.canonicalize().ok()?;
    let owner_display = owner_path
        .strip_prefix(&root)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| owner_path.to_string_lossy().to_string());
    Some((owner_path, owner_display))
}

fn is_gerbil_owner_path(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|name| name.to_str());
    if matches!(file_name, Some("gerbil.pkg" | "build.ss")) {
        return true;
    }
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ss" | "ssi" | "scm" | "sld")
    )
}

pub(crate) fn collect_gerbil_owner_items(source: &str) -> Vec<GerbilOwnerItem> {
    let mut items = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let trimmed = line.trim_start();
        if trimmed.starts_with("(package:") {
            items.push(GerbilOwnerItem {
                name: "gerbil.pkg".to_string(),
                kind: "package",
                start_line: line_no,
                end_line: line_no,
                language_kind: "package-form",
            });
        }
        if let Some((head, name)) = gerbil_definition_item(trimmed) {
            items.push(GerbilOwnerItem {
                name,
                kind: definition_kind(head),
                start_line: line_no,
                end_line: line_no,
                language_kind: "definition",
            });
        }
        if trimmed.starts_with("(export") {
            for name in symbols_after_head(trimmed).into_iter().take(12) {
                items.push(GerbilOwnerItem {
                    name,
                    kind: "export",
                    start_line: line_no,
                    end_line: line_no,
                    language_kind: "module-export",
                });
            }
        }
        for name in call_heads(trimmed) {
            items.push(GerbilOwnerItem {
                name,
                kind: "call",
                start_line: line_no,
                end_line: line_no,
                language_kind: "call",
            });
        }
    }
    dedupe_items(items)
}

fn gerbil_definition_item(line: &str) -> Option<(&str, String)> {
    let rest = line.strip_prefix('(')?;
    let (head, after_head) = next_symbol(rest)?;
    if !is_definition_head(head) {
        return None;
    }
    let after_head = after_head.trim_start();
    if let Some(signature) = after_head.strip_prefix('(') {
        let (name, _) = next_symbol(signature)?;
        return Some((head, name.to_string()));
    }
    let (name, _) = next_symbol(after_head)?;
    Some((head, name.to_string()))
}

fn is_definition_head(head: &str) -> bool {
    matches!(
        head,
        "def"
            | "def*"
            | "define"
            | "define-values"
            | "define-syntax"
            | "defstruct"
            | "defclass"
            | ".defclass"
            | "defmethod"
            | ".defmethod"
            | "defgeneric"
            | ".defgeneric"
            | "defrules"
            | "defrule"
            | "defsyntax"
            | ".def"
    )
}

fn definition_kind(head: &str) -> &'static str {
    match head {
        "defclass" | ".defclass" => "class",
        "defstruct" => "struct",
        "defmethod" | ".defmethod" => "method",
        "defgeneric" | ".defgeneric" => "generic",
        ".def" => "poo-object",
        _ => "def",
    }
}

fn call_heads(line: &str) -> Vec<String> {
    let mut calls = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'(' {
            index += 1;
            continue;
        }
        let after = &line[index + 1..];
        if let Some((head, _)) = next_symbol(after)
            && !head.is_empty()
            && !head.starts_with(';')
        {
            calls.push(head.to_string());
        }
        index += 1;
    }
    calls
}

fn symbols_after_head(line: &str) -> Vec<String> {
    let Some(rest) = line.strip_prefix('(') else {
        return Vec::new();
    };
    let Some((_, mut remaining)) = next_symbol(rest) else {
        return Vec::new();
    };
    let mut symbols = Vec::new();
    loop {
        remaining = remaining.trim_start_matches(|character: char| {
            character.is_whitespace() || matches!(character, '(' | ')' | '\'' | '`' | ',')
        });
        let Some((symbol, next_remaining)) = next_symbol(remaining) else {
            break;
        };
        symbols.push(symbol.to_string());
        remaining = next_remaining;
    }
    symbols
}

fn next_symbol(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start();
    let end = input
        .find(|character: char| {
            character.is_whitespace() || matches!(character, '(' | ')' | '[' | ']' | '"' | ';')
        })
        .unwrap_or(input.len());
    if end == 0 {
        return None;
    }
    Some((&input[..end], &input[end..]))
}

fn dedupe_items(items: Vec<GerbilOwnerItem>) -> Vec<GerbilOwnerItem> {
    let mut seen = std::collections::BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert((item.name.clone(), item.start_line)))
        .collect()
}

pub(crate) fn render_inline_gerbil_owner_items(
    owner: &str,
    query: &str,
    items: &[GerbilOwnerItem],
) -> String {
    let query_terms = owner_items_query_terms(query);
    let mut matched_items = items
        .iter()
        .filter(|item| gerbil_owner_item_matches(item, &query_terms))
        .collect::<Vec<_>>();
    matched_items.sort_by(|left, right| {
        owner_item_kind_rank(left.kind)
            .cmp(&owner_item_kind_rank(right.kind))
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.end_line.cmp(&right.end_line))
            .then_with(|| left.name.cmp(&right.name))
    });
    let match_count = matched_items.len();
    let shown_items = matched_items.into_iter().take(80).collect::<Vec<_>>();
    let mut output = String::new();
    output.push_str(&format!(
        "[gerbil-owner-items] path={} matches={} shown={} limit=80 reason=rust-inline-gerbil-owner-items\n",
        owner,
        match_count,
        shown_items.len(),
    ));
    for (index, item) in shown_items.iter().enumerate() {
        let structural_selector = format!(
            "gerbil-scheme://{}#item/{}/{}",
            owner,
            item.kind,
            item.name.replace(char::is_whitespace, "-")
        );
        output.push_str(&format!(
            "{}=item:symbol({})@{}!syntax\n",
            item_alias(index),
            item.name,
            structural_selector,
        ));
    }
    for item in &shown_items {
        let source_locator_hint = format!("{}:{}:{}", owner, item.start_line, item.end_line);
        let structural_selector = format!(
            "gerbil-scheme://{}#item/{}/{}",
            owner,
            item.kind,
            item.name.replace(char::is_whitespace, "-")
        );
        output.push_str(&format!(
            "|item kind={} name={} structuralSelector={} displayLineRange={}:{} sourceLocatorHint={} source=rust-inline languageKind={} projection=outline codePolicy=code-after-exact-selector\n",
            item.kind,
            item.name,
            structural_selector,
            item.start_line,
            item.end_line,
            source_locator_hint,
            item.language_kind
        ));
    }
    if let Some(first) = shown_items.first() {
        output.push_str(&format!(
            "nextCommand=asp gerbil-scheme query --selector {}:{}:{} --workspace . --code\n",
            owner, first.start_line, first.end_line
        ));
    }
    output.push_str("reason=owner-item-selector-ready\n");
    output.push_str("|note kind=runtime-prefilter message=owner-items-rust-inline-gerbil\n");
    output
}

fn owner_item_kind_rank(kind: &str) -> u8 {
    match kind {
        "package" | "def" | "class" | "struct" | "method" | "generic" | "poo-object" => 0,
        "export" => 1,
        "call" => 2,
        _ => 3,
    }
}

fn item_alias(index: usize) -> String {
    if index == 0 {
        "I".to_string()
    } else {
        format!("I{}", index + 1)
    }
}

fn owner_items_query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == '|' || character.is_whitespace() || character == ',')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn gerbil_owner_item_matches(item: &GerbilOwnerItem, query_terms: &[String]) -> bool {
    if query_terms.is_empty() {
        return false;
    }
    let name = item.name.to_ascii_lowercase();
    query_terms.iter().any(|term| {
        if term.len() < 3 {
            name == *term
        } else {
            name.contains(term.as_str())
        }
    })
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
