//! Canonical content identity primitives for semantic artifacts and source snapshots.
//!
//! The crate separates stable domain tokens and value objects from artifact models,
//! deterministic hashing, and source-snapshot evidence. Public APIs remain available
//! from the crate root while each implementation branch retains a single owner.

mod derived_artifact_evidence;
mod domain;
mod hashing;
mod model;
mod source_snapshot;
mod value;

pub use derived_artifact_evidence::{
    DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION, DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID,
    DerivedArtifactAuthorityState, DerivedSourceArtifactEvidence, DerivedSourceArtifactKind,
};
pub use domain::{
    ARTIFACT_IDENTITY_SCHEMA_ID, ARTIFACT_IDENTITY_SCHEMA_VERSION, EDGE_DOMAIN_V1,
    HASH_ALGORITHM_BLAKE3, JSON_DOMAIN_V1, LEAF_DOMAIN_V1, NODE_DOMAIN_V1, ROOT_DOMAIN_V1,
};
pub use hashing::{
    DerivedArtifactKeyInput, hash_blob, hash_derived_artifact_key, hash_leaf, hash_node,
    hash_normalized_json, hash_root,
};
pub use model::{
    ArtifactChildRef, ArtifactIdentityDocument, ArtifactLeafInput, ArtifactNodeInput,
    ArtifactRootInput, ArtifactRootRef,
};
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
mod store;

pub use store::ContentAddressedStore;
#[cfg(test)]
#[path = "../tests/unit/store.rs"]
mod store_tests;

#[cfg(test)]
#[path = "../tests/unit/hashing.rs"]
mod hashing_tests;

pub mod active_artifact_merkle_v1;
pub mod canonical_item_identity;

#[cfg(test)]
#[path = "../tests/unit/canonical_item_identity.rs"]
mod canonical_item_identity_tests;
pub mod exact_selector_cache;
pub mod exact_selector_merkle;
pub mod exact_selector_projection_packet;
#[cfg(test)]
#[path = "../tests/unit/overlay.rs"]
mod overlay_tests;
pub mod workspace_merkle_v1;
extern crate self as agent_semantic_content_identity;
