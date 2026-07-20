use crate::evidence_graph::{ClientDbEvidenceGraphEdge, ClientDbEvidenceGraphNode};
use agent_semantic_content_identity::DerivedSourceArtifactEvidence;

/// Feature-gated EvidenceGraph entity row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbGraphEntity {
    pub id: String,
    pub kind: String,
    pub semantic_kind: Option<String>,
    pub label: String,
    pub selector: Option<String>,
    pub path: Option<String>,
    pub language_id: Option<String>,
    pub provider_id: Option<String>,
    pub query_keys: Vec<String>,
}

/// Feature-gated EvidenceGraph edge row written through the Turso adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// Persistence receipt for writing an EvidenceGraph projection into Turso.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbEvidenceGraphPersistReport {
    pub entity_count: usize,
    pub edge_count: usize,
    pub graph_artifact_digest: String,
}

/// One owner-local graph read model from a single snapshot-bound query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TursoClientDbGraphOwnerReadModel {
    /// Shared snapshot/artifact authority evidence for this graph read.
    pub artifact_evidence: DerivedSourceArtifactEvidence,
    /// Whether the current artifact contains the parser-owned owner node.
    pub owner_present: bool,
    /// Exact parser-owned selector nodes available to the owner-item route.
    pub selector_nodes: Vec<TursoClientDbGraphEntity>,
}

impl From<&ClientDbEvidenceGraphNode> for TursoClientDbGraphEntity {
    fn from(node: &ClientDbEvidenceGraphNode) -> Self {
        Self {
            id: node.id.clone(),
            kind: node.kind.to_string(),
            semantic_kind: node.semantic_kind.clone(),
            label: node.label.clone(),
            selector: node.selector.clone(),
            path: node.path.clone(),
            language_id: node.language_id.clone(),
            provider_id: node.provider_id.clone(),
            query_keys: node.query_keys.clone(),
        }
    }
}

impl From<&ClientDbEvidenceGraphEdge> for TursoClientDbGraphEdge {
    fn from(edge: &ClientDbEvidenceGraphEdge) -> Self {
        Self {
            from: edge.from.clone(),
            to: edge.to.clone(),
            kind: edge.kind.to_string(),
        }
    }
}
