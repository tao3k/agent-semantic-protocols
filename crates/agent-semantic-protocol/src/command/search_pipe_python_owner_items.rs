//! Python exact-owner owner-items fast path.

use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
struct PythonOwnerItem {
    name: String,
    kind: &'static str,
    start_line: usize,
    end_line: usize,
    public: bool,
    syn: &'static str,
}

pub(super) fn run_inline_python_owner_items_query(
    language_id: &str,
    owner: &Path,
    query: &str,
    project_root: &Path,
) -> Result<bool, String> {
    if language_id != "python" {
        return Ok(false);
    }
    let Some((owner_path, owner_display)) = inline_python_owner_path(project_root, owner) else {
        return Ok(false);
    };
    let source = fs::read_to_string(&owner_path)
        .map_err(|error| format!("failed to read {}: {error}", owner_path.display()))?;
    let items = collect_python_owner_items(&source);
    print!(
        "{}",
        render_inline_python_owner_items(
            &owner_display,
            query,
            source.lines().count(),
            python_module_has_docstring(&source),
            &items,
        )
    );
    Ok(true)
}

fn inline_python_owner_path(project_root: &Path, owner: &Path) -> Option<(PathBuf, String)> {
    let owner_path = if owner.is_absolute() {
        normalize_path(owner)
    } else {
        normalize_path(&project_root.join(owner))
    };
    if owner_path
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("py")
        || !owner_path.is_file()
    {
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

fn collect_python_owner_items(source: &str) -> Vec<PythonOwnerItem> {
    let starts = source
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            if line
                .chars()
                .next()
                .is_some_and(|character| character.is_whitespace())
            {
                return None;
            }
            let trimmed = line.trim_start();
            let (kind, prefix, syn) = if trimmed.starts_with("async def ") {
                ("function", "async def ", "function_definition/name")
            } else if trimmed.starts_with("def ") {
                ("function", "def ", "function_definition/name")
            } else if trimmed.starts_with("class ") {
                ("class", "class ", "class_definition/name")
            } else {
                return None;
            };
            python_definition_name(trimmed, prefix).map(|name| (line_index + 1, name, kind, syn))
        })
        .collect::<Vec<_>>();

    starts
        .iter()
        .enumerate()
        .map(|(index, (start_line, name, kind, syn))| {
            let next_start = starts
                .get(index + 1)
                .map(|(line, _, _, _)| line.saturating_sub(1))
                .unwrap_or_else(|| source.lines().count().max(*start_line));
            PythonOwnerItem {
                name: name.clone(),
                kind,
                start_line: *start_line,
                end_line: trim_python_item_end(source, *start_line, next_start),
                public: !name.starts_with('_'),
                syn,
            }
        })
        .collect()
}

fn python_definition_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name = rest
        .chars()
        .take_while(|character| *character == '_' || character.is_ascii_alphanumeric())
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn trim_python_item_end(source: &str, start_line: usize, proposed_end: usize) -> usize {
    let lines = source.lines().collect::<Vec<_>>();
    let mut end_line = proposed_end.min(lines.len()).max(start_line);
    while end_line > start_line
        && lines
            .get(end_line - 1)
            .is_some_and(|line| line.trim().is_empty())
    {
        end_line -= 1;
    }
    end_line
}

fn python_module_has_docstring(source: &str) -> bool {
    source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .is_some_and(|line| line.starts_with("\"\"\"") || line.starts_with("'''"))
}

fn render_inline_python_owner_items(
    owner: &str,
    query: &str,
    line_count: usize,
    has_docstring: bool,
    items: &[PythonOwnerItem],
) -> String {
    let query_terms = owner_items_query_tokens(query);
    let matched_items = items
        .iter()
        .filter(|item| python_owner_item_matches(item, &query_terms))
        .collect::<Vec<_>>();
    let selected_items = matched_items;
    let item_status = if selected_items.is_empty() {
        "miss"
    } else {
        "hit"
    };
    let item_match = if item_status == "hit" {
        selected_items
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    } else {
        "none".to_string()
    };
    let public_exports = items
        .iter()
        .filter(|item| item.public)
        .take(6)
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let next = selected_items
        .iter()
        .take(3)
        .map(|item| python_item_structural_selector(owner, item))
        .collect::<Vec<_>>()
        .join(",");
    let mut output = String::new();
    output.push_str(&format!(
        "[search-owner] q={} owner=1 edge=0 item={} itemQuery=\"{}\" itemStatus={} itemMatch={} selection=item-query\n",
        owner,
        selected_items.len(),
        compact_field(query),
        item_status,
        item_match,
    ));
    output.push_str(&format!(
        "|query itemQuery=\"{}\" status={} match={} item={} reason=rust-inline-python-owner-items output=names next={}\n",
        compact_field(query),
        item_status,
        item_match,
        selected_items.len(),
        if item_status == "hit" { "item-skeleton" } else { "revise-query" },
    ));
    output.push_str(&format!(
        "|owner {} role=\"root,module\" public=true exp={} kind=module surface=source doc={} lines={} exportKind=inferred next={}\n",
        owner,
        if public_exports.is_empty() { "-" } else { public_exports.as_str() },
        has_docstring,
        line_count,
        if next.is_empty() { "-" } else { next.as_str() },
    ));
    for item in &selected_items {
        let selector = python_item_structural_selector(owner, item);
        output.push_str(&format!(
            "|item {} kind={}{} structuralSelector={} displayLineRange={}:{} sourceLocatorHint={}:{}:{} syn={} tsqRef=semantic-tree-sitter-query/python-owner-items.v1 projection=skeleton codePolicy=code-after-exact-selector\n",
            item.name,
            item.kind,
            if item.public { " public=true" } else { "" },
            selector,
            item.start_line,
            item.end_line,
            owner,
            item.start_line,
            item.end_line,
            item.syn,
        ));
    }
    output.push_str(&format!(
        "|hit path={} kind=path score=4 reason=owner-match\n",
        owner
    ));
    output.push_str("|note kind=runtime-prefilter message=owner-items-rust-inline-python\n");
    output.push_str(&format!(
        "|runtime paths=1 ownerPath={} reason=owner-items-rust-inline-python\n",
        owner
    ));
    if let Some(first_item) = selected_items.first() {
        let selector = python_item_structural_selector(owner, first_item);
        let hint = format!(
            "{}:{}:{}",
            owner, first_item.start_line, first_item.end_line
        );
        output.push_str("[route-graph] profile=asp-search-routing evidence=known-owner+symbol chosen=KNOWN_OWNER reason=\"owner and symbol evidence matched parser item; inspect skeleton before code\" frontier=A1.item-skeleton,A2.syntax-outline,A3.query-code avoid=search-prime|direct-source-read|line-range-selector\n");
        output.push_str(&format!(
            "A1=item-skeleton(selector={},projection=skeleton,hint={})!skeleton\n",
            selector, hint
        ));
        output.push_str(&format!(
            "A2=syntax-outline(selector={},projection=outline,hint={})!syntax\n",
            selector, hint
        ));
        output.push_str(&format!(
            "A3=query-code(selector={},requiresExact=true,codePolicy=exact-only,hint={})!query-code\n",
            selector, hint
        ));
        output.push_str("actionFrontier=A1.item-skeleton,A2.syntax-outline,A3.query-code\n");
        output.push_str("recommendedNext=A1.item-skeleton\n");
        output.push_str(&format!(
            "nextCommand=asp python query --from-hook item-skeleton --selector '{}' --workspace . --names-only\n",
            selector
        ));
        output.push_str("reason=owner-item-skeleton-ready\n");
        output.push_str("avoid=selector-code-before-exact,direct-source-read,manual-window-scan\n");
    } else if !next.is_empty() {
        output.push_str(&format!("|next {next}\n"));
    }
    output
}

fn python_item_structural_selector(owner: &str, item: &PythonOwnerItem) -> String {
    format!("python://{}#item/{}/{}", owner, item.kind, item.name)
}

fn owner_items_query_tokens(query: &str) -> Vec<String> {
    query
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .map(str::trim)
        .filter(|term| term.len() >= 2)
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn python_owner_item_matches(item: &PythonOwnerItem, query_terms: &[String]) -> bool {
    if query_terms.is_empty() {
        return false;
    }
    let name = item.name.to_ascii_lowercase();
    query_terms
        .iter()
        .any(|term| name.contains(term.as_str()) || term.contains(name.as_str()))
}

fn compact_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "'")
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
