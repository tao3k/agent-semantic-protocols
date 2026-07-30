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
    let canonical_selector =
        agent_semantic_content_identity::CanonicalItemSelector::parse(selector)
            .ok()
            .filter(|selector| selector.language_id.as_str() == language_id);
    let owner = canonical_selector
        .as_ref()
        .map(selector_owner)
        .unwrap_or("-");
    let symbol = canonical_selector
        .as_ref()
        .map(selector_symbol)
        .unwrap_or("-");
    let actions =
        selector_seed_actions(canonical_selector.as_ref(), owner, symbol, query, workspace);
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
    output.push_str("nextClasses=query-selector,owner-items\n");
    output.push_str("avoid=shell-and,manual-command-join,repeat-search-pipe,raw-read\n");
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
    selector: Option<&agent_semantic_content_identity::CanonicalItemSelector>,
    owner: &str,
    symbol: &str,
    query: &str,
    workspace: &str,
) -> Vec<ActionNode> {
    let mut actions = Vec::new();
    if let Some(selector) = selector {
        let language_id = selector.language_id.as_str();
        actions.push(ActionNode {
            id: "A1".to_string(),
            kind: "query-projection".to_string(),
            suffix: "selector-seed".to_string(),
            route: ActionRoute::QueryProjection {
                language_id: language_id.to_string(),
                selector: selector.clone(),
                owner: owner.to_string(),
                symbol: symbol.to_string(),
                workspace: workspace.to_string(),
            },
        });
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
    }
    actions
        .into_iter()
        .enumerate()
        .map(|(index, mut action)| {
            action.id = format!("A{}", index + 1);
            action
        })
        .collect()
}

fn selector_owner(selector: &agent_semantic_content_identity::CanonicalItemSelector) -> &str {
    selector
        .structural_selector()
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('#'))
        .map(|(owner, _)| owner)
        .expect("validated canonical selector has an owner")
}

fn selector_symbol(selector: &agent_semantic_content_identity::CanonicalItemSelector) -> &str {
    selector.symbol.as_str()
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
