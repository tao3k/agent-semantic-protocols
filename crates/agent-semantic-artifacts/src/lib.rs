//! Public facade for ASP artifact identity primitives.

pub mod blake3_content_digest;
mod edge;
mod identity;
mod repair_chain;
pub mod runtime_artifact_catalog;
pub mod runtime_artifact_publication;
pub mod runtime_artifact_quiescence;
pub mod runtime_artifact_retention;
mod schema_v1_digest;
mod state_home_binding;
mod state_home_catalog;
mod state_home_layout;
mod state_home_retention;

pub use agent_semantic_content_identity::{
    ResolutionAuthority, ResolutionEvidence, ResolutionState, SOURCE_RESOLUTION_SCHEMA_ID,
    SOURCE_SNAPSHOT_ALGORITHM, SOURCE_SNAPSHOT_SCHEMA_ID, SnapshotBoundResolution,
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, provider_digest,
};

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
    RepairChainFrameIdentity, RepairChainFrameInput, RepairChainFrameKind, RepairChainParentRef,
    build_repair_chain_frame,
};
pub use state_home_binding::{
    HostProjectReference, PROJECT_BINDING_SCHEMA_ID, PROJECT_BINDING_SCHEMA_VERSION,
    ProjectBinding, RepoIdentity, WorkspaceIdentity,
};
pub use state_home_catalog::{
    CatalogBatchReceipt, CatalogGeneration, CatalogObservation, CatalogObservationReceipt,
    STATE_HOME_CATALOG_SCHEMA_ID, STATE_HOME_CATALOG_SCHEMA_VERSION, StateHomeCatalog,
};
pub use state_home_layout::{StateHomeLayout, WorkspaceStatePaths};
pub use state_home_retention::{
    CleanupDisposition, CleanupPlan, CleanupPlanEntry, RETENTION_PLAN_SCHEMA_ID,
    RETENTION_PLAN_SCHEMA_VERSION, RetainedObject, RetentionLease, RetentionObjectKind,
    RetentionPlanner,
};

#[cfg(test)]
#[path = "../tests/unit/state_home_binding.rs"]
mod state_home_binding_tests;

#[cfg(test)]
#[path = "../tests/unit/state_home_catalog.rs"]
mod state_home_catalog_tests;

#[cfg(test)]
#[path = "../tests/unit/state_home_layout.rs"]
mod state_home_layout_tests;

#[cfg(test)]
#[path = "../tests/unit/state_home_retention.rs"]
mod state_home_retention_tests;
