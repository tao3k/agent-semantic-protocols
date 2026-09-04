//! Public facade for ASP artifact identity primitives.

pub mod blake3_content_digest;
mod edge;
mod identity;
pub mod installed_provider_binding;
mod repair_chain;
pub mod runtime_artifact_activation;
pub mod runtime_artifact_catalog;
pub mod runtime_artifact_publication;
pub mod runtime_artifact_quiescence;
pub mod runtime_artifact_retention;
pub mod runtime_artifact_slots;
pub mod runtime_artifact_store;
pub mod runtime_provider_catalog;
mod schema_v1_digest;
mod state_home_binding;
mod state_home_catalog;
mod state_home_layout;
mod state_home_retention;

pub use agent_semantic_content_identity::ResolutionAuthority;
pub use agent_semantic_content_identity::ResolutionEvidence;
pub use agent_semantic_content_identity::ResolutionState;
pub use agent_semantic_content_identity::SOURCE_RESOLUTION_SCHEMA_ID;
pub use agent_semantic_content_identity::SOURCE_SNAPSHOT_ALGORITHM;
pub use agent_semantic_content_identity::SOURCE_SNAPSHOT_SCHEMA_ID;
pub use agent_semantic_content_identity::SnapshotBoundResolution;
pub use agent_semantic_content_identity::SourceSnapshotEvidence;
pub use agent_semantic_content_identity::SourceSnapshotKind;
pub use agent_semantic_content_identity::WorkspaceSnapshot;
pub use agent_semantic_content_identity::provider_digest;

pub use edge::ARTIFACT_EDGE_SCHEMA_ID;
pub use edge::ARTIFACT_EDGE_SCHEMA_VERSION;
pub use edge::ArtifactRootEdge;
pub use edge::ArtifactRootEdgeInput;
pub use edge::build_artifact_root_edge;
pub use edge::hash_artifact_root_edge;
pub use identity::ARTIFACT_IDENTITY_SCHEMA_ID;
pub use identity::ARTIFACT_IDENTITY_SCHEMA_VERSION;
pub use identity::ArtifactChildRef;
pub use identity::ArtifactGeneration;
pub use identity::ArtifactHash;
pub use identity::ArtifactIdentityDocument;
pub use identity::ArtifactJson;
pub use identity::ArtifactKind;
pub use identity::ArtifactLeafInput;
pub use identity::ArtifactNodeInput;
pub use identity::ArtifactRepoId;
pub use identity::ArtifactRootInput;
pub use identity::ArtifactRootRef;
pub use identity::ArtifactScopeId;
pub use identity::ArtifactWorkspaceId;
pub use identity::EDGE_DOMAIN_V1;
pub use identity::HASH_ALGORITHM_BLAKE3;
pub use identity::JSON_DOMAIN_V1;
pub use identity::LEAF_DOMAIN_V1;
pub use identity::NODE_DOMAIN_V1;
pub use identity::ROOT_DOMAIN_V1;
pub use identity::hash_leaf;
pub use identity::hash_node;
pub use identity::hash_normalized_json;
pub use identity::hash_root;
pub use repair_chain::REPAIR_CHAIN_FRAME_SCHEMA_ID;
pub use repair_chain::REPAIR_CHAIN_FRAME_SCHEMA_VERSION;
pub use repair_chain::RepairChainFrame;
pub use repair_chain::RepairChainFrameIdentity;
pub use repair_chain::RepairChainFrameInput;
pub use repair_chain::RepairChainFrameKind;
pub use repair_chain::RepairChainParentRef;
pub use repair_chain::build_repair_chain_frame;
pub use state_home_binding::HostProjectReference;
pub use state_home_binding::PROJECT_BINDING_SCHEMA_ID;
pub use state_home_binding::PROJECT_BINDING_SCHEMA_VERSION;
pub use state_home_binding::ProjectBinding;
pub use state_home_binding::RepoIdentity;
pub use state_home_binding::WorkspaceIdentity;
pub use state_home_catalog::CatalogBatchReceipt;
pub use state_home_catalog::CatalogGeneration;
pub use state_home_catalog::CatalogObservation;
pub use state_home_catalog::CatalogObservationReceipt;
pub use state_home_catalog::STATE_HOME_CATALOG_SCHEMA_ID;
pub use state_home_catalog::STATE_HOME_CATALOG_SCHEMA_VERSION;
pub use state_home_catalog::admit_state_home_catalog_batch;
pub use state_home_catalog::validate_state_home_catalog_observations;
pub use state_home_layout::StateHomeLayout;
pub use state_home_layout::WorkspaceStatePaths;
pub use state_home_retention::CleanupDisposition;
pub use state_home_retention::CleanupPlan;
pub use state_home_retention::CleanupPlanEntry;
pub use state_home_retention::RETENTION_PLAN_SCHEMA_ID;
pub use state_home_retention::RETENTION_PLAN_SCHEMA_VERSION;
pub use state_home_retention::RetainedObject;
pub use state_home_retention::RetentionLease;
pub use state_home_retention::RetentionObjectKind;
pub use state_home_retention::RetentionPlanner;

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
