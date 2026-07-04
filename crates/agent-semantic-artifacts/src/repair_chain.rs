//! Typed Merkle frames for ASP evidence-to-repair chains.
//!
//! The repair-chain model turns search evidence, edit boundaries, change sets,
//! and proof receipts into artifact roots. DB storage and render expansion stay
//! outside this crate.

use crate::identity::{
    ArtifactChildRef, ArtifactGeneration, ArtifactHash, ArtifactJson, ArtifactKind,
    ArtifactNodeInput, ArtifactRepoId, ArtifactRootInput, ArtifactRootRef, ArtifactScopeId,
    ArtifactWorkspaceId, hash_node, hash_normalized_json,
};
use serde::{Deserialize, Serialize};

/// Schema id for Merkle repair-chain frame nodes.
pub const REPAIR_CHAIN_FRAME_SCHEMA_ID: &str = "semantic-artifact-repair-chain-frame";
/// Schema version for Merkle repair-chain frame nodes.
pub const REPAIR_CHAIN_FRAME_SCHEMA_VERSION: &str = "1";

/// Shared artifact root kinds for the evidence-to-repair chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub enum RepairChainFrameKind {
    /// Intent-to-evidence provenance.
    #[serde(rename = "howFromFrame")]
    HowFromFrame,
    /// Evidence-to-change plan.
    #[serde(rename = "howFixFrame")]
    HowFixFrame,
    /// Changed selector/artifact boundary.
    #[serde(rename = "changeSet")]
    ChangeSet,
    /// Proof and validation receipts.
    #[serde(rename = "proofReceipt")]
    ProofReceipt,
    /// Topology or compact graph delta.
    #[serde(rename = "graphDiff")]
    GraphDiff,
}

impl RepairChainFrameKind {
    /// Borrow the shared root kind token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HowFromFrame => "howFromFrame",
            Self::HowFixFrame => "howFixFrame",
            Self::ChangeSet => "changeSet",
            Self::ProofReceipt => "proofReceipt",
            Self::GraphDiff => "graphDiff",
        }
    }

    fn artifact_kind(self) -> ArtifactKind {
        ArtifactKind::new(self.as_str())
    }
}

/// Parent root edge included in a repair-chain frame.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairChainParentRef {
    /// Semantic parent role such as `searchReceipt`, `howFrom`, or `proof`.
    pub role: String,
    /// Stable ordinal for repeated parent roles.
    pub ordinal: u64,
    /// Parent artifact root.
    pub root: ArtifactRootRef,
}

impl RepairChainParentRef {
    /// Build a parent edge with ordinal zero.
    pub fn new(role: impl Into<String>, root: ArtifactRootRef) -> Self {
        Self {
            role: role.into(),
            ordinal: 0,
            root,
        }
    }

    /// Assign a stable ordinal for repeated parent roles.
    pub fn with_ordinal(mut self, ordinal: u64) -> Self {
        self.ordinal = ordinal;
        self
    }

    fn child_name(&self) -> String {
        format!(
            "{}:{}",
            self.root.root_kind.as_str(),
            self.root.root_hash.as_integrity_ref()
        )
    }
}

/// Input for building one repair-chain Merkle frame.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairChainFrameInput {
    /// Frame kind and artifact root kind.
    pub frame_kind: RepairChainFrameKind,
    /// Stable State Core repository identity.
    pub repo_id: ArtifactRepoId,
    /// Stable State Core workspace identity.
    pub workspace_id: ArtifactWorkspaceId,
    /// Stable scope identity.
    pub scope_id: ArtifactScopeId,
    /// Artifact generation identity.
    pub generation: ArtifactGeneration,
    /// Optional producer identity hash.
    pub producer_hash: Option<ArtifactHash>,
    /// Optional schema digest for frame payload contracts.
    pub schema_hash: Option<ArtifactHash>,
    /// Canonical JSON payload for compact or expandable frame facts.
    pub content: ArtifactJson,
    /// Parent roots that explain the frame lineage.
    pub parents: Vec<RepairChainParentRef>,
}

impl RepairChainFrameInput {
    /// Build repair-chain frame input without optional producer/schema hashes.
    pub fn new(
        frame_kind: RepairChainFrameKind,
        repo_id: ArtifactRepoId,
        workspace_id: ArtifactWorkspaceId,
        scope_id: ArtifactScopeId,
        generation: ArtifactGeneration,
        content: ArtifactJson,
        parents: Vec<RepairChainParentRef>,
    ) -> Self {
        Self {
            frame_kind,
            repo_id,
            workspace_id,
            scope_id,
            generation,
            producer_hash: None,
            schema_hash: None,
            content,
            parents,
        }
    }

    /// Attach an optional producer hash.
    pub fn with_producer_hash(mut self, producer_hash: ArtifactHash) -> Self {
        self.producer_hash = Some(producer_hash);
        self
    }

    /// Attach an optional schema hash.
    pub fn with_schema_hash(mut self, schema_hash: ArtifactHash) -> Self {
        self.schema_hash = Some(schema_hash);
        self
    }
}

/// Built repair-chain frame with its compact root reference.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairChainFrame {
    /// Frame kind and artifact root kind.
    pub frame_kind: RepairChainFrameKind,
    /// Compact root reference for receipts, manifests, DB rows, and render.
    pub root: ArtifactRootRef,
    /// Hash of the canonical frame payload.
    pub content_hash: ArtifactHash,
    /// Parent roots that explain the frame lineage.
    pub parents: Vec<RepairChainParentRef>,
}

/// Build a repair-chain frame and its artifact root.
pub fn build_repair_chain_frame(input: RepairChainFrameInput) -> RepairChainFrame {
    let content_hash = hash_normalized_json(&input.content);
    let children = input
        .parents
        .iter()
        .map(|parent| {
            ArtifactChildRef::new(
                parent.role.clone(),
                parent.child_name(),
                parent.root.root_hash.clone(),
                parent.ordinal,
            )
        })
        .collect();
    let node_hash = hash_node(&ArtifactNodeInput {
        kind: input.frame_kind.artifact_kind(),
        schema_id: REPAIR_CHAIN_FRAME_SCHEMA_ID.to_string(),
        schema_version: REPAIR_CHAIN_FRAME_SCHEMA_VERSION.to_string(),
        producer_hash: input.producer_hash.clone(),
        payload_hash: Some(content_hash.clone()),
        metadata_hash: input.schema_hash.clone(),
        children,
    });
    let root = ArtifactRootRef::from_input(
        ArtifactRootInput {
            repo_id: input.repo_id,
            workspace_id: input.workspace_id,
            scope_id: input.scope_id,
            generation: input.generation,
            root_kind: input.frame_kind.artifact_kind(),
            node_hash,
        },
        input.producer_hash,
        input.schema_hash,
        Some(content_hash.clone()),
    );

    RepairChainFrame {
        frame_kind: input.frame_kind,
        root,
        content_hash,
        parents: input.parents,
    }
}
