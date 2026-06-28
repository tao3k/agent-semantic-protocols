//! Selector-seeded search pipe rendering.

use std::path::{Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::search_pipe_action_frontier::{ActionNode, ActionRoute, render_action_rows};

pub(super) struct SelectorSeedSearchPipeRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) selector: &'a str,
    pub(super) query: &'a str,
    pub(super) workspace: Option<&'a Path>,
    pub(super) scopes: &'a [PathBuf],
    pub(super) view: &'a str,
    pub(super) frontier_receipt: Option<&'a GraphTurboReceiptRequest>,
}

pub(super) fn print_selector_seeded_search_pipe(
    request: SelectorSeedSearchPipeRequest<'_>,
) -> Result<(), String> {
    reject_unsupported_view(request.view, request.frontier_receipt)?;
    let workspace = command_workspace(
        request.project_root,
        request.locator_root,
        request.workspace,
        request.scopes,
    );
    let output = render_selector_seeded_search_pipe(
        request.language_id,
        request.selector,
        request.query,
        &workspace,
    );
    print!("{output}");
    Ok(())
}

pub fn render_selector_seeded_search_pipe(
    language_id: &str,
    selector: &str,
    query: &str,
    workspace: &str,
) -> String {
    let owner = selector_owner(selector);
    let symbol = selector_symbol(selector).unwrap_or("-");
    let actions = selector_seed_actions(language_id, selector, &owner, symbol, query, workspace);
    let mut output = String::new();
    output.push_str(&format!(
        "[search-pipe] lang={} view=seeds source=selector ranker=selector-seed\n",
        language_id
    ));
    output.push_str(&format!("query={}\n", query));
    output.push_str(&format!("selectorSeed={}\n", selector));
    output.push_str(&format!("ownerSeed={owner}\n"));
    output.push_str(&format!("symbolSeed={symbol}\n"));
    output.push_str(
        "seedPlan=selector-query alg=asp-search-pipe-selector-v0 budget=frontier<=3 repeated=0\n",
    );
    output.push_str(&format!(
        "commandHandles=querySelector={};ownerItems={owner}:{};rgQuery={}\n",
        shell_arg(selector),
        compact_query_handle(query),
        compact_query_handle(query)
    ));
    output.push_str(&render_action_rows(&actions));
    output.push_str("nextClasses=query-selector,owner-items,rg-query\n");
    output.push_str(
        "avoid=shell-and,manual-command-join,repeat-search-pipe,raw-read,direct-source-read\n",
    );
    output.push_str(&format!(
        "sourceTrace=selectorSeed:used[owner={owner};symbol={symbol};workspace={workspace}]\n"
    ));
    output
}

fn reject_unsupported_view(
    view: &str,
    frontier_receipt: Option<&GraphTurboReceiptRequest>,
) -> Result<(), String> {
    if frontier_receipt.is_some() {
        return Err(
            "--frontier-receipt-out is not supported for selector-seeded search pipe".to_string(),
        );
    }
    if view == "seeds" {
        return Ok(());
    }
    Err("search pipe --selector supports --view seeds".to_string())
}

fn selector_seed_actions(
    language_id: &str,
    selector: &str,
    owner: &str,
    symbol: &str,
    query: &str,
    workspace: &str,
) -> Vec<ActionNode> {
    vec![
        ActionNode {
            id: "A1".to_string(),
            kind: "query-code".to_string(),
            suffix: "selector-seed".to_string(),
            route: ActionRoute::QueryCode {
                language_id: language_id.to_string(),
                selector: selector.to_string(),
                owner: owner.to_string(),
                symbol: symbol.to_string(),
                workspace: workspace.to_string(),
            },
        },
        ActionNode {
            id: "A2".to_string(),
            kind: "owner-items".to_string(),
            suffix: "selector-owner-items".to_string(),
            route: ActionRoute::OwnerItems {
                language_id: language_id.to_string(),
                owner: owner.to_string(),
                query: query.to_string(),
                scope: workspace.to_string(),
            },
        },
        ActionNode {
            id: "A3".to_string(),
            kind: "rg-query".to_string(),
            suffix: "selector-context".to_string(),
            route: ActionRoute::RgQuery {
                query: query.to_string(),
                scope: workspace.to_string(),
                command_scope: Some(owner.to_string()),
            },
        },
    ]
}

fn selector_owner(selector: &str) -> String {
    let without_scheme = selector
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(selector);
    let without_fragment = without_scheme
        .split_once('#')
        .map(|(owner, _)| owner)
        .unwrap_or(without_scheme);
    strip_line_range(without_fragment)
}

fn strip_line_range(value: &str) -> String {
    let Some((path, range)) = value.rsplit_once(':') else {
        return value.to_string();
    };
    let mut parts = range.split('-');
    let Some(start) = parts.next() else {
        return value.to_string();
    };
    let Some(end) = parts.next() else {
        return value.to_string();
    };
    if parts.next().is_none()
        && !start.is_empty()
        && !end.is_empty()
        && start.chars().all(|character| character.is_ascii_digit())
        && end.chars().all(|character| character.is_ascii_digit())
    {
        path.to_string()
    } else {
        value.to_string()
    }
}

fn selector_symbol(selector: &str) -> Option<&str> {
    selector
        .split_once('#')
        .map(|(_, fragment)| fragment)
        .and_then(|fragment| fragment.rsplit('/').find(|part| !part.is_empty()))
}

fn command_workspace(
    project_root: &Path,
    locator_root: &Path,
    workspace: Option<&Path>,
    scopes: &[PathBuf],
) -> String {
    workspace
        .or_else(|| scopes.first().map(PathBuf::as_path))
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| display_project_root(project_root, locator_root))
}

fn display_project_root(project_root: &Path, locator_root: &Path) -> String {
    if project_root == locator_root {
        return ".".to_string();
    }
    if project_root.as_os_str().is_empty() {
        ".".to_string()
    } else {
        project_root.display().to_string()
    }
}

fn compact_query_handle(query: &str) -> String {
    query
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join("|")
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
