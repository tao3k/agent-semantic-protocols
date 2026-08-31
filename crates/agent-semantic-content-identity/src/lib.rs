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
pub mod content_binding;
pub mod search_execution;

pub mod host_session_binding {
    use serde::{Deserialize, Serialize};

    pub const HOST_SESSION_SCHEMA_ID: &str = "asp.host-session-binding";
    pub const HOST_SESSION_SCHEMA_VERSION: &str = "1";

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct HostSessionBinding {
        pub schema_id: String,
        pub schema_version: String,
        pub platform: String,
        pub root_session_id: String,
        pub parent_session_id: String,
        pub current_session_id: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum HostSessionBindingError {
        SchemaMismatch,
        MissingField(&'static str),
        ParentEqualsCurrent,
        ConflictingIdentity,
    }

    impl HostSessionBinding {
        pub fn new(
            platform: impl Into<String>,
            root_session_id: impl Into<String>,
            parent_session_id: impl Into<String>,
            current_session_id: impl Into<String>,
        ) -> Result<Self, HostSessionBindingError> {
            let binding = Self {
                schema_id: HOST_SESSION_SCHEMA_ID.into(),
                schema_version: HOST_SESSION_SCHEMA_VERSION.into(),
                platform: platform.into(),
                root_session_id: root_session_id.into(),
                parent_session_id: parent_session_id.into(),
                current_session_id: current_session_id.into(),
            };
            binding.validate()?;
            Ok(binding)
        }

        pub fn validate(&self) -> Result<(), HostSessionBindingError> {
            if self.schema_id != HOST_SESSION_SCHEMA_ID
                || self.schema_version != HOST_SESSION_SCHEMA_VERSION
            {
                return Err(HostSessionBindingError::SchemaMismatch);
            }
            for (name, value) in [
                ("platform", &self.platform),
                ("rootSessionId", &self.root_session_id),
                ("parentSessionId", &self.parent_session_id),
                ("currentSessionId", &self.current_session_id),
            ] {
                if value.is_empty() {
                    return Err(HostSessionBindingError::MissingField(name));
                }
            }
            if self.parent_session_id == self.current_session_id {
                return Err(HostSessionBindingError::ParentEqualsCurrent);
            }
            Ok(())
        }

        pub fn from_environment<F>(mut read: F) -> Result<Self, HostSessionBindingError>
        where
            F: FnMut(&str) -> Option<String>,
        {
            let platform = read("ASP_HOST_PLATFORM")
                .ok_or(HostSessionBindingError::MissingField("platform"))?;
            let root = read("ASP_ROOT_SESSION_ID")
                .ok_or(HostSessionBindingError::MissingField("rootSessionId"))?;
            let parent = read("CODEX_SESSION_ID")
                .ok_or(HostSessionBindingError::MissingField("parentSessionId"))?;
            let current = read("CODEX_THREAD_ID")
                .ok_or(HostSessionBindingError::MissingField("currentSessionId"))?;
            Self::new(platform, root, parent, current)
        }
    }
}

pub mod runtime_execution {
    use serde::{Deserialize, Serialize};

    use crate::content_binding::{ContentBinding, ContentBindingError};

    pub const RUNTIME_EXECUTION_SCHEMA_ID: &str = "asp.runtime-execution-binding";
    pub const RUNTIME_EXECUTION_SCHEMA_VERSION: &str = "1";

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RuntimeExecutionBinding {
        pub schema_id: String,
        pub schema_version: String,
        pub content_binding: ContentBinding,
        pub runtime_artifact_digest: String,
        pub evaluator_policy_digest: String,
        pub active_artifact_receipt_digest: String,
        pub evaluator_abi_digest: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RuntimeExecutionBindingError {
        Binding(ContentBindingError),
        SchemaMismatch,
        InvalidDigest { field: &'static str },
        ContentMismatch,
    }

    impl RuntimeExecutionBinding {
        pub fn new(
            content_binding: ContentBinding,
            runtime_artifact_digest: impl Into<String>,
            evaluator_policy_digest: impl Into<String>,
            active_artifact_receipt_digest: impl Into<String>,
            evaluator_abi_digest: impl Into<String>,
        ) -> Result<Self, RuntimeExecutionBindingError> {
            let binding = Self {
                schema_id: RUNTIME_EXECUTION_SCHEMA_ID.into(),
                schema_version: RUNTIME_EXECUTION_SCHEMA_VERSION.into(),
                content_binding,
                runtime_artifact_digest: runtime_artifact_digest.into(),
                evaluator_policy_digest: evaluator_policy_digest.into(),
                active_artifact_receipt_digest: active_artifact_receipt_digest.into(),
                evaluator_abi_digest: evaluator_abi_digest.into(),
            };
            binding.validate()?;
            Ok(binding)
        }

        pub fn validate(&self) -> Result<(), RuntimeExecutionBindingError> {
            if self.schema_id != RUNTIME_EXECUTION_SCHEMA_ID
                || self.schema_version != RUNTIME_EXECUTION_SCHEMA_VERSION
            {
                return Err(RuntimeExecutionBindingError::SchemaMismatch);
            }
            self.content_binding
                .validate()
                .map_err(RuntimeExecutionBindingError::Binding)?;
            for (field, value) in [
                ("runtimeArtifactDigest", &self.runtime_artifact_digest),
                ("evaluatorPolicyDigest", &self.evaluator_policy_digest),
                (
                    "activeArtifactReceiptDigest",
                    &self.active_artifact_receipt_digest,
                ),
                ("evaluatorAbiDigest", &self.evaluator_abi_digest),
            ] {
                if !is_content_digest(value) {
                    return Err(RuntimeExecutionBindingError::InvalidDigest { field });
                }
            }
            Ok(())
        }

        pub fn digest(&self) -> String {
            let mut hasher = blake3::Hasher::new();
            for value in [
                self.content_binding.identity.digest(),
                self.runtime_artifact_digest.clone(),
                self.evaluator_policy_digest.clone(),
                self.active_artifact_receipt_digest.clone(),
                self.evaluator_abi_digest.clone(),
            ] {
                hasher.update(value.as_bytes());
                hasher.update(&[0]);
            }
            format!("blake3-256:{}", hasher.finalize().to_hex())
        }

        pub fn admits(&self, other: &Self) -> Result<(), RuntimeExecutionBindingError> {
            self.validate()?;
            other.validate()?;
            if self != other {
                return Err(RuntimeExecutionBindingError::ContentMismatch);
            }
            Ok(())
        }
    }

    fn is_content_digest(value: &str) -> bool {
        let Some(hex) = value.strip_prefix("blake3-256:") else {
            return false;
        };
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    }
}
