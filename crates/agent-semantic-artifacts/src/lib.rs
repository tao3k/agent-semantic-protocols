//! Public facade for ASP artifact identity primitives.

mod edge;
mod identity;
mod repair_chain;

pub use edge::{
    ARTIFACT_EDGE_SCHEMA_ID, ARTIFACT_EDGE_SCHEMA_VERSION, ArtifactRootEdge, ArtifactRootEdgeInput,
    build_artifact_root_edge, hash_artifact_root_edge,
};
pub use identity::{
    ARTIFACT_IDENTITY_SCHEMA_ID, ARTIFACT_IDENTITY_SCHEMA_VERSION, ArtifactChildRef,
    ArtifactGeneration, ArtifactHash, ArtifactIdentityDocument, ArtifactJson, ArtifactKind,
    ArtifactLeafInput, ArtifactNodeInput, ArtifactRepoId, ArtifactRootInput, ArtifactRootRef,
    ArtifactScopeId, ArtifactWorkspaceId, EDGE_DOMAIN_V1, HASH_ALGORITHM_BLAKE3, JSON_DOMAIN_V1,
    LEAF_DOMAIN_V1, NODE_DOMAIN_V1, ROOT_DOMAIN_V1, hash_leaf, hash_node, hash_normalized_json,
    hash_root,
};
pub use repair_chain::{
    REPAIR_CHAIN_FRAME_SCHEMA_ID, REPAIR_CHAIN_FRAME_SCHEMA_VERSION, RepairChainFrame,
    RepairChainFrameInput, RepairChainFrameKind, RepairChainParentRef, build_repair_chain_frame,
};
