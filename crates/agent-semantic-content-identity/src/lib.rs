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
/// Provider-neutral project-resolution receipts and immutable package-graph facts.
mod project_resolution;
mod schema_contract_identity;
pub mod semantic_ids;
mod source_snapshot;
pub mod structural_selector;
mod value;

pub use active_artifact_merkle::ActiveArtifactSetId;
pub use canonical_item_identity::CANONICAL_ITEM_SELECTOR_SCHEMA_ID;
pub use canonical_item_identity::CANONICAL_ITEM_SELECTOR_SCHEMA_VERSION;
pub use canonical_item_identity::CanonicalItemIdentity;
pub use canonical_item_identity::CanonicalItemKind;
pub use canonical_item_identity::CanonicalItemLanguageId;
pub use canonical_item_identity::CanonicalItemScope;
pub use canonical_item_identity::CanonicalItemScopeKind;
pub use canonical_item_identity::CanonicalItemScopeRelation;
pub use canonical_item_identity::CanonicalItemScopeSymbol;
pub use canonical_item_identity::CanonicalItemSelector;
pub use canonical_item_identity::CanonicalItemSymbol;
pub use content_addressed_path::blake3_digest_from_canonical_artifact_path;

pub use derived_artifact_evidence::DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION;
pub use derived_artifact_evidence::DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID;
pub use derived_artifact_evidence::DerivedArtifactAuthorityState;
pub use derived_artifact_evidence::DerivedSourceArtifactEvidence;
pub use derived_artifact_evidence::DerivedSourceArtifactKind;
pub use domain::ARTIFACT_IDENTITY_SCHEMA_ID;
pub use domain::ARTIFACT_IDENTITY_SCHEMA_VERSION;
pub use domain::EDGE_DOMAIN_V1;
pub use domain::HASH_ALGORITHM_BLAKE3;
pub use domain::JSON_DOMAIN_V1;
pub use domain::LEAF_DOMAIN_V1;
pub use domain::NODE_DOMAIN_V1;
pub use domain::ROOT_DOMAIN_V1;
pub use file_artifact::FileArtifactMetadataV1;
pub use file_artifact::file_artifact_metadata_digest_v1;
pub use file_artifact::file_artifact_metadata_v1;
pub use file_artifact::file_content_digest_v1;
pub use hashing::DerivedArtifactKeyInput;
pub use hashing::hash_blob;
pub use hashing::hash_derived_artifact_key;
pub use hashing::hash_leaf;
pub use hashing::hash_node;
pub use hashing::hash_normalized_json;
pub use hashing::hash_root;
pub use model::ArtifactChildRef;
pub use model::ArtifactIdentityDocument;
pub use model::ArtifactLeafInput;
pub use model::ArtifactNodeInput;
pub use model::ArtifactRootInput;
pub use model::ArtifactRootRef;
pub use project_resolution::AdmittedProjectResolution;
pub use project_resolution::ExternalDependency;
pub use project_resolution::InternalDependencyEdge;
pub use project_resolution::LANGUAGE_PACKAGE_GRAPH_SCHEMA_ID;
pub use project_resolution::LanguagePackage;
pub use project_resolution::LanguagePackageGraph;
pub use project_resolution::LanguageTarget;
pub use project_resolution::PROJECT_RESOLUTION_SCHEMA_ID;
pub use project_resolution::ProjectFile;
pub use project_resolution::ProjectResolutionConflict;
pub use project_resolution::ProjectResolutionMetrics;
pub use project_resolution::ProjectResolutionReceipt;
pub use project_resolution::ResolvedSourceExclusion;
pub use project_resolution::ResolvedSourceScope;
pub use project_resolution::UnresolvedProjectReference;
pub use project_resolution::project_resolution_schema_digest;
pub use project_resolution::workspace_source_scope_generation_digest;
pub use schema_contract_identity::SchemaContractIdentity;
pub use schema_contract_identity::schema_contract_identities;
pub use semantic_ids::Blake3DigestV1;
pub use semantic_ids::HostPlatformV1;
pub use semantic_ids::HostSessionIdV1;
pub use semantic_ids::LanguageIdV1;
pub use semantic_ids::ProjectionEvidenceContextRefV1;
pub use semantic_ids::ProviderIdV1;
pub use semantic_ids::ProviderRelationEndpointIdV1;
pub use semantic_ids::ProviderRelationEndpointKindV1;
pub use semantic_ids::ProviderRelationKindV1;
pub use semantic_ids::SchemaIdV1;
pub use semantic_ids::SemanticProjectionKindV1;
pub use semantic_ids::SemanticProjectionRootSelectorV1;
pub use source_snapshot::ResolutionAuthority;
pub use source_snapshot::ResolutionEvidence;
pub use source_snapshot::ResolutionState;
pub use source_snapshot::SOURCE_RESOLUTION_SCHEMA_ID;
pub use source_snapshot::SOURCE_SNAPSHOT_ALGORITHM;
pub use source_snapshot::SOURCE_SNAPSHOT_SCHEMA_ID;
pub use source_snapshot::SnapshotBoundResolution;
pub use source_snapshot::SourceSnapshotEvidence;
pub use source_snapshot::SourceSnapshotKind;
pub use source_snapshot::WorkspaceOverlayPaths;
pub use source_snapshot::WorkspaceSnapshot;
pub use source_snapshot::provider_digest;
pub use value::ArtifactGeneration;
pub use value::ArtifactHash;
pub use value::ArtifactJson;
pub use value::ArtifactKind;
pub use value::ArtifactRepoId;
pub use value::ArtifactScopeId;
pub use value::ArtifactWorkspaceId;
#[cfg(test)]
#[path = "../tests/unit/derived_artifact_evidence.rs"]
mod derived_artifact_evidence_tests;

#[cfg(test)]
#[path = "../tests/unit/source_snapshot.rs"]
mod source_snapshot_tests;

#[cfg(test)]
#[path = "../tests/unit/project_resolution.rs"]
mod project_resolution_tests;

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
pub use exact_selector_generation_fixture::ExactSelectorGenerationRecordV1;
pub use exact_selector_generation_fixture::ExactSelectorLanguageIdV1;
pub use exact_selector_generation_fixture::ExactSelectorMaterializationProofErrorV1;
pub use exact_selector_generation_fixture::ExactSelectorMaterializationProofV1;
pub use exact_selector_generation_fixture::ExactSelectorMerkleProofSideV1;
pub use exact_selector_generation_fixture::ExactSelectorMerkleProofStepV1;
pub use exact_selector_generation_fixture::ExactSelectorOwnerPathV1;
pub use exact_selector_generation_fixture::ExactSelectorProjectionModeV1;
pub use exact_selector_generation_fixture::ExactSelectorProviderIdV1;
pub mod callable_skeleton_projection;
pub mod exact_selector_merkle;
pub use exact_selector_merkle::ContentDigestV1;
pub use exact_selector_merkle::MerkleInclusionSideV1;
pub use exact_selector_merkle::MerkleInclusionStepV1;
pub(crate) use exact_selector_merkle::canonical_digest_v1;
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
pub mod content_binding;
pub mod search_execution;

pub mod host_session_binding;

pub mod runtime_execution;
