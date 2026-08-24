//! Shared graph projection for an exact-selector SearchLoop entry.

use serde_json::{Value, json};

use crate::stable_graph_node_id;

#[derive(Clone, Copy, Debug)]
pub struct GraphSelectorSeedProjectionRequest<'a> {
    pub language_id: &'a str,
    pub selector: &'a str,
    pub query: &'a str,
    pub workspace: &'a str,
}

#[derive(Clone, Debug)]
pub struct GraphSelectorSeedProjection {
    pub canonical_selector: Option<agent_semantic_content_identity::CanonicalItemSelector>,
    pub owner: Option<String>,
    pub symbol: Option<String>,
    pub language_node_id: String,
    pub selector_node_id: Option<String>,
    pub owner_node_id: Option<String>,
    pub query_node_id: Option<String>,
    pub nodes: Vec<Value>,
    pub edges: Vec<Value>,
}

pub fn graph_selector_seed_projection(
    request: GraphSelectorSeedProjectionRequest<'_>,
) -> GraphSelectorSeedProjection {
    let identity = selector_seed_identity(&request);
    let nodes = selector_seed_nodes(&request, &identity);
    let edges = selector_seed_edges(&identity);
    GraphSelectorSeedProjection {
        canonical_selector: identity.canonical_selector,
        owner: identity.owner,
        symbol: identity.symbol,
        language_node_id: identity.language_node_id,
        selector_node_id: identity.selector_node_id,
        owner_node_id: identity.owner_node_id,
        query_node_id: identity.query_node_id,
        nodes,
        edges,
    }
}

struct GraphSelectorSeedIdentity {
    canonical_selector: Option<agent_semantic_content_identity::CanonicalItemSelector>,
    owner: Option<String>,
    symbol: Option<String>,
    language_node_id: String,
    selector_node_id: Option<String>,
    owner_node_id: Option<String>,
    query_node_id: Option<String>,
    unresolved_selector_node_id: Option<String>,
}

fn selector_seed_identity(
    request: &GraphSelectorSeedProjectionRequest<'_>,
) -> GraphSelectorSeedIdentity {
    let canonical_selector =
        agent_semantic_content_identity::CanonicalItemSelector::parse(request.selector)
            .ok()
            .filter(|selector| selector.language_id.as_str() == request.language_id);
    let owner = canonical_selector.as_ref().map(|selector| {
        selector
            .structural_selector()
            .split_once("://")
            .and_then(|(_, rest)| rest.split_once('#'))
            .map(|(owner, _)| owner.to_string())
            .expect("validated canonical selector has an owner")
    });
    let symbol = canonical_selector
        .as_ref()
        .map(|selector| selector.symbol.as_str().to_string());
    let language_node_id = stable_graph_node_id("provider-root", request.language_id);
    let selector_node_id = canonical_selector
        .as_ref()
        .map(|selector| stable_graph_node_id("item", selector.structural_selector()));
    let owner_node_id = owner
        .as_deref()
        .map(|owner| stable_graph_node_id("owner", owner));
    let query_node_id =
        (!request.query.trim().is_empty()).then(|| stable_graph_node_id("query", request.query));
    let unresolved_selector_node_id = canonical_selector
        .is_none()
        .then(|| stable_graph_node_id("selector-input", request.selector));
    GraphSelectorSeedIdentity {
        canonical_selector,
        owner,
        symbol,
        language_node_id,
        selector_node_id,
        owner_node_id,
        query_node_id,
        unresolved_selector_node_id,
    }
}

fn selector_seed_nodes(
    request: &GraphSelectorSeedProjectionRequest<'_>,
    identity: &GraphSelectorSeedIdentity,
) -> Vec<Value> {
    let mut nodes = vec![json!({
        "id": identity.language_node_id,
        "kind": "provider-root",
        "role": "language",
        "value": request.language_id,
        "languageId": request.language_id,
        "fields": { "workspace": request.workspace },
    })];
    if let Some(node_id) = identity.unresolved_selector_node_id.as_ref() {
        nodes.push(json!({
            "id": node_id,
            "kind": "selector-input",
            "role": "unresolved",
            "value": request.selector,
            "fields": {
                "languageId": request.language_id,
                "reasonKind": "selector-invalid-or-language-mismatch",
            },
        }));
    }
    if let Some(query_node_id) = identity.query_node_id.as_ref() {
        nodes.push(json!({
            "id": query_node_id,
            "kind": "query",
            "role": "intent",
            "value": request.query,
        }));
    }
    if let (Some(owner), Some(owner_node_id)) =
        (identity.owner.as_deref(), identity.owner_node_id.as_ref())
    {
        nodes.push(json!({
            "id": owner_node_id,
            "kind": "owner",
            "role": "source-owner",
            "value": owner,
            "path": owner,
            "ownerPath": owner,
            "languageId": request.language_id,
        }));
    }
    if let (Some(selector), Some(selector_node_id)) = (
        identity.canonical_selector.as_ref(),
        identity.selector_node_id.as_ref(),
    ) {
        nodes.push(json!({
            "id": selector_node_id,
            "kind": "item",
            "role": "selector",
            "value": selector.structural_selector(),
            "ownerPath": identity.owner.as_deref(),
            "symbol": identity.symbol.as_deref(),
            "structuralSelector": selector.structural_selector(),
            "projection": selector_projection_kind(request.language_id),
            "requiresExact": true,
            "languageId": request.language_id,
        }));
    }
    nodes
}

fn selector_seed_edges(identity: &GraphSelectorSeedIdentity) -> Vec<Value> {
    let mut edges = Vec::new();
    if let Some(owner_node_id) = identity.owner_node_id.as_ref() {
        edges.push(graph_edge(
            &identity.language_node_id,
            owner_node_id,
            "serves_owner",
        ));
    }
    if let (Some(owner_node_id), Some(selector_node_id)) = (
        identity.owner_node_id.as_ref(),
        identity.selector_node_id.as_ref(),
    ) {
        edges.push(graph_edge(owner_node_id, selector_node_id, "owns_item"));
    }
    if let (Some(query_node_id), Some(selector_node_id)) = (
        identity.query_node_id.as_ref(),
        identity.selector_node_id.as_ref(),
    ) {
        edges.push(graph_edge(query_node_id, selector_node_id, "targets_item"));
    } else if let (Some(query_node_id), Some(unresolved_selector_node_id)) = (
        identity.query_node_id.as_ref(),
        identity.unresolved_selector_node_id.as_ref(),
    ) {
        edges.push(graph_edge(
            query_node_id,
            unresolved_selector_node_id,
            "targets_unresolved_selector",
        ));
    }
    edges
}

fn selector_projection_kind(language_id: &str) -> &'static str {
    if matches!(language_id, "markdown" | "md" | "org" | "text") {
        "content"
    } else {
        "source"
    }
}

fn graph_edge(source: &str, target: &str, relation: &str) -> Value {
    json!({
        "source": source,
        "target": target,
        "relation": relation,
    })
}

#[cfg(test)]
#[path = "../tests/unit/graph_selector_seed_projection.rs"]
mod tests;
