//! DB-owned EvidenceGraph read-model projection.

use std::path::PathBuf;

use serde::Serialize;

use crate::{
    ClientDbSourceIndexImport, ClientDbStructuralDependencyUsage, ClientDbStructuralIndexImport,
    ClientDbStructuralSymbol,
};

pub const CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID: &str =
    "agent.semantic-protocols.evidence-graph-read-model";
pub const CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEvidenceGraph {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub generation_id: String,
    pub project_root: PathBuf,
    pub nodes: Vec<ClientDbEvidenceGraphNode>,
    pub edges: Vec<ClientDbEvidenceGraphEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEvidenceGraphNode {
    pub id: String,
    pub kind: &'static str,
    pub semantic_kind: Option<String>,
    pub label: String,
    pub path: Option<String>,
    pub selector: Option<String>,
    pub query_keys: Vec<String>,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDbEvidenceGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: &'static str,
}

#[must_use]
pub fn source_index_evidence_graph(import: &ClientDbSourceIndexImport) -> ClientDbEvidenceGraph {
    let mut graph =
        empty_evidence_graph(import.generation_id.as_str(), import.project_root.clone());
    for owner in &import.owners {
        let owner_path = owner.owner_path.as_str();
        let owner_id = source_owner_node_id(import.generation_id.as_str(), owner_path);
        graph.nodes.push(ClientDbEvidenceGraphNode {
            id: owner_id,
            kind: "source-owner",
            semantic_kind: None,
            label: owner_path.to_string(),
            path: Some(owner_path.to_string()),
            selector: None,
            query_keys: owner
                .query_keys
                .iter()
                .map(|key| key.as_str().to_string())
                .collect(),
            language_id: owner
                .language_id
                .as_ref()
                .map(|language_id| language_id.as_str().to_string()),
            provider_id: owner
                .provider_id
                .as_ref()
                .map(|provider_id| provider_id.as_str().to_string()),
        });
    }
    for selector in &import.selectors {
        let owner_path = selector.owner_path.as_str();
        let selector_id = source_selector_node_id(&selector.selector_id);
        graph.nodes.push(ClientDbEvidenceGraphNode {
            id: selector_id.clone(),
            kind: "selector",
            semantic_kind: selector.kind.clone(),
            label: selector
                .symbol
                .as_deref()
                .unwrap_or(selector.selector_id.as_str())
                .to_string(),
            path: Some(owner_path.to_string()),
            selector: Some(selector.selector_id.clone()),
            query_keys: selector
                .query_keys
                .iter()
                .map(|key| key.as_str().to_string())
                .collect(),
            language_id: None,
            provider_id: None,
        });
        graph.edges.push(ClientDbEvidenceGraphEdge {
            from: source_owner_node_id(import.generation_id.as_str(), owner_path),
            to: selector_id,
            kind: "contains-selector",
        });
    }
    graph
}

#[must_use]
pub fn structural_index_evidence_graph(
    import: &ClientDbStructuralIndexImport,
) -> ClientDbEvidenceGraph {
    let mut graph =
        empty_evidence_graph(import.generation_id.as_str(), import.project_root.clone());
    for owner in &import.owners {
        let owner_path = owner.owner_path.as_str();
        graph.nodes.push(ClientDbEvidenceGraphNode {
            id: structural_owner_node_id(import.generation_id.as_str(), owner_path),
            kind: "structural-owner",
            semantic_kind: None,
            label: owner_path.to_string(),
            path: Some(owner_path.to_string()),
            selector: None,
            query_keys: owner
                .query_keys
                .iter()
                .map(|key| key.as_str().to_string())
                .collect(),
            language_id: Some(import.language_id.as_str().to_string()),
            provider_id: Some(import.provider_id.as_str().to_string()),
        });
    }
    for symbol in &import.symbols {
        project_structural_symbol(import, symbol, &mut graph);
    }
    for dependency in &import.dependency_usages {
        project_structural_dependency(import, dependency, &mut graph);
    }
    graph
}

fn project_structural_symbol(
    import: &ClientDbStructuralIndexImport,
    symbol: &ClientDbStructuralSymbol,
    graph: &mut ClientDbEvidenceGraph,
) {
    let owner_path = symbol.owner_path.as_str();
    let symbol_id = structural_symbol_node_id(
        import.generation_id.as_str(),
        owner_path,
        symbol.name.as_str(),
        symbol.kind.as_str(),
    );
    graph.nodes.push(ClientDbEvidenceGraphNode {
        id: symbol_id.clone(),
        kind: "symbol",
        semantic_kind: Some(symbol.kind.as_str().to_string()),
        label: symbol.name.as_str().to_string(),
        path: Some(owner_path.to_string()),
        selector: symbol
            .source_locator
            .as_ref()
            .map(|locator| locator.as_str().to_string()),
        query_keys: symbol
            .query_keys
            .iter()
            .map(|key| key.as_str().to_string())
            .collect(),
        language_id: Some(import.language_id.as_str().to_string()),
        provider_id: Some(import.provider_id.as_str().to_string()),
    });
    graph.edges.push(ClientDbEvidenceGraphEdge {
        from: structural_owner_node_id(import.generation_id.as_str(), owner_path),
        to: symbol_id,
        kind: "defines-symbol",
    });
}

fn project_structural_dependency(
    import: &ClientDbStructuralIndexImport,
    dependency: &ClientDbStructuralDependencyUsage,
    graph: &mut ClientDbEvidenceGraph,
) {
    let owner_path = dependency.owner_path.as_str();
    let dependency_label = dependency
        .api_name
        .as_ref()
        .map(|api_name| {
            format!(
                "{}::{}",
                dependency.package_name.as_str(),
                api_name.as_str()
            )
        })
        .unwrap_or_else(|| dependency.package_name.as_str().to_string());
    let dependency_id =
        structural_dependency_node_id(import.generation_id.as_str(), owner_path, &dependency_label);
    graph.nodes.push(ClientDbEvidenceGraphNode {
        id: dependency_id.clone(),
        kind: "dependency-usage",
        semantic_kind: None,
        label: dependency_label,
        path: Some(owner_path.to_string()),
        selector: dependency
            .source_locator
            .as_ref()
            .map(|locator| locator.as_str().to_string()),
        query_keys: dependency
            .query_keys
            .iter()
            .map(|key| key.as_str().to_string())
            .collect(),
        language_id: Some(import.language_id.as_str().to_string()),
        provider_id: Some(import.provider_id.as_str().to_string()),
    });
    graph.edges.push(ClientDbEvidenceGraphEdge {
        from: structural_owner_node_id(import.generation_id.as_str(), owner_path),
        to: dependency_id,
        kind: "uses-dependency",
    });
}

pub(crate) fn empty_evidence_graph(
    generation_id: &str,
    project_root: PathBuf,
) -> ClientDbEvidenceGraph {
    ClientDbEvidenceGraph {
        schema_id: CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_ID,
        schema_version: CLIENT_DB_EVIDENCE_GRAPH_SCHEMA_VERSION,
        generation_id: generation_id.to_string(),
        project_root,
        nodes: Vec::new(),
        edges: Vec::new(),
    }
}

fn source_owner_node_id(generation_id: &str, owner_path: &str) -> String {
    format!("source-owner:{generation_id}:{owner_path}")
}

fn source_selector_node_id(selector_id: &str) -> String {
    format!("selector:{selector_id}")
}

fn structural_owner_node_id(generation_id: &str, owner_path: &str) -> String {
    format!("structural-owner:{generation_id}:{owner_path}")
}

fn structural_symbol_node_id(
    generation_id: &str,
    owner_path: &str,
    symbol_name: &str,
    symbol_kind: &str,
) -> String {
    format!("symbol:{generation_id}:{owner_path}:{symbol_kind}:{symbol_name}")
}

fn structural_dependency_node_id(
    generation_id: &str,
    owner_path: &str,
    dependency_label: &str,
) -> String {
    format!("dependency:{generation_id}:{owner_path}:{dependency_label}")
}
