use agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelector;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchSourceLeaf {
    pub owner_path: String,
    pub owner_content_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchGeneration {
    pub generation_id: String,
    pub root_digest: String,
    pub root_depth: usize,
    pub leaf_count: usize,
    pub owner_count: usize,
    pub selector_count: usize,
    pub language_id: String,
    pub provider_id: String,
    pub parser_identity_digest: String,
    pub query_pack_digest: String,
    pub source_leaves: Vec<MemorySearchSourceLeaf>,
}

impl MemorySearchGeneration {
    /// Memory Search is the resident, high-change projection of an active generation.
    ///
    /// `rootDepth` identifies the projection mode, not the physical height of a
    /// Merkle tree. Resident Memory Search is therefore always depth zero.
    pub fn expected_root_depth(_leaf_count: usize) -> usize {
        0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchItem {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub canonical_item_selector: CanonicalItemSelector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySearchRequest {
    pub expected_generation_id: String,
    pub requested_owner_path: String,
    pub canonical_item_selector: CanonicalItemSelector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemorySearchResolutionState {
    LiveHit,
    LiveRelocated,
    Ambiguous,
    KindMismatch,
    Missing,
    GenerationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchGenerationReceipt {
    pub generation_id: String,
    pub root_digest: String,
    pub root_depth: usize,
    pub leaf_count: usize,
    pub owner_count: usize,
    pub selector_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchPerformanceReceipt {
    pub generation_load_micros: u128,
    pub index_lookup_micros: u128,
    pub candidate_count: usize,
    pub source_bytes_materialized: usize,
    pub db_opens: usize,
    pub db_queries: usize,
    pub provider_subprocesses: usize,
    pub cache_writes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySearchResolution {
    pub state: MemorySearchResolutionState,
    pub resolved: Option<MemorySearchItem>,
    pub candidates: Vec<MemorySearchItem>,
    pub actual_kinds: Vec<String>,
    pub generation: MemorySearchGenerationReceipt,
    pub performance: MemorySearchPerformanceReceipt,
}
