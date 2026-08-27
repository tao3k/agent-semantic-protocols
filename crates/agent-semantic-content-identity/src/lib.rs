//! Canonical content identity primitives for semantic artifacts and source snapshots.
//!
//! The crate separates stable domain tokens and value objects from artifact models,
//! deterministic hashing, and source-snapshot evidence. Public APIs remain available
//! from the crate root while each implementation branch retains a single owner.

pub mod canonical_item_identity;
mod content_addressed_path;
mod derived_artifact_evidence;
mod domain;
mod file_artifact;
mod hashing;
mod model;
mod schema_contract_identity;
mod source_snapshot;
pub mod structural_selector;
mod value;

pub use active_artifact_merkle::ActiveArtifactSetId;
pub use canonical_item_identity::{
    CANONICAL_ITEM_SELECTOR_SCHEMA_ID, CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION,
    CanonicalItemIdentity, CanonicalItemKind, CanonicalItemLanguageId, CanonicalItemScope,
    CanonicalItemScopeKind, CanonicalItemScopeRelation, CanonicalItemScopeSymbol,
    CanonicalItemSelector, CanonicalItemSymbol,
};
pub use content_addressed_path::blake3_digest_from_canonical_artifact_path;

pub use derived_artifact_evidence::{
    DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION, DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID,
    DerivedArtifactAuthorityState, DerivedSourceArtifactEvidence, DerivedSourceArtifactKind,
};
pub use domain::{
    ARTIFACT_IDENTITY_SCHEMA_ID, ARTIFACT_IDENTITY_SCHEMA_VERSION, EDGE_DOMAIN_V1,
    HASH_ALGORITHM_BLAKE3, JSON_DOMAIN_V1, LEAF_DOMAIN_V1, NODE_DOMAIN_V1, ROOT_DOMAIN_V1,
};
pub use file_artifact::{
    FileArtifactMetadataV1, file_artifact_metadata_digest_v1, file_artifact_metadata_v1,
    file_content_digest_v1,
};
pub use hashing::{
    DerivedArtifactKeyInput, hash_blob, hash_derived_artifact_key, hash_leaf, hash_node,
    hash_normalized_json, hash_root,
};
pub use model::{
    ArtifactChildRef, ArtifactIdentityDocument, ArtifactLeafInput, ArtifactNodeInput,
    ArtifactRootInput, ArtifactRootRef,
};
pub use schema_contract_identity::{SchemaContractIdentity, schema_contract_identities};
pub use source_snapshot::{
    ResolutionAuthority, ResolutionEvidence, ResolutionState, SOURCE_RESOLUTION_SCHEMA_ID,
    SOURCE_SNAPSHOT_ALGORITHM, SOURCE_SNAPSHOT_SCHEMA_ID, SnapshotBoundResolution,
    SourceSnapshotEvidence, SourceSnapshotKind, WorkspaceSnapshot, provider_digest,
};
pub use value::{
    ArtifactGeneration, ArtifactHash, ArtifactJson, ArtifactKind, ArtifactRepoId, ArtifactScopeId,
    ArtifactWorkspaceId,
};
#[cfg(test)]
#[path = "../tests/unit/derived_artifact_evidence.rs"]
mod derived_artifact_evidence_tests;

#[cfg(test)]
#[path = "../tests/unit/source_snapshot.rs"]
mod source_snapshot_tests;

#[cfg(test)]
#[path = "../tests/unit/source_snapshot_contract.rs"]
mod source_snapshot_contract_tests;

#[cfg(test)]
#[path = "../tests/unit/schema_contract_identity.rs"]
mod schema_contract_identity_tests;
mod store;

pub use store::ContentAddressedStore;
#[cfg(test)]
#[path = "../tests/unit/store.rs"]
mod store_tests;

#[cfg(test)]
#[path = "../tests/unit/hashing.rs"]
mod hashing_tests;

pub mod active_artifact_merkle;

#[cfg(test)]
#[path = "../tests/unit/canonical_item_identity.rs"]
mod canonical_item_identity_tests;
pub mod exact_selector_cache;
pub use exact_selector_cache::ExactSelectorProjectionRecordV1;
pub mod exact_selector_generation_fixture;
pub use exact_selector_generation_fixture::{
    ExactSelectorGenerationRecordV1, ExactSelectorMaterializationProofErrorV1,
    ExactSelectorMaterializationProofV1, ExactSelectorMerkleProofSideV1,
    ExactSelectorMerkleProofStepV1, ExactSelectorProjectionModeV1,
};
pub mod callable_skeleton_projection;
pub mod exact_selector_merkle;
pub mod exact_selector_projection_packet;
pub mod exact_structural_selector;
#[cfg(test)]
#[path = "../tests/unit/overlay.rs"]
mod overlay_tests;
pub mod projection_evidence_context;
pub mod provider_projection_relation;
pub mod semantic_projection;
pub mod workspace_generation_evidence;
pub mod workspace_memory_generation_segment;
pub mod workspace_merkle_v1;
pub mod workspace_search_identity;
extern crate self as agent_semantic_content_identity;
