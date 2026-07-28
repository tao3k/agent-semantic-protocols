use agent_semantic_content_identity::canonical_item_identity::CanonicalItemSelectorV1;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchSourceLeafV1 {
    pub owner_path: String,
    pub owner_content_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchGenerationV1 {
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
    pub source_leaves: Vec<MemorySearchSourceLeafV1>,
}

impl MemorySearchGenerationV1 {
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
pub struct MemorySearchItemV1 {
    pub owner_path: String,
    pub owner_content_digest: String,
    pub canonical_item_selector: CanonicalItemSelectorV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySearchRequestV1 {
    pub expected_generation_id: String,
    pub requested_owner_path: String,
    pub canonical_item_selector: CanonicalItemSelectorV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemorySearchResolutionStateV1 {
    LiveHit,
    LiveRelocated,
    Ambiguous,
    KindMismatch,
    Missing,
    GenerationMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchGenerationReceiptV1 {
    pub generation_id: String,
    pub root_digest: String,
    pub root_depth: usize,
    pub leaf_count: usize,
    pub owner_count: usize,
    pub selector_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchPerformanceReceiptV1 {
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
pub struct MemorySearchResolutionV1 {
    pub state: MemorySearchResolutionStateV1,
    pub resolved: Option<MemorySearchItemV1>,
    pub candidates: Vec<MemorySearchItemV1>,
    pub actual_kinds: Vec<String>,
    pub generation: MemorySearchGenerationReceiptV1,
    pub performance: MemorySearchPerformanceReceiptV1,
}

#[derive(Clone, Debug)]
pub struct MemorySearchIndexV1 {
    generation: MemorySearchGenerationV1,
    items: Vec<MemorySearchItemV1>,
    exact: BTreeMap<MemorySearchIdentityKeyV1, Vec<usize>>,
    symbol: BTreeMap<MemorySearchIdentityKeyV1, Vec<usize>>,
    load_micros: u128,
}

pub trait MemorySearchBackendV1 {
    fn load_generation(&self) -> Result<MemorySearchIndexV1, String>;
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MemorySearchIdentityKeyV1 {
    language_id: String,
    kind: Option<String>,
    symbol: String,
    scopes: Vec<u8>,
}

fn identity_key(
    selector: &CanonicalItemSelectorV1,
    include_kind: bool,
) -> MemorySearchIdentityKeyV1 {
    MemorySearchIdentityKeyV1 {
        language_id: selector.language_id.as_str().to_owned(),
        kind: include_kind.then(|| selector.kind.as_str().to_owned()),
        symbol: selector.symbol.as_str().to_owned(),
        scopes: serde_json::to_vec(&selector.scopes).expect("canonical item scopes"),
    }
}

impl MemorySearchIndexV1 {
    pub fn build(
        generation: MemorySearchGenerationV1,
        items: Vec<MemorySearchItemV1>,
    ) -> Result<Self, String> {
        let started = Instant::now();
        let expected_depth = MemorySearchGenerationV1::expected_root_depth(generation.leaf_count);
        let source_leaves = generation
            .source_leaves
            .iter()
            .map(|leaf| (leaf.owner_path.as_str(), leaf.owner_content_digest.as_str()))
            .collect::<std::collections::BTreeMap<_, _>>();
        if source_leaves.len() != generation.source_leaves.len()
            || generation.leaf_count != generation.source_leaves.len()
            || generation.owner_count != generation.leaf_count
            || generation.selector_count != items.len()
            || generation.root_depth != expected_depth
            || generation.root_digest.len() != 64
            || generation.parser_identity_digest.len() != 64
            || generation.query_pack_digest.len() != 64
        {
            return Err("memory-search generation evidence mismatch".to_string());
        }
        let materialized_snapshot =
            agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                source_leaves
                    .iter()
                    .map(|(owner_path, content_digest)| (*owner_path, *content_digest)),
            );
        if materialized_snapshot.root_digest() != generation.root_digest {
            return Err("memory-search generation root mismatch".to_string());
        }
        let mut exact = BTreeMap::<MemorySearchIdentityKeyV1, Vec<usize>>::new();
        let mut symbol = BTreeMap::<MemorySearchIdentityKeyV1, Vec<usize>>::new();
        for (index, item) in items.iter().enumerate() {
            item.canonical_item_selector
                .validate()
                .map_err(|error| format!("invalid memory-search item identity: {error}"))?;
            if item.canonical_item_selector.language_id.as_str() != generation.language_id
                || item.owner_path.is_empty()
                || item.owner_content_digest.len() != 64
                || source_leaves.get(item.owner_path.as_str())
                    != Some(&item.owner_content_digest.as_str())
            {
                return Err("memory-search item evidence mismatch".to_string());
            }
            exact
                .entry(identity_key(&item.canonical_item_selector, true))
                .or_default()
                .push(index);
            symbol
                .entry(identity_key(&item.canonical_item_selector, false))
                .or_default()
                .push(index);
        }
        Ok(Self {
            generation,
            items,
            exact,
            symbol,
            load_micros: started.elapsed().as_micros(),
        })
    }

    pub fn resolve(&self, request: &MemorySearchRequestV1) -> MemorySearchResolutionV1 {
        let started = Instant::now();
        if request.expected_generation_id != self.generation.generation_id {
            return self.receipt(
                MemorySearchResolutionStateV1::GenerationMismatch,
                None,
                vec![],
                vec![],
                started,
            );
        }
        let candidates = self
            .exact
            .get(&identity_key(&request.canonical_item_selector, true))
            .into_iter()
            .flatten()
            .map(|index| self.items[*index].clone())
            .collect::<Vec<_>>();
        if let Some(hit) = candidates
            .iter()
            .find(|item| item.owner_path == request.requested_owner_path)
            .cloned()
        {
            return self.receipt(
                MemorySearchResolutionStateV1::LiveHit,
                Some(hit),
                candidates,
                vec![],
                started,
            );
        }
        if candidates.len() == 1 {
            return self.receipt(
                MemorySearchResolutionStateV1::LiveRelocated,
                candidates.first().cloned(),
                candidates,
                vec![],
                started,
            );
        }
        if candidates.len() > 1 {
            return self.receipt(
                MemorySearchResolutionStateV1::Ambiguous,
                None,
                candidates,
                vec![],
                started,
            );
        }
        let actual_kinds = self
            .symbol
            .get(&identity_key(&request.canonical_item_selector, false))
            .into_iter()
            .flatten()
            .map(|index| {
                self.items[*index]
                    .canonical_item_selector
                    .kind
                    .as_str()
                    .to_owned()
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let state = if actual_kinds.is_empty() {
            MemorySearchResolutionStateV1::Missing
        } else {
            MemorySearchResolutionStateV1::KindMismatch
        };
        self.receipt(state, None, vec![], actual_kinds, started)
    }

    fn receipt(
        &self,
        state: MemorySearchResolutionStateV1,
        resolved: Option<MemorySearchItemV1>,
        candidates: Vec<MemorySearchItemV1>,
        actual_kinds: Vec<String>,
        started: Instant,
    ) -> MemorySearchResolutionV1 {
        MemorySearchResolutionV1 {
            state,
            resolved,
            actual_kinds,
            generation: MemorySearchGenerationReceiptV1 {
                generation_id: self.generation.generation_id.clone(),
                root_digest: self.generation.root_digest.clone(),
                root_depth: self.generation.root_depth,
                leaf_count: self.generation.leaf_count,
                owner_count: self.generation.owner_count,
                selector_count: self.generation.selector_count,
            },
            performance: MemorySearchPerformanceReceiptV1 {
                generation_load_micros: self.load_micros,
                index_lookup_micros: started.elapsed().as_micros(),
                candidate_count: candidates.len(),
                source_bytes_materialized: 0,
                db_opens: 0,
                db_queries: 0,
                provider_subprocesses: 0,
                cache_writes: 0,
            },
            candidates,
        }
    }
}
