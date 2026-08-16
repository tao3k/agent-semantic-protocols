use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
    sync::Arc,
};

use crate::exact_selector_merkle::{ContentDigestV1, canonical_digest_v1};
use serde::{Deserialize, Serialize};

const EMPTY_DOMAIN: &[u8] = b"asp.workspace-path-radix-merkle-v1-incremental.empty";
const LEAF_DOMAIN: &[u8] = b"asp.workspace-path-radix-merkle-v1-incremental.leaf";
const NODE_DOMAIN: &[u8] = b"asp.workspace-path-radix-merkle-v1-incremental.node";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceMerkleIncrementalV1Error {
    InvalidPath,
    DuplicatePath,
    MissingPath,
    PreviousDigestMismatch,
    DuplicateSiblingEdge,
    InvalidProof,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceMerkleDeltaMetricsIncrementalV1 {
    pub touched_leaf_count: usize,
    pub written_node_count: usize,
    pub reused_node_count: usize,
    pub full_merkle_rebuilds: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleProofSiblingIncrementalV1 {
    pub edge: u8,
    pub digest: ContentDigestV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleProofStepIncrementalV1 {
    pub depth: usize,
    pub terminal_digest: Option<ContentDigestV1>,
    pub siblings: Vec<WorkspaceMerkleProofSiblingIncrementalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleProofIncrementalV1 {
    pub owner_path: String,
    pub source_blob_digest: ContentDigestV1,
    pub owner_subtree_digest: ContentDigestV1,
    pub steps: Vec<WorkspaceMerkleProofStepIncrementalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMerkleNodeRecordIncrementalV1 {
    pub path_prefix_hex: String,
    pub terminal_digest: Option<ContentDigestV1>,
    pub children: Vec<WorkspaceMerkleProofSiblingIncrementalV1>,
    pub digest: ContentDigestV1,
}

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

#[derive(Debug, Clone)]
pub struct WorkspacePathMerkleTreeIncrementalV1 {
    root: Arc<RadixNodeIncrementalV1>,
    leaves: Arc<BTreeMap<String, ContentDigestV1>>,
}

impl WorkspacePathMerkleTreeIncrementalV1 {
    pub fn empty() -> Self {
        Self {
            root: RadixNodeIncrementalV1::empty(),
            leaves: Arc::new(BTreeMap::new()),
        }
    }

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

    pub fn root_digest(&self) -> &ContentDigestV1 {
        &self.root.digest
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    pub fn node_count(&self) -> usize {
        count_nodes(&self.root)
    }

    pub fn node_table(&self) -> Vec<WorkspaceMerkleNodeRecordIncrementalV1> {
        let mut records = Vec::with_capacity(self.node_count());
        collect_node_records(&self.root, &mut Vec::new(), &mut records);
        records
    }

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

    pub fn source_blob_digest(&self, owner_path: &str) -> Option<&ContentDigestV1> {
        self.leaves.get(owner_path)
    }

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

pub fn verify_owner_inclusion_incremental_v1(
    proof: &WorkspaceMerkleProofIncrementalV1,
    expected_root_digest: &ContentDigestV1,
) -> Result<bool, WorkspaceMerkleIncrementalV1Error> {
    validate_path(&proof.owner_path)?;
    if proof.steps.len() != proof.owner_path.len() + 1
        || derive_owner_subtree_digest_incremental_v1(&proof.owner_path, &proof.source_blob_digest)
            != proof.owner_subtree_digest
    {
        return Ok(false);
    }
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
                return Ok(false);
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
    Ok(child_digest.as_ref() == Some(expected_root_digest))
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
    for (edge, digest) in children {
        payload.push(*edge);
        payload.extend_from_slice(digest.as_str().as_bytes());
    }
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
mod tests {
    use super::*;
    use crate::exact_selector_merkle::parse_content_digest_v1;

    fn digest(byte: char) -> ContentDigestV1 {
        parse_content_digest_v1(&byte.to_string().repeat(64)).expect("test digest")
    }

    #[test]
    fn one_owner_update_rewrites_only_the_owner_path() {
        let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([
            ("src/lib.rs".to_owned(), digest('1')),
            ("src/sibling.rs".to_owned(), digest('2')),
        ])
        .expect("base tree");
        let previous_node_count = tree.node_count();
        let (successor, metrics) = tree
            .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
                owner_path: "src/lib.rs".to_owned(),
                previous_source_blob_digest: Some(digest('1')),
                source_blob_digest: digest('3'),
            }])
            .expect("incremental update");
        assert_ne!(successor.root_digest(), tree.root_digest());
        assert_eq!(successor.leaf_count(), 2);
        assert_eq!(successor.node_count(), previous_node_count);
        assert_eq!(metrics.touched_leaf_count, 1);
        assert_eq!(metrics.written_node_count, "src/lib.rs".len() + 1);
        assert!(metrics.reused_node_count > 0);
        assert_eq!(metrics.full_merkle_rebuilds, 0);
    }

    #[test]
    fn inclusion_proof_round_trips_after_incremental_update() {
        let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([
            ("src/lib.rs".to_owned(), digest('1')),
            ("src/sibling.rs".to_owned(), digest('2')),
        ])
        .expect("base tree");
        let (successor, _) = tree
            .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
                owner_path: "src/lib.rs".to_owned(),
                previous_source_blob_digest: Some(digest('1')),
                source_blob_digest: digest('3'),
            }])
            .expect("incremental update");
        let proof = successor
            .inclusion_proof("src/lib.rs")
            .expect("owner proof");
        assert!(
            verify_owner_inclusion_incremental_v1(&proof, successor.root_digest())
                .expect("valid proof")
        );
        assert!(
            !verify_owner_inclusion_incremental_v1(&proof, tree.root_digest()).expect("stale root")
        );
    }

    #[test]
    fn delta_preconditions_fail_closed() {
        let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([(
            "src/lib.rs".to_owned(),
            digest('1'),
        )])
        .expect("base tree");
        let error = tree
            .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Remove {
                owner_path: "src/lib.rs".to_owned(),
                previous_source_blob_digest: digest('2'),
            }])
            .expect_err("digest drift must fail");
        assert_eq!(
            error,
            WorkspaceMerkleIncrementalV1Error::PreviousDigestMismatch
        );
    }

    #[test]
    fn node_table_is_deterministic_and_linear_in_tree_nodes() {
        let tree = WorkspacePathMerkleTreeIncrementalV1::from_file_digests([
            ("src/lib.rs".to_owned(), digest('1')),
            ("src/sibling.rs".to_owned(), digest('2')),
        ])
        .expect("tree");
        let records = tree.node_table();
        assert_eq!(records.len(), tree.node_count());
        assert_eq!(records.first().expect("root record").path_prefix_hex, "");
        assert_eq!(
            records.first().expect("root record").digest,
            *tree.root_digest()
        );
        assert_eq!(tree.node_table_digest(), tree.node_table_digest());

        let (successor, _) = tree
            .apply_delta(&[WorkspaceMerkleDeltaOperationIncrementalV1::Upsert {
                owner_path: "src/lib.rs".to_owned(),
                previous_source_blob_digest: Some(digest('1')),
                source_blob_digest: digest('3'),
            }])
            .expect("successor");
        assert_ne!(tree.node_table_digest(), successor.node_table_digest());
    }
}
