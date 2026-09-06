// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Binds normalized workspace paths and source blobs into one V1 Merkle root.

use crate::ContentDigestV1;
use crate::MerkleInclusionSideV1;
use crate::MerkleInclusionStepV1;
use crate::canonical_digest_v1;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Component;
use std::path::Path;

const EMPTY_DOMAIN: &[u8] = b"asp.workspace-merkle-empty.v1";
const LEAF_DOMAIN: &[u8] = b"asp.workspace-file-leaf.v1";
const NODE_DOMAIN: &[u8] = b"asp.workspace-merkle-node.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
/// Immutable Merkle tree over normalized workspace source paths.
pub struct WorkspacePathMerkleTreeV1 {
    leaves: Vec<WorkspaceMerkleLeafV1>,
    /// Immutable Merkle levels, from owner leaves through the root.
    ///
    /// Exact projection emits many selectors for the same workspace snapshot.
    /// Keeping the levels makes each inclusion proof O(log owners) instead of
    /// rebuilding the complete tree for every selector.
    levels: Vec<Vec<ContentDigestV1>>,
    root_digest: ContentDigestV1,
}

impl WorkspacePathMerkleTreeV1 {
    pub fn from_file_digests(
        file_digests: impl IntoIterator<Item = (String, ContentDigestV1)>,
    ) -> Result<Self, WorkspaceMerkleV1Error> {
        let mut entries = file_digests.into_iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));

        let mut paths = BTreeSet::new();
        let mut leaves = Vec::with_capacity(entries.len());
        for (path, source_blob_digest) in entries {
            validate_path(&path)?;
            if !paths.insert(path.clone()) {
                return Err(WorkspaceMerkleV1Error::DuplicatePath);
            }
            let owner_subtree_digest = derive_owner_subtree_digest_v1(&path, &source_blob_digest);
            leaves.push(WorkspaceMerkleLeafV1 {
                path,
                source_blob_digest,
                owner_subtree_digest,
            });
        }

        let levels = merkle_levels(
            leaves
                .iter()
                .map(|leaf| leaf.owner_subtree_digest.clone())
                .collect(),
        );
        let root_digest = levels
            .last()
            .and_then(|level| level.first())
            .cloned()
            .unwrap_or_else(|| canonical_digest_v1(EMPTY_DOMAIN, &[]));
        Ok(Self {
            leaves,
            levels,
            root_digest,
        })
    }

    pub fn root_digest(&self) -> &ContentDigestV1 {
        &self.root_digest
    }

    pub fn owner_subtree_digest(&self, path: &str) -> Option<&ContentDigestV1> {
        self.leaf_index(path)
            .map(|index| &self.leaves[index].owner_subtree_digest)
    }

    pub fn source_blob_digest(&self, path: &str) -> Option<&ContentDigestV1> {
        self.leaf_index(path)
            .map(|index| &self.leaves[index].source_blob_digest)
    }

    pub fn inclusion_proof(&self, path: &str) -> Option<Vec<MerkleInclusionStepV1>> {
        let mut target_index = self.leaf_index(path)?;
        let mut proof = Vec::with_capacity(self.levels.len().saturating_sub(1));

        for level in self.levels.iter().take(self.levels.len().saturating_sub(1)) {
            let target_is_left = target_index % 2 == 0;
            let sibling_index = if target_is_left {
                (target_index + 1).min(level.len() - 1)
            } else {
                target_index - 1
            };
            proof.push(MerkleInclusionStepV1 {
                side: if target_is_left {
                    MerkleInclusionSideV1::Right
                } else {
                    MerkleInclusionSideV1::Left
                },
                digest: level[sibling_index].clone(),
            });
            target_index /= 2;
        }
        Some(proof)
    }

    fn leaf_index(&self, path: &str) -> Option<usize> {
        self.leaves
            .binary_search_by(|leaf| leaf.path.as_bytes().cmp(path.as_bytes()))
            .ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceMerkleLeafV1 {
    path: String,
    source_blob_digest: ContentDigestV1,
    owner_subtree_digest: ContentDigestV1,
}

/// Derives the owner-subtree digest committed by one workspace leaf.
pub fn derive_owner_subtree_digest_v1(
    owner_path: &str,
    source_blob_digest: &ContentDigestV1,
) -> ContentDigestV1 {
    canonical_digest_v1(
        LEAF_DOMAIN,
        &[
            owner_path.as_bytes(),
            source_blob_digest.as_str().as_bytes(),
        ],
    )
}

/// Verifies one owner and source blob against an admitted workspace root.
pub struct WorkspaceOwnerInclusionV1<'a> {
    pub owner_path: &'a str,
    pub source_blob_digest: &'a ContentDigestV1,
    pub expected_owner_subtree_digest: &'a ContentDigestV1,
    pub inclusion_proof: &'a [MerkleInclusionStepV1],
    pub expected_workspace_root_digest: &'a ContentDigestV1,
}

/// Verifies one named owner inclusion request against its admitted workspace root.
pub fn verify_owner_inclusion_v1(input: WorkspaceOwnerInclusionV1<'_>) -> bool {
    let WorkspaceOwnerInclusionV1 {
        owner_path,
        source_blob_digest,
        expected_owner_subtree_digest,
        inclusion_proof,
        expected_workspace_root_digest,
    } = input;
    let mut current = canonical_digest_bytes_v1(
        LEAF_DOMAIN,
        &[
            owner_path.as_bytes(),
            source_blob_digest.as_str().as_bytes(),
        ],
    );
    if !digest_bytes_match_hex(&current, expected_owner_subtree_digest) {
        return false;
    }
    let mut current_hex = [0_u8; 64];
    for step in inclusion_proof {
        encode_lower_hex(&current, &mut current_hex);
        current = match step.side {
            MerkleInclusionSideV1::Left => canonical_digest_bytes_v1(
                NODE_DOMAIN,
                &[step.digest.as_str().as_bytes(), &current_hex],
            ),
            MerkleInclusionSideV1::Right => canonical_digest_bytes_v1(
                NODE_DOMAIN,
                &[&current_hex, step.digest.as_str().as_bytes()],
            ),
        };
    }
    digest_bytes_match_hex(&current, expected_workspace_root_digest)
}

fn canonical_digest_bytes_v1(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(domain.len() as u64).to_be_bytes());
    hasher.update(domain);
    hasher.update(&(parts.len() as u64).to_be_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

fn digest_bytes_match_hex(digest: &[u8; 32], expected: &ContentDigestV1) -> bool {
    let mut encoded = [0_u8; 64];
    encode_lower_hex(digest, &mut encoded);
    encoded.as_slice() == expected.as_str().as_bytes()
}

fn encode_lower_hex(digest: &[u8; 32], output: &mut [u8; 64]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in digest.iter().copied().enumerate() {
        output[index * 2] = HEX[(byte >> 4) as usize];
        output[index * 2 + 1] = HEX[(byte & 0x0f) as usize];
    }
}

fn merkle_levels(mut level: Vec<ContentDigestV1>) -> Vec<Vec<ContentDigestV1>> {
    let mut levels = Vec::new();
    while !level.is_empty() {
        levels.push(level);
        if levels.last().is_some_and(|level| level.len() == 1) {
            break;
        }
        level = next_level(levels.last().expect("a Merkle level was just added"));
    }
    levels
}

fn next_level(level: &[ContentDigestV1]) -> Vec<ContentDigestV1> {
    level
        .chunks(2)
        .map(|pair| {
            let left = &pair[0];
            let right = pair.get(1).unwrap_or(left);
            hash_node(left, right)
        })
        .collect()
}

fn hash_node(left: &ContentDigestV1, right: &ContentDigestV1) -> ContentDigestV1 {
    canonical_digest_v1(
        NODE_DOMAIN,
        &[left.as_str().as_bytes(), right.as_str().as_bytes()],
    )
}

fn validate_path(path: &str) -> Result<(), WorkspaceMerkleV1Error> {
    let path_value = Path::new(path);
    if path.trim().is_empty()
        || path_value.is_absolute()
        || path_value.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(WorkspaceMerkleV1Error::InvalidPath);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Deterministic construction or inclusion failure for a workspace Merkle tree.
pub enum WorkspaceMerkleV1Error {
    InvalidPath,
    DuplicatePath,
}

impl fmt::Display for WorkspaceMerkleV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPath => "workspace Merkle path must be normalized and relative",
            Self::DuplicatePath => "workspace Merkle paths must be unique",
        })
    }
}

impl std::error::Error for WorkspaceMerkleV1Error {}
#[path = "workspace_merkle_incremental_v1.rs"]
mod incremental_v1;
pub use incremental_v1::WorkspaceMerkleDeltaMetricsIncrementalV1;
pub use incremental_v1::WorkspaceMerkleDeltaOperationIncrementalV1;
pub use incremental_v1::WorkspaceMerkleIncrementalV1Error;
pub use incremental_v1::WorkspaceMerkleNodeRecordIncrementalV1;
pub use incremental_v1::WorkspaceMerkleProofIncrementalV1;
pub use incremental_v1::WorkspaceMerkleProofSiblingIncrementalV1;
pub use incremental_v1::WorkspaceMerkleProofStepIncrementalV1;
pub use incremental_v1::WorkspacePathMerkleTreeIncrementalV1;
pub use incremental_v1::derive_owner_subtree_digest_incremental_v1;
pub use incremental_v1::verify_owner_inclusion_incremental_v1;
