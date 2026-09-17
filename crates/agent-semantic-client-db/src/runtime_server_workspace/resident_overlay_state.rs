// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Copy-on-write state and generation identity for resident workspace overlays.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use super::resident_content_overlay::{OwnerContentOverlayTrie, OwnerContentOverlayValue};
use super::resident_tantivy_overlay::ResidentTantivyDeltaLayer;
use super::resident_topology_overlay::ResidentTopologyDeltaLayer;
use super::{WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorOverlay};

#[derive(Debug, Clone)]
pub(super) struct ResidentOverlayState {
    pub(super) base_generation_digest: String,
    pub(super) generation_digest: String,
    pub(super) revision: u64,
    pub(super) workspace_snapshot: Arc<agent_semantic_content_identity::WorkspaceSnapshot>,
    pub(super) owner_identity_tree: Arc<
        agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeIncrementalV1,
    >,
    pub(super) content_owners: OwnerContentOverlayTrie,
    pub(super) tantivy_delta_head: Option<Arc<ResidentTantivyDeltaLayer>>,
    pub(super) topology_delta_head: Option<Arc<ResidentTopologyDeltaLayer>>,
    pub(super) owners: Arc<HashMap<String, WorkspaceOwnerSnapshot>>,
    pub(super) relations: Arc<HashMap<String, Vec<crate::ClientDbSourceIndexOwnedRelation>>>,
    pub(super) tombstones: Arc<HashSet<String>>,
    pub(super) semantic_owners: Arc<HashSet<String>>,
    pub(super) owner_grams: Arc<HashMap<String, BTreeSet<u32>>>,
    pub(super) selectors:
        Arc<HashMap<(super::model::ExactProjectionKind, String), WorkspaceRuntimeSelectorOverlay>>,
}

impl ResidentOverlayState {
    pub(super) fn empty() -> Self {
        Self {
            base_generation_digest: String::new(),
            generation_digest: String::new(),
            revision: 0,
            workspace_snapshot: Arc::new(
                agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                    std::iter::empty::<(String, String)>(),
                ),
            ),
            owner_identity_tree: Arc::new(
                agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeIncrementalV1::empty(),
            ),
            content_owners: OwnerContentOverlayTrie::default(),
            tantivy_delta_head: None,
            topology_delta_head: None,
            owners: Arc::new(HashMap::new()),
            relations: Arc::new(HashMap::new()),
            tombstones: Arc::new(HashSet::new()),
            semantic_owners: Arc::new(HashSet::new()),
            owner_grams: Arc::new(HashMap::new()),
            selectors: Arc::new(HashMap::new()),
        }
    }

    pub(super) fn new(generation: &WorkspaceMemoryGeneration) -> Self {
        let owner_identity_tree = agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeIncrementalV1::from_file_digests(
            generation.owners.iter().map(|owner| {
                (
                    owner.owner_path.clone(),
                    parse_typed_content_digest(&owner.content_digest)
                        .expect("validated generation owner content digest"),
                )
            }),
        )
        .expect("validated generation owner paths");
        Self {
            base_generation_digest: generation.generation_digest.clone(),
            generation_digest: generation.generation_digest.clone(),
            revision: 0,
            // The canonical generation remains reachable through the lease.
            // This state owns only changed owners; reads fall through to `base`.
            workspace_snapshot: Arc::new(generation.workspace_snapshot.clone()),
            owner_identity_tree: Arc::new(owner_identity_tree),
            content_owners: OwnerContentOverlayTrie::default(),
            tantivy_delta_head: None,
            topology_delta_head: None,
            owners: Arc::new(HashMap::new()),
            relations: Arc::new(HashMap::new()),
            tombstones: Arc::new(HashSet::new()),
            semantic_owners: Arc::new(HashSet::new()),
            owner_grams: Arc::new(HashMap::new()),
            selectors: Arc::new(HashMap::new()),
        }
    }

    pub(super) fn advance(&mut self) {
        self.revision = self.revision.saturating_add(1);
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.base_generation_digest.as_bytes());
        hasher.update(&self.revision.to_le_bytes());
        let mut owner_digests = self
            .owners
            .iter()
            .map(|(path, owner)| (path, &owner.content_digest))
            .collect::<Vec<_>>();
        owner_digests.sort_unstable();
        for (path, digest) in owner_digests {
            hasher.update(path.as_bytes());
            hasher.update(digest.as_bytes());
        }
        let mut tombstones = self.tombstones.iter().collect::<Vec<_>>();
        tombstones.sort_unstable();
        for path in tombstones {
            hasher.update(path.as_bytes());
            hasher.update(b"\0removed");
        }
        let mut semantic_owners = self.semantic_owners.iter().collect::<Vec<_>>();
        semantic_owners.sort_unstable();
        for path in semantic_owners {
            hasher.update(path.as_bytes());
            hasher.update(b"\0semantic");
        }
        self.generation_digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    }

    pub(super) fn advance_owner_content_mutation(
        &mut self,
        operations: &[agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1],
    ) {
        self.revision = self.revision.saturating_add(1);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"agent.semantic-protocols.owner-content-transaction.v1\0");
        hasher.update(self.generation_digest.as_bytes());
        hasher.update(&self.revision.to_le_bytes());
        for operation in operations {
            match operation {
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1::Upsert { owner_path, source_blob_digest, .. } => {
                    hasher.update(b"upsert\0");
                    hasher.update(owner_path.as_bytes());
                    hasher.update(source_blob_digest.as_str().as_bytes());
                }
                agent_semantic_content_identity::workspace_merkle_v1::WorkspaceMerkleDeltaOperationIncrementalV1::Remove { owner_path, previous_source_blob_digest } => {
                    hasher.update(b"remove\0");
                    hasher.update(owner_path.as_bytes());
                    hasher.update(previous_source_blob_digest.as_str().as_bytes());
                }
            }
        }
        hasher.update(self.owner_identity_tree.root_digest().as_str().as_bytes());
        self.generation_digest = format!("blake3-256:{}", hasher.finalize().to_hex());
    }

    pub(super) fn owner_snapshot(
        &self,
        base: &WorkspaceMemoryGeneration,
        owner_path: &str,
    ) -> Option<WorkspaceOwnerSnapshot> {
        if self.semantic_owners.contains(owner_path)
            && let Some(owner) = self.owners.get(owner_path)
        {
            return Some(owner.clone());
        }
        if let Some(content) = self.content_owners.get(owner_path) {
            return match content {
                OwnerContentOverlayValue::Present { owner, .. } => Some(owner.as_ref().clone()),
                OwnerContentOverlayValue::Removed => None,
            };
        }
        if self.tombstones.contains(owner_path) {
            return None;
        }
        self.owners
            .get(owner_path)
            .cloned()
            .or_else(|| base_owner(base, owner_path).cloned())
    }
}

pub(super) fn parse_typed_content_digest(
    digest: &str,
) -> Result<agent_semantic_content_identity::exact_selector_merkle::ContentDigestV1, String> {
    let payload = digest
        .strip_prefix("blake3-256:")
        .ok_or_else(|| "owner content digest must use blake3-256".to_owned())?;
    agent_semantic_content_identity::exact_selector_merkle::parse_content_digest_v1(payload)
}

pub(super) fn base_owner<'a>(
    base: &'a WorkspaceMemoryGeneration,
    owner_path: &str,
) -> Option<&'a WorkspaceOwnerSnapshot> {
    base.owners
        .iter()
        .find(|owner| owner.owner_path == owner_path)
}
