use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;

use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;
use agent_semantic_search_projection::ResidentSearchHit;

use crate::SearchGenerationGraphRequest;

/// Inputs admitted by one immutable Runtime search generation.
pub struct ResidentGraphSearchRequest<'a> {
    pub operation_id: &'a str,
    pub operation: &'a str,
    pub query: &'a str,
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub generation_digest: &'a str,
    pub source_snapshot: &'a SourceSnapshotEvidence,
    pub workspace_generation: &'a WorkspaceGenerationEvidenceV1,
    pub lexical_hits: &'a [ResidentSearchHit],
    /// Immutable whole-generation graph admitted with the resident mmap.
    /// Query-specific lexical hits are represented only by `entryNodeIds`.
    pub generation_graph: &'a ResidentGraphGeneration,
}

/// Compact graph stage evidence retained by the v1 Runtime search receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphSearchStage {
    pub generation_digest: String,
    pub result_digest: String,
    pub ranked_owner_paths: Vec<String>,
    pub elapsed_micros: u64,
    pub work: ResidentGraphSearchWork,
}

/// Explicit upper bounds for one warm resident graph traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentGraphSearchBudget {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_frontier: usize,
    pub max_results: usize,
}

/// Intent-only graph evaluation admitted against one immutable generation.
pub struct ResidentGraphEvaluationRequest<'a> {
    pub operation_id: &'a str,
    pub generation_digest: &'a str,
    pub source_snapshot: &'a SourceSnapshotEvidence,
    pub workspace_generation: &'a WorkspaceGenerationEvidenceV1,
    pub entry_node_ids: &'a [String],
    pub generation_graph: &'a ResidentGraphGeneration,
}

/// Explicit upper bounds for a public resident graph evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidentGraphEvaluationBudget {
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_results: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphRankedNode {
    pub id: String,
    pub kind: String,
    pub owner_path: Option<String>,
    pub score: usize,
    pub distance: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResidentGraphEvaluatedEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentGraphEvaluation {
    pub ranked_nodes: Vec<ResidentGraphRankedNode>,
    pub edges: Vec<ResidentGraphEvaluatedEdge>,
    pub work: ResidentGraphSearchWork,
}

/// Machine-readable work counters for the Ready-path graph stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentGraphSearchWork {
    pub visited_nodes: usize,
    pub visited_edges: usize,
    pub frontier_peak: usize,
    pub provider_rpc_count: usize,
    pub cache_hit_count: usize,
    pub cache_miss_count: usize,
    pub cache_entry_count: usize,
    pub cache_value_bytes: usize,
    pub cache_capacity: usize,
    pub cache_shard_count: usize,
}

/// Measured construction phases for one immutable graph attachment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResidentGraphBuildMetrics {
    pub owner_projection_nanos: u64,
    pub relation_projection_nanos: u64,
    pub identity_hash_nanos: u64,
}

/// Canonical whole-generation graph shared by all warm queries.
#[derive(Clone, Debug)]
pub struct ResidentGraphGeneration {
    pub(super) generation_request: Option<Arc<SearchGenerationGraphRequest>>,
    pub(super) digest: String,
    pub(super) source_root_digest: String,
    pub(super) leaf_count: u64,
    pub(super) owner_count: u64,
    pub(super) owner_node_ids: Arc<BTreeSet<String>>,
    pub(super) owner_paths_by_node_id: Arc<BTreeMap<String, String>>,
    pub(super) nodes_by_id: Arc<BTreeMap<String, ResidentGraphNode>>,
    pub(super) adjacency: Arc<BTreeMap<String, Vec<ResidentGraphEvaluatedEdge>>>,
    pub(super) rank_cache: Arc<Vec<Mutex<Vec<(String, Arc<ResidentGraphSearchStage>)>>>>,
    pub(super) build_metrics: ResidentGraphBuildMetrics,
}

#[derive(Clone, Debug)]
pub(super) struct ResidentGraphNode {
    pub(super) kind: String,
    pub(super) owner_path: Option<String>,
}

impl ResidentGraphGeneration {
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes_by_id.len()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(Vec::len).sum()
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub fn build_metrics(&self) -> ResidentGraphBuildMetrics {
        self.build_metrics
    }

    /// Return the exact provider-fact request that produced this generation.
    ///
    /// Published generations always retain this immutable request so an
    /// optional graph worker can be started lazily without rereading source,
    /// invoking a provider, or inventing a second generation identity.
    pub fn generation_request(&self) -> Result<&SearchGenerationGraphRequest, String> {
        self.generation_request
            .as_deref()
            .ok_or_else(|| "resident graph lacks its generation request authority".to_owned())
    }

    pub(super) fn matches_generation_binding(
        &self,
        source_snapshot: &SourceSnapshotEvidence,
        workspace_generation: &WorkspaceGenerationEvidenceV1,
    ) -> bool {
        self.source_root_digest == source_snapshot.root_digest
            && self.source_root_digest == workspace_generation.root_digest
            && self.leaf_count == workspace_generation.leaf_count
            && self.owner_count == workspace_generation.owner_count
    }
}
