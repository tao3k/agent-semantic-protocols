//! Selector-seeded search pipe rendering.

use std::fmt;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::graph::GraphTurboReceiptRequest;
use super::search_pipe_action_frontier::{ActionFrontierEntry, ActionNode, ActionRoute};
use super::search_pipe_graph_nodes::stable_node_id;

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

pub struct SelectorSeedCursorBinding {
    pub state_digest: String,
    pub graph_generation: u64,
    pub workspace_generation: String,
    pub remaining_budget: u64,
    pub route_budget: agent_semantic_loop::search_graph_cursor::GraphRouteBudget,
}

pub struct SelectorSeededSearchCursorRequest<'a> {
    pub search: SelectorSeededSearchPipeRequest<'a>,
    pub binding: SelectorSeedCursorBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectorSeedCursorError {
    ProjectionNodeIdentityMissing,
    CurrentNodeMissing,
    Cursor(agent_semantic_loop::search_graph_cursor::SearchGraphCursorError),
}

impl fmt::Display for SelectorSeedCursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectionNodeIdentityMissing => {
                formatter.write_str("selector graph projection contains a node without identity")
            }
            Self::CurrentNodeMissing => {
                formatter.write_str("selector graph projection has no cursor entry node")
            }
            Self::Cursor(error) => write!(formatter, "selector graph cursor is invalid: {error}"),
        }
    }
}

impl std::error::Error for SelectorSeedCursorError {}

impl From<agent_semantic_loop::search_graph_cursor::SearchGraphCursorError>
    for SelectorSeedCursorError
{
    fn from(error: agent_semantic_loop::search_graph_cursor::SearchGraphCursorError) -> Self {
        Self::Cursor(error)
    }
}

#[derive(Clone, Debug)]
struct SelectorSeedTopology {
    language_id: String,
    selector_seed: String,
    query: String,
    workspace: String,
    nodes: Vec<Value>,
    edges: Vec<Value>,
    actions: Vec<ActionNode>,
    action_frontier: Vec<ActionFrontierEntry>,
}

pub fn render_selector_seeded_search_pipe(request: SelectorSeededSearchPipeRequest<'_>) -> String {
    let topology = selector_seed_topology(request);
    render_selector_seed_topology(&topology)
}

pub fn selector_seeded_search_cursor(
    request: SelectorSeededSearchCursorRequest<'_>,
) -> Result<agent_semantic_loop::search_graph_cursor::SearchGraphCursor, SelectorSeedCursorError> {
    let topology = selector_seed_topology(request.search);
    let materialized_node_ids = topology
        .nodes
        .iter()
        .map(|node| {
            node["id"]
                .as_str()
                .map(str::to_string)
                .ok_or(SelectorSeedCursorError::ProjectionNodeIdentityMissing)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let current_node = ["query", "selector-input", "item", "provider-root"]
        .into_iter()
        .find_map(|kind| topology_node_field(&topology, kind, "id"))
        .map(str::to_string)
        .ok_or(SelectorSeedCursorError::CurrentNodeMissing)?;
    let frontier = topology_node_ids(&topology, Some("action"));
    let visited = topology
        .nodes
        .iter()
        .filter(|node| node["kind"] != "action")
        .filter_map(|node| node["id"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let state = agent_semantic_loop::search_graph_cursor::InteractiveSearchGraphState::new(
        agent_semantic_loop::search_graph_cursor::InteractiveSearchGraphStateRequest {
            state_digest: request.binding.state_digest,
            graph_generation: request.binding.graph_generation,
            workspace_generation: request.binding.workspace_generation,
            current_node,
            remaining_budget: request.binding.remaining_budget,
            obligations: Vec::new(),
            frontier,
            visited,
            route_measure: agent_semantic_loop::search_graph_cursor::GraphRouteMeasure::zero(),
            route_budget: request.binding.route_budget,
        },
    )?;
    agent_semantic_loop::search_graph_cursor::SearchGraphCursor::bootstrap(
        agent_semantic_loop::search_graph_cursor::SearchGraphCursorBootstrapRequest {
            state,
            materialized_node_ids,
        },
    )
    .map_err(Into::into)
}

fn render_selector_seed_topology(topology: &SelectorSeedTopology) -> String {
    let owner = topology_node_field(topology, "owner", "value").unwrap_or("-");
    let symbol = topology_node_field(topology, "item", "symbol").unwrap_or("-");
    let materialized_frontier = topology
        .actions
        .iter()
        .zip(topology.action_frontier.iter())
        .filter(|(action, entry)| topology_materializes_action(topology, action, entry))
        .collect::<Vec<_>>();
    let mut output = String::new();
    output.push_str(&format!(
        "[search-pipe] lang={} view=seeds source=selector ranker=selector-seed\n",
        topology.language_id
    ));
    output.push_str(&format!("query={}\n", topology.query));
    output.push_str(&format!("selectorSeed={}\n", topology.selector_seed));
    output.push_str(&format!("ownerSeed={owner}\n"));
    output.push_str(&format!("symbolSeed={symbol}\n"));
    output.push_str(
        "seedPlan=selector-query alg=asp-search-pipe-selector-v0 budget=frontier<=3 repeated=0\n",
    );
    output.push_str(&render_action_frontier_line(&materialized_frontier));
    output.push_str(&render_recommended_next_line(&materialized_frontier));
    output.push_str(&render_materialized_next_command_line(
        &materialized_frontier,
    ));
    output.push_str("nextClasses=query-selector,owner-items\n");
    output.push_str("avoid=shell-and,manual-command-join,repeat-search-pipe,raw-read\n");
    output.push_str(&format!(
        "sourceTrace=selectorSeed:used[owner={owner};symbol={symbol};workspace={}]\n",
        topology.workspace
    ));
    output
}

fn render_action_frontier_line(actions: &[(&ActionNode, &ActionFrontierEntry)]) -> String {
    let frontier = actions
        .iter()
        .map(|(_, entry)| format!("{}.{}", entry.id, entry.kind))
        .collect::<Vec<_>>()
        .join(",");
    format!("actionFrontier={frontier}\n")
}

fn render_recommended_next_line(actions: &[(&ActionNode, &ActionFrontierEntry)]) -> String {
    actions
        .first()
        .map(|(_, entry)| format!("recommendedNext={}.{}\n", entry.id, entry.kind))
        .unwrap_or_else(|| "recommendedNext=-\n".to_string())
}

fn render_materialized_next_command_line(
    actions: &[(&ActionNode, &ActionFrontierEntry)],
) -> String {
    actions
        .iter()
        .find_map(|(action, _)| action.materialized_command())
        .map(|command| format!("nextCommand={command}\n"))
        .unwrap_or_else(|| "nextCommand=-\n".to_string())
}

fn topology_node_field<'a>(
    topology: &'a SelectorSeedTopology,
    kind: &str,
    field: &str,
) -> Option<&'a str> {
    topology
        .nodes
        .iter()
        .find(|node| node["kind"] == kind)
        .and_then(|node| node[field].as_str())
}

fn topology_node_ids(topology: &SelectorSeedTopology, kind: Option<&str>) -> Vec<String> {
    topology
        .nodes
        .iter()
        .filter(|node| kind.is_none_or(|kind| node["kind"] == kind))
        .filter_map(|node| node["id"].as_str().map(str::to_string))
        .collect()
}

fn topology_materializes_action(
    topology: &SelectorSeedTopology,
    action: &ActionNode,
    entry: &ActionFrontierEntry,
) -> bool {
    let action_node_id = action_graph_node_id(action, entry);
    topology
        .edges
        .iter()
        .any(|edge| edge["source"] == action_node_id && edge["relation"] == "materializes")
}

fn selector_seed_topology(request: SelectorSeededSearchPipeRequest<'_>) -> SelectorSeedTopology {
    let agent_semantic_search::GraphSelectorSeedProjection {
        canonical_selector,
        owner,
        symbol,
        selector_node_id,
        owner_node_id,
        mut nodes,
        mut edges,
        ..
    } = agent_semantic_search::graph_selector_seed_projection(
        agent_semantic_search::GraphSelectorSeedProjectionRequest {
            language_id: request.language_id,
            selector: request.selector,
            query: request.query,
            workspace: request.workspace,
        },
    );
    let actions = selector_seed_actions(
        canonical_selector.as_ref(),
        owner.as_deref().unwrap_or("-"),
        symbol.as_deref().unwrap_or("-"),
        request.query,
        request.workspace,
    );
    let action_frontier = actions
        .iter()
        .map(ActionNode::frontier_entry)
        .collect::<Vec<_>>();
    for (action, entry) in actions.iter().zip(action_frontier.iter()) {
        let action_node_id = action_graph_node_id(action, entry);
        nodes.push(json!({
            "id": action_node_id,
            "kind": "action",
            "role": entry.kind,
            "value": entry.target,
            "action": entry.capability_id,
            "fields": {
                "frontierId": entry.id,
                "targetRole": entry.target_role,
                "suffix": action.suffix,
            },
        }));
        let target_node_id = match entry.target_role.as_str() {
            "selector" => selector_node_id.as_ref(),
            "owner" => owner_node_id.as_ref(),
            _ => None,
        };
        if let Some(target_node_id) = target_node_id {
            edges.push(graph_edge(&action_node_id, target_node_id, "materializes"));
        }
    }
    SelectorSeedTopology {
        language_id: request.language_id.to_string(),
        selector_seed: request.selector.to_string(),
        query: request.query.to_string(),
        workspace: request.workspace.to_string(),
        nodes,
        edges,
        actions,
        action_frontier,
    }
}

fn action_graph_node_id(action: &ActionNode, entry: &ActionFrontierEntry) -> String {
    stable_node_id(
        "action",
        &format!("{}:{}:{}", entry.kind, action.suffix, entry.target),
    )
}

fn graph_edge(source: &str, target: &str, relation: &str) -> Value {
    json!({
        "source": source,
        "target": target,
        "relation": relation,
    })
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

#[cfg(test)]
#[path = "../../tests/unit/search_pipe_selector_seed.rs"]
mod tests;
