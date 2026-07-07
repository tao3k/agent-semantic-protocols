use std::io::{self, Write};
use std::path::Path;

use super::item::OwnerItem;
use super::request::{OwnerItemQuery, OwnerQueryRequest};

pub(super) fn format_code_matches(source: &str, matches: &[&OwnerItem]) -> String {
    let mut rendered = String::new();
    for (index, item) in matches.iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }
        rendered.push_str(&select_line_range(source, item.start_line, item.end_line));
    }
    rendered
}

pub(super) fn render_empty_code_match_error(
    request: &OwnerQueryRequest,
    item_query: &OwnerItemQuery,
    path: &Path,
    project_root: &Path,
    locator_root: &Path,
    same_name_kinds: &[&str],
) -> Result<(), String> {
    let display_path = display_relative_path(path, project_root, locator_root)
        .to_string_lossy()
        .replace('\\', "/");
    let selector_kind = item_query.kind().unwrap_or("any");
    let (state, reason, actual_kinds) = if same_name_kinds.is_empty() {
        ("not-found", "item-not-found", String::new())
    } else {
        (
            "kind-mismatch",
            "item-kind-mismatch",
            format!(" actualKinds={}", same_name_kinds.join(",")),
        )
    };
    Err(format!(
        "exact selector matched no owner item: ownerPath={display_path} itemQuery={} selectorKind={selector_kind} state={state} reason={reason}{actual_kinds} recommendedNext=asp {} search owner {display_path} items --query {} --workspace . --view seeds",
        item_query.term(),
        request.language_id,
        item_query.term()
    ))
}

pub(super) fn write_owner_query_stdout(rendered: &str) -> Result<(), String> {
    io::stdout()
        .write_all(rendered.as_bytes())
        .map_err(|error| format!("failed to write owner query stdout: {error}"))
}

pub(super) fn format_full_source(source: &str) -> String {
    source.to_string()
}

pub(super) fn format_locator_matches(
    request: &OwnerQueryRequest,
    item_query: &OwnerItemQuery,
    path: &Path,
    project_root: &Path,
    locator_root: &Path,
    line_count: usize,
    matches: &[&OwnerItem],
) -> String {
    let output = if request.names_only {
        "names"
    } else {
        "locator"
    };
    let display_path = display_relative_path(path, project_root, locator_root);
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "[search-owner] q={} pkg=. own=1 item={} itemQuery={} output={output}\n",
        display_path.display(),
        matches.len(),
        item_query.term()
    ));
    rendered.push_str(&format!(
        "|owner {} role=source source=asp-syn-owner lines={line_count}\n",
        display_path.display()
    ));
    for item in matches {
        let structural_selector = owner_item_structural_selector(request, display_path, item);
        rendered.push_str(&format!(
                "|item name={} kind={} owner={} structuralSelector={} displayLineRange={}:{} sourceLocatorHint={}:{}:{} syn=node:{} projection={} codePolicy=code-after-exact-selector\n",
                item.name,
            item.kind,
            display_path.display(),
            structural_selector,
            item.start_line,
            item.end_line,
            display_path.display(),
            item.start_line,
            item.end_line,
            item.syntax_node,
                item_query.projection()
            ));
    }
    let search_frame = search_frame_owner_items_receipt(request, item_query, display_path, matches);
    if matches.is_empty() {
        rendered.push_str(&format!(
                "|query itemQuery={} status=miss match=none item=0 reason=asp-syn-owner-query output={output} next=revise-query{search_frame}\n",
                item_query.term(),
            ));
    } else {
        rendered.push_str(&format!(
                "|query itemQuery={} status=hit match=exact item={} reason=asp-syn-owner-query output={output} next=query --code codePolicy=requires-exact-code{search_frame}\n",
                item_query.term(),
                matches.len()
            ));
    }
    rendered
}

pub(super) fn format_non_source_owner_query(
    request: &OwnerQueryRequest,
    item_query: &OwnerItemQuery,
    path: &Path,
    project_root: &Path,
    locator_root: &Path,
    source: &str,
) -> Result<String, String> {
    if item_query.is_code_projection() {
        if source_contains_owner_term(source, item_query.term()) {
            Ok(format_full_source(source))
        } else {
            Ok(format_code_matches(source, &[]))
        }
    } else {
        Ok(format_locator_matches(
            request,
            item_query,
            path,
            project_root,
            locator_root,
            source.lines().count(),
            &[],
        ))
    }
}

fn source_contains_owner_term(source: &str, term: &str) -> bool {
    source
        .split(|character: char| !is_owner_term_character(character))
        .any(|token| token == term)
}

fn is_owner_term_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn owner_item_structural_selector(
    request: &OwnerQueryRequest,
    display_path: &Path,
    item: &OwnerItem,
) -> String {
    format!(
        "{}://{}#item/{}/{}",
        request.language_id,
        display_path.display(),
        item.kind.replace(char::is_whitespace, "-"),
        item.name.replace(char::is_whitespace, "-")
    )
}

fn search_frame_owner_items_receipt(
    request: &OwnerQueryRequest,
    item_query: &OwnerItemQuery,
    display_path: &Path,
    matches: &[&OwnerItem],
) -> String {
    let source_trace = format!(
        "asp-syn-owner:{}:items={}",
        display_path.display(),
        matches.len()
    );
    if let Some(item) = matches.first() {
        let selector = owner_item_structural_selector(request, display_path, item);
        let projection_flag = if item_query.projection() == "content" {
            "--content"
        } else {
            "--code"
        };
        let next_command = format!(
            "asp {} query --selector {} --workspace . {}",
            request.language_id, selector, projection_flag
        );
        let where_frame = format!("owner:{}#item/{}", display_path.display(), item.name);
        return format!(
            " nextCommand={} recommendedNext=query-exact-selector actionFrontier=query-exact-selector,revise-query sourceTrace={} avoid=inline-code-in-search,raw-read,repeat-owner whereFrame={} howFrame=exact-selector-read",
            quote_search_frame_value(&next_command),
            quote_search_frame_value(&source_trace),
            quote_search_frame_value(&where_frame)
        );
    }

    let next_command = format!(
        "asp {} search owner {} items --query {} --workspace . --view seeds",
        request.language_id,
        display_path.display(),
        item_query.term()
    );
    let where_frame = format!("owner:{}", display_path.display());
    format!(
        " nextCommand={} recommendedNext=revise-query actionFrontier=revise-query,search-owner sourceTrace={} avoid=inline-code-in-search,raw-read,repeat-owner whereFrame={} howFrame=revise-query",
        quote_search_frame_value(&next_command),
        quote_search_frame_value(&source_trace),
        quote_search_frame_value(&where_frame)
    )
}

fn unresolved_owner_search_frame_receipt(
    request: &OwnerQueryRequest,
    item_query: &OwnerItemQuery,
    display_path: &str,
) -> String {
    let next_command = format!(
        "asp {} search owner {} items --query {} --workspace . --view seeds",
        request.language_id,
        display_path,
        item_query.term()
    );
    let source_trace = format!("owner-not-found:{display_path}");
    let where_frame = format!("owner:{display_path}");
    format!(
        " nextCommand={} recommendedNext=search-owner actionFrontier=search-owner,revise-owner sourceTrace={} avoid=inline-code-in-search,raw-read,repeat-owner whereFrame={} howFrame=resolve-owner",
        quote_search_frame_value(&next_command),
        quote_search_frame_value(&source_trace),
        quote_search_frame_value(&where_frame)
    )
}

fn quote_search_frame_value(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn format_unresolved_owner_query(request: &OwnerQueryRequest) -> Result<String, String> {
    let Some(item_query) = request.item_query() else {
        return Ok(String::new());
    };
    if item_query.is_code_projection() {
        let display_path = request.owner_path.to_string_lossy().replace('\\', "/");
        return Err(format!(
            "stale-index exact selector resolved no code payload: ownerPath={display_path} itemQuery={} state=stale-index recommendedNext=asp {} search owner {display_path} items --workspace . --view seeds",
            item_query.term(),
            request.language_id
        ));
    }
    let output = if request.names_only {
        "names"
    } else {
        "locator"
    };
    let display_path = request.owner_path.to_string_lossy().replace('\\', "/");
    let search_frame = unresolved_owner_search_frame_receipt(request, item_query, &display_path);
    let rendered = format!(
        "[search-owner] q={display_path} pkg=. own=0 item=0 itemQuery={} output={output}\n|query itemQuery={} status=miss match=none item=0 reason=owner-not-found output={output} next=search-owner{search_frame}\n",
        item_query.term(),
        item_query.term()
    );
    Ok(rendered)
}

fn select_line_range(source: &str, start: usize, end: usize) -> String {
    source
        .split_inclusive('\n')
        .skip(start.saturating_sub(1))
        .take(end.saturating_sub(start).saturating_add(1))
        .collect()
}

fn display_relative_path<'a>(
    path: &'a Path,
    project_root: &'a Path,
    locator_root: &'a Path,
) -> &'a Path {
    path.strip_prefix(locator_root)
        .or_else(|_| path.strip_prefix(project_root))
        .unwrap_or(path)
}
