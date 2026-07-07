//! Selector-seeded search pipe rendering.

use std::path::{Path, PathBuf};

use super::graph::GraphTurboReceiptRequest;
use super::search_pipe_action_frontier::{ActionNode, ActionRoute, render_next_command_line};

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
    let output = render_selector_seeded_search_pipe(SelectorSeededSearchPipeRequest {
        language_id: request.language_id,
        selector: request.selector,
        query: request.query,
        workspace: &workspace,
    });
    print!("{output}");
    Ok(())
}

/// Render a selector-seeded `search pipe` frontier without running providers.
#[derive(Clone, Copy, Debug)]
pub struct SelectorSeededSearchPipeRequest<'a> {
    pub language_id: &'a str,
    pub selector: &'a str,
    pub query: &'a str,
    pub workspace: &'a str,
}

pub fn render_selector_seeded_search_pipe(request: SelectorSeededSearchPipeRequest<'_>) -> String {
    let SelectorSeededSearchPipeRequest {
        language_id,
        selector,
        query,
        workspace,
    } = request;
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
    output.push_str(&render_action_frontier_line(&actions));
    output.push_str(&render_recommended_next_line(&actions));
    output.push_str(&render_next_command_line(&actions));
    output.push_str("nextClasses=query-selector,owner-items,rg-query\n");
    output.push_str(
        "avoid=shell-and,manual-command-join,repeat-search-pipe,raw-read,direct-source-read\n",
    );
    output.push_str(&format!(
        "sourceTrace=selectorSeed:used[owner={owner};symbol={symbol};workspace={workspace}]\n"
    ));
    output
}

fn render_action_frontier_line(actions: &[ActionNode]) -> String {
    let frontier = actions
        .iter()
        .map(|action| format!("{}.{}", action.id, action.kind))
        .collect::<Vec<_>>()
        .join(",");
    format!("actionFrontier={frontier}\n")
}

fn render_recommended_next_line(actions: &[ActionNode]) -> String {
    actions
        .first()
        .map(|action| format!("recommendedNext={}.{}\n", action.id, action.kind))
        .unwrap_or_else(|| "recommendedNext=-\n".to_string())
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
    let mut actions = Vec::new();
    if is_executable_structural_selector(selector) {
        actions.push(ActionNode {
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
        });
    }
    actions.push(ActionNode {
        id: String::new(),
        kind: "owner-items".to_string(),
        suffix: "selector-owner-items".to_string(),
        route: ActionRoute::OwnerItems {
            language_id: language_id.to_string(),
            owner: owner.to_string(),
            query: query.to_string(),
            scope: workspace.to_string(),
        },
    });
    actions.push(ActionNode {
        id: String::new(),
        kind: "rg-query".to_string(),
        suffix: "selector-context".to_string(),
        route: ActionRoute::RgQuery {
            query: query.to_string(),
            scope: workspace.to_string(),
            command_scope: Some(owner.to_string()),
        },
    });
    actions
        .into_iter()
        .enumerate()
        .map(|(index, mut action)| {
            action.id = format!("A{}", index + 1);
            action
        })
        .collect()
}

fn is_executable_structural_selector(selector: &str) -> bool {
    let Some(kind) = structural_selector_item_kind(selector) else {
        return false;
    };
    matches!(
        kind,
        "const"
            | "enum"
            | "field"
            | "fn"
            | "function"
            | "impl"
            | "macro"
            | "method"
            | "mod"
            | "module"
            | "static"
            | "struct"
            | "trait"
            | "type"
    )
}

fn structural_selector_item_kind(selector: &str) -> Option<&str> {
    let (_, item) = selector.split_once("#item/")?;
    let (kind, _) = item.split_once('/')?;
    (!kind.is_empty()).then_some(kind)
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
