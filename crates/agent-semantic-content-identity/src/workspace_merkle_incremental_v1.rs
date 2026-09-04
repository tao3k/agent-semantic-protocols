//! Persistent incremental workspace Merkle tree, delta metrics, and inclusion proofs.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Component;
use std::path::Path;
use std::sync::Arc;

use crate::exact_selector_merkle::ContentDigestV1;
use crate::exact_selector_merkle::canonical_digest_v1;
use serde::Deserialize;
use serde::Serialize;

const EMPTY_DOMAIN: &[u8] = b"asp.workspace-path-radix-merkle-v1-incremental.empty";
const LEAF_DOMAIN: &[u8] = b"asp.workspace-path-radix-merkle-v1-incremental.leaf";
const NODE_DOMAIN: &[u8] = b"asp.workspace-path-radix-merkle-v1-incremental.node";

/// Typed failures produced by incremental workspace Merkle operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceMerkleIncrementalV1Error {
    InvalidPath,
    DuplicatePath,
    MissingPath,
    PreviousDigestMismatch,
    DuplicateSiblingEdge,
    InvalidProof,
}

/// One content-addressed delta applied to the incremental workspace tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceMerkleDeltaOperationIncrementalV1 {
    Upsert {
        owner_path: String,
        previous_source_blob_digest: Option<ContentDigestV1>,
        source_blob_digest: ContentDigestV1,
    },
    Remove {
        owner_path: String,
        previous_source_blob_digest: ContentDigestV1,
    },
}

/// Measured structural work performed by one incremental delta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceMerkleDeltaMetricsIncrementalV1 {
    pub touched_leaf_count: usize,
    pub written_node_count: usize,
    pub reused_node_count: usize,
    pub full_merkle_rebuilds: usize,
}

/// One sibling edge retained in an incremental inclusion proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleProofSiblingIncrementalV1 {
    pub edge: u8,
    pub digest: ContentDigestV1,
}

/// One radix depth step retained in an incremental inclusion proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleProofStepIncrementalV1 {
    pub depth: usize,
    pub terminal_digest: Option<ContentDigestV1>,
    pub siblings: Vec<WorkspaceMerkleProofSiblingIncrementalV1>,
}

/// Complete inclusion proof for one owner path and source blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleProofIncrementalV1 {
    pub owner_path: String,
    pub source_blob_digest: ContentDigestV1,
    pub owner_subtree_digest: ContentDigestV1,
    pub steps: Vec<WorkspaceMerkleProofStepIncrementalV1>,
}

/// Serializable record for one persistent radix node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleNodeRecordIncrementalV1 {
    pub path_prefix_hex: String,
    pub terminal_digest: Option<ContentDigestV1>,
    pub children: Vec<WorkspaceMerkleProofSiblingIncrementalV1>,
    pub digest: ContentDigestV1,
}

/// Persistent path-radix Merkle tree supporting content-addressed deltas.
#[derive(Debug, Clone)]
struct RadixNodeIncrementalV1 {
    terminal_digest: Option<ContentDigestV1>,
    children: BTreeMap<u8, Arc<RadixNodeIncrementalV1>>,
    digest: ContentDigestV1,
}

impl RadixNodeIncrementalV1 {
    fn new(
        terminal_digest: Option<ContentDigestV1>,
        children: BTreeMap<u8, Arc<Self>>,
    ) -> Arc<Self> {
        let digest = radix_node_digest(&terminal_digest, &children);
        Arc::new(Self {
            terminal_digest,
            children,
            digest,
        })
    }

    fn empty() -> Arc<Self> {
        Self::new(None, BTreeMap::new())
    }
}

/// Persistent path-radix Merkle tree supporting content-addressed deltas.
#[derive(Debug, Clone)]
pub struct WorkspacePathMerkleTreeIncrementalV1 {
    root: Arc<RadixNodeIncrementalV1>,
    leaves: Arc<BTreeMap<String, ContentDigestV1>>,
}

impl WorkspacePathMerkleTreeIncrementalV1 {
    /// Creates an empty workspace Merkle tree.
    pub fn empty() -> Self {
        Self {
            root: RadixNodeIncrementalV1::empty(),
            leaves: Arc::new(BTreeMap::new()),
        }
    }

    /// Builds a workspace Merkle tree from unique normalized owner paths.
    pub fn from_file_digests(
        file_digests: impl IntoIterator<Item = (String, ContentDigestV1)>,
    ) -> Result<Self, WorkspaceMerkleIncrementalV1Error> {
        let mut leaves = BTreeMap::new();
        for (path, digest) in file_digests {
            validate_path(&path)?;
            if leaves.insert(path, digest).is_some() {
                return Err(WorkspaceMerkleIncrementalV1Error::DuplicatePath);
            }
        }
        let mut tree = Self::empty();
        for (path, digest) in &leaves {
            let owner_digest = derive_owner_subtree_digest_incremental_v1(path, digest);
            tree.root = replace_path(&tree.root, path.as_bytes(), 0, Some(owner_digest));
        }
        tree.leaves = Arc::new(leaves);
        Ok(tree)
    }

    /// Returns the current root digest.
    pub fn root_digest(&self) -> &ContentDigestV1 {
        &self.root.digest
    }

    /// Returns the number of owner-path leaves.
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// Returns the number of persistent radix nodes.
    pub fn node_count(&self) -> usize {
        count_nodes(&self.root)
    }

    /// Materializes a deterministic node table for persistence.
    pub fn node_table(&self) -> Vec<WorkspaceMerkleNodeRecordIncrementalV1> {
        let mut records = Vec::with_capacity(self.node_count());
        collect_node_records(&self.root, &mut Vec::new(), &mut records);
        records
    }

    /// Returns the content digest of the deterministic node table.
    pub fn node_table_digest(&self) -> ContentDigestV1 {
        let records = self.node_table();
        let mut payload = Vec::new();
        payload.extend_from_slice(&(records.len() as u64).to_be_bytes());
        for record in records {
            payload.extend_from_slice(&(record.path_prefix_hex.len() as u64).to_be_bytes());
            payload.extend_from_slice(record.path_prefix_hex.as_bytes());
            payload.extend_from_slice(record.digest.as_str().as_bytes());
        }
        canonical_digest_v1(
            b"asp.workspace-path-radix-merkle-v1-incremental.node-table",
            &[&payload],
        )
    }

    /// Looks up the exact source blob digest for one owner path.
    pub fn source_blob_digest(&self, owner_path: &str) -> Option<&ContentDigestV1> {
        self.leaves.get(owner_path)
    }

    /// Applies a bounded set of content-addressed upserts and removals.
    pub fn apply_delta(
        &self,
        operations: &[WorkspaceMerkleDeltaOperationIncrementalV1],
    ) -> Result<(Self, WorkspaceMerkleDeltaMetricsIncrementalV1), WorkspaceMerkleIncrementalV1Error>
    {
        let mut root = Arc::clone(&self.root);
        let mut leaves = self.leaves.as_ref().clone();
        let mut touched_paths = BTreeSet::new();
        let mut written_node_count = 0usize;
        let mut reused_node_count = 0usize;

        for operation in operations {
            let (owner_path, replacement) = match operation {
                WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
                    owner_path,
                    previous_source_blob_digest,
                    source_blob_digest,
                } => {
                    validate_path(owner_path)?;
                    match (leaves.get(owner_path), previous_source_blob_digest) {
                        (None, None) => {}
                        (Some(actual), Some(expected)) if actual == expected => {}
                        (None, Some(_)) | (Some(_), None) => {
                            return Err(WorkspaceMerkleIncrementalV1Error::PreviousDigestMismatch);
                        }
                        (Some(_), Some(_)) => {
                            return Err(WorkspaceMerkleIncrementalV1Error::PreviousDigestMismatch);
                        }
                    }
                    leaves.insert(owner_path.clone(), source_blob_digest.clone());
                    (
                        owner_path,
                        Some(derive_owner_subtree_digest_incremental_v1(
                            owner_path,
                            source_blob_digest,
                        )),
                    )
                }
                WorkspaceMerkleDeltaOperationIncrementalV1::Remove {
                    owner_path,
                    previous_source_blob_digest,
                } => {
                    validate_path(owner_path)?;
                    let Some(actual) = leaves.get(owner_path) else {
                        return Err(WorkspaceMerkleIncrementalV1Error::MissingPath);
                    };
                    if actual != previous_source_blob_digest {
                        return Err(WorkspaceMerkleIncrementalV1Error::PreviousDigestMismatch);
                    }
                    leaves.remove(owner_path);
                    (owner_path, None)
                }
            };
            let (next_root, written, reused) =
                replace_path_with_metrics(&root, owner_path.as_bytes(), 0, replacement);
            root = next_root;
            touched_paths.insert(owner_path.clone());
            written_node_count = written_node_count.saturating_add(written);
            reused_node_count = reused_node_count.saturating_add(reused);
        }

        Ok((
            Self {
                root,
                leaves: Arc::new(leaves),
            },
            WorkspaceMerkleDeltaMetricsIncrementalV1 {
                touched_leaf_count: touched_paths.len(),
                written_node_count,
                reused_node_count,
                full_merkle_rebuilds: 0,
            },
        ))
    }

    /// Builds an inclusion proof for one current owner path.
    pub fn inclusion_proof(&self, owner_path: &str) -> Option<WorkspaceMerkleProofIncrementalV1> {
        let source_blob_digest = self.leaves.get(owner_path)?.clone();
        let owner_subtree_digest =
            derive_owner_subtree_digest_incremental_v1(owner_path, &source_blob_digest);
        let mut node = Arc::clone(&self.root);
        let mut steps = Vec::with_capacity(owner_path.len() + 1);
        for (depth, edge) in owner_path.bytes().enumerate() {
            steps.push(proof_step(&node, depth, Some(edge)));
            node = Arc::clone(node.children.get(&edge)?);
        }
        steps.push(proof_step(&node, owner_path.len(), None));
        Some(WorkspaceMerkleProofIncrementalV1 {
            owner_path: owner_path.to_owned(),
            source_blob_digest,
            owner_subtree_digest,
            steps,
        })
    }
}

/// Derives the leaf digest binding an owner path to its source blob.
pub fn derive_owner_subtree_digest_incremental_v1(
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

/// Verifies one incremental owner inclusion proof against an expected root.
pub fn verify_owner_inclusion_incremental_v1(
    proof: &WorkspaceMerkleProofIncrementalV1,
    expected_root_digest: &ContentDigestV1,
) -> Result<bool, WorkspaceMerkleIncrementalV1Error> {
    validate_path(&proof.owner_path)?;
    if !incremental_proof_shape_matches(proof) {
        return Ok(false);
    }
    let child_digest = fold_incremental_proof_steps(proof)?;
    Ok(child_digest.as_ref() == Some(expected_root_digest))
}

fn incremental_proof_shape_matches(proof: &WorkspaceMerkleProofIncrementalV1) -> bool {
    proof.steps.len() == proof.owner_path.len() + 1
        && derive_owner_subtree_digest_incremental_v1(&proof.owner_path, &proof.source_blob_digest)
            == proof.owner_subtree_digest
}

fn fold_incremental_proof_steps(
    proof: &WorkspaceMerkleProofIncrementalV1,
) -> Result<Option<ContentDigestV1>, WorkspaceMerkleIncrementalV1Error> {
    let mut child_digest = None;
    for depth in (0..proof.steps.len()).rev() {
        let step = &proof.steps[depth];
        if step.depth != depth {
            return Err(WorkspaceMerkleIncrementalV1Error::InvalidProof);
        }
        let mut child_digests = BTreeMap::new();
        for sibling in &step.siblings {
            if child_digests
                .insert(sibling.edge, sibling.digest.clone())
                .is_some()
            {
                return Err(WorkspaceMerkleIncrementalV1Error::DuplicateSiblingEdge);
            }
        }
        if let Some(digest) = child_digest.take() {
            let edge = proof.owner_path.as_bytes()[depth];
            if child_digests.insert(edge, digest).is_some() {
                return Err(WorkspaceMerkleIncrementalV1Error::DuplicateSiblingEdge);
            }
        }
        let terminal_digest = if depth == proof.owner_path.len() {
            if step.terminal_digest.as_ref() != Some(&proof.owner_subtree_digest) {
                return Ok(None);
            }
            Some(proof.owner_subtree_digest.clone())
        } else {
            step.terminal_digest.clone()
        };
        child_digest = Some(radix_node_digest_from_digests(
            &terminal_digest,
            &child_digests,
        ));
    }
    Ok(child_digest)
}

fn proof_step(
    node: &RadixNodeIncrementalV1,
    depth: usize,
    followed_edge: Option<u8>,
) -> WorkspaceMerkleProofStepIncrementalV1 {
    WorkspaceMerkleProofStepIncrementalV1 {
        depth,
        terminal_digest: node.terminal_digest.clone(),
        siblings: node
            .children
            .iter()
            .filter(|(edge, _)| Some(**edge) != followed_edge)
            .map(|(edge, child)| WorkspaceMerkleProofSiblingIncrementalV1 {
                edge: *edge,
                digest: child.digest.clone(),
            })
            .collect(),
    }
}

fn replace_path(
    node: &Arc<RadixNodeIncrementalV1>,
    path: &[u8],
    depth: usize,
    replacement: Option<ContentDigestV1>,
) -> Arc<RadixNodeIncrementalV1> {
    replace_path_with_metrics(node, path, depth, replacement).0
}

fn replace_path_with_metrics(
    node: &Arc<RadixNodeIncrementalV1>,
    path: &[u8],
    depth: usize,
    replacement: Option<ContentDigestV1>,
) -> (Arc<RadixNodeIncrementalV1>, usize, usize) {
    if depth == path.len() {
        return (
            RadixNodeIncrementalV1::new(replacement, node.children.clone()),
            1,
            node.children.len(),
        );
    }
    let edge = path[depth];
    let child = node
        .children
        .get(&edge)
        .cloned()
        .unwrap_or_else(RadixNodeIncrementalV1::empty);
    let (next_child, written, reused) =
        replace_path_with_metrics(&child, path, depth + 1, replacement);
    let mut children = node.children.clone();
    if next_child.terminal_digest.is_none() && next_child.children.is_empty() {
        children.remove(&edge);
    } else {
        children.insert(edge, next_child);
    }
    (
        RadixNodeIncrementalV1::new(node.terminal_digest.clone(), children),
        written + 1,
        reused
            + node
                .children
                .len()
                .saturating_sub(usize::from(node.children.contains_key(&edge))),
    )
}

fn radix_node_digest(
    terminal_digest: &Option<ContentDigestV1>,
    children: &BTreeMap<u8, Arc<RadixNodeIncrementalV1>>,
) -> ContentDigestV1 {
    radix_node_digest_from_digests(
        terminal_digest,
        &children
            .iter()
            .map(|(edge, node)| (*edge, node.digest.clone()))
            .collect(),
    )
}

fn radix_node_digest_from_digests(
    terminal_digest: &Option<ContentDigestV1>,
    children: &BTreeMap<u8, ContentDigestV1>,
) -> ContentDigestV1 {
    if terminal_digest.is_none() && children.is_empty() {
        return canonical_digest_v1(EMPTY_DOMAIN, &[]);
    }
    let mut payload = Vec::new();
    match terminal_digest {
        Some(digest) => {
            payload.push(1);
            payload.extend_from_slice(digest.as_str().as_bytes());
        }
        None => payload.push(0),
    }
    payload.extend_from_slice(&(children.len() as u64).to_be_bytes());
    payload.extend(children.iter().flat_map(|(edge, digest)| {
        std::iter::once(*edge).chain(digest.as_str().as_bytes().iter().copied())
    }));
    canonical_digest_v1(NODE_DOMAIN, &[&payload])
}

fn count_nodes(node: &RadixNodeIncrementalV1) -> usize {
    1 + node
        .children
        .values()
        .map(|child| count_nodes(child))
        .sum::<usize>()
}

fn collect_node_records(
    node: &RadixNodeIncrementalV1,
    prefix: &mut Vec<u8>,
    records: &mut Vec<WorkspaceMerkleNodeRecordIncrementalV1>,
) {
    records.push(WorkspaceMerkleNodeRecordIncrementalV1 {
        path_prefix_hex: encode_hex(prefix),
        terminal_digest: node.terminal_digest.clone(),
        children: node
            .children
            .iter()
            .map(|(edge, child)| WorkspaceMerkleProofSiblingIncrementalV1 {
                edge: *edge,
                digest: child.digest.clone(),
            })
            .collect(),
        digest: node.digest.clone(),
    });
    for (edge, child) in &node.children {
        prefix.push(*edge);
        collect_node_records(child, prefix, records);
        prefix.pop();
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn validate_path(path: &str) -> Result<(), WorkspaceMerkleIncrementalV1Error> {
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
        return Err(WorkspaceMerkleIncrementalV1Error::InvalidPath);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/workspace_merkle_incremental_v1.rs"]
mod tests;
