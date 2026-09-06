// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Exact content binding primitives for the Server-owned publication model.
//!
//! Protocol versioning belongs to the shared schema. This Rust module keeps
//! the type namespace stable while exposing the schema version as a constant.

use serde::Deserialize;
use serde::Serialize;

/// Schema identifier for a complete Runtime content binding.
pub const CONTENT_BINDING_SCHEMA_ID: &str = "asp.content-binding";
/// Schema version for Runtime content bindings.
pub const CONTENT_BINDING_SCHEMA_VERSION: &str = "1";
const DIGEST_PREFIX: &str = "blake3-256:";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Immutable Runtime artifact identity admitted for execution.
pub struct RuntimeArtifactReference {
    pub schema_digest: String,
    pub artifact_digest: String,
    pub provider_contract_digest: String,
    pub signer_key_id: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Immutable workspace snapshot identity admitted for execution.
pub struct WorkspaceSnapshotReference {
    pub workspace_id: String,
    pub workspace_catalog_digest: String,
    pub snapshot_digest: String,
    pub source_root_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Source-index generation identity bound to its snapshot root.
pub struct SourceGenerationReference {
    pub source_generation_id: String,
    pub source_snapshot_digest: String,
    pub source_index_digest: String,
    pub provider_id: String,
    pub provider_contract_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Authority and publication epoch that linearized a binding.
pub struct AuthorityStamp {
    pub key_id: String,
    pub canonical_digest: String,
    pub signature: String,
}

/// The schema-shaped binding envelope. The six fields remain in the stable
/// `ContentIdentity` type so callers cannot accidentally use an authority
/// stamp as content identity; this envelope is the JSON contract represented
/// by `schemas/content-binding.schema.json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContentBinding {
    #[serde(rename = "schemaId")]
    pub schema_id: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(flatten)]
    pub identity: ContentIdentity,
    #[serde(rename = "authorityStamp")]
    pub authority_stamp: AuthorityStamp,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Canonical digest and complete component references for one content state.
pub struct ContentIdentity {
    pub runtime_artifact_digest: String,
    pub workspace_snapshot_digest: String,
    pub source_generation_digest: String,
    pub source_index_digest: String,
    pub schema_digest: String,
    pub provider_catalog_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Linearized publication commit for a canonical content identity.
pub struct ContentPublicationCommit {
    pub identity: ContentIdentity,
    pub commit_digest: String,
    pub authority_stamp: AuthorityStamp,
    pub mutation_id: String,
    pub lease_id: String,
    pub expected_digest: Option<String>,
    pub durable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Read-only observation checked against a publication commit.
pub struct ActivationObservation {
    pub publication_nonce: String,
    pub commit_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Deterministic failure while validating or admitting a content binding.
pub enum ContentBindingError {
    InvalidDigest { field: &'static str },
    NonDurableCommit,
    ContentMismatch,
    CommitDigestMismatch,
    InvalidCommitFence,
    RollbackRequiresAuthority,
}

impl ContentIdentity {
    pub fn validate(&self) -> Result<(), ContentBindingError> {
        for (field, digest) in [
            ("runtimeArtifactDigest", &self.runtime_artifact_digest),
            ("workspaceSnapshotDigest", &self.workspace_snapshot_digest),
            ("sourceGenerationDigest", &self.source_generation_digest),
            ("sourceIndexDigest", &self.source_index_digest),
            ("schemaDigest", &self.schema_digest),
            ("providerCatalogDigest", &self.provider_catalog_digest),
        ] {
            if !is_digest(digest) {
                return Err(ContentBindingError::InvalidDigest { field });
            }
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("ContentIdentity is serializable")
    }

    pub fn digest(&self) -> String {
        format!(
            "{DIGEST_PREFIX}{}",
            blake3::hash(&self.canonical_bytes()).to_hex()
        )
    }

    pub fn from_references(
        artifact: &RuntimeArtifactReference,
        workspace: &WorkspaceSnapshotReference,
        source: &SourceGenerationReference,
        provider_catalog_digest: String,
    ) -> Result<Self, ContentBindingError> {
        artifact.validate()?;
        workspace.validate()?;
        source.validate()?;
        if artifact.provider_contract_digest != source.provider_contract_digest {
            return Err(ContentBindingError::ContentMismatch);
        }
        if workspace.snapshot_digest != source.source_snapshot_digest {
            return Err(ContentBindingError::ContentMismatch);
        }
        let identity = Self {
            runtime_artifact_digest: artifact.artifact_digest.clone(),
            workspace_snapshot_digest: workspace.snapshot_digest.clone(),
            source_generation_digest: source.digest(),
            source_index_digest: source.source_index_digest.clone(),
            schema_digest: artifact.schema_digest.clone(),
            provider_catalog_digest,
        };
        identity.validate()?;
        Ok(identity)
    }
}

impl RuntimeArtifactReference {
    fn validate(&self) -> Result<(), ContentBindingError> {
        validate_digest("schemaDigest", &self.schema_digest)?;
        validate_digest("artifactDigest", &self.artifact_digest)?;
        validate_digest("providerContractDigest", &self.provider_contract_digest)?;
        if self.signer_key_id.is_empty() || self.signature.is_empty() {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        Ok(())
    }
}

impl WorkspaceSnapshotReference {
    fn validate(&self) -> Result<(), ContentBindingError> {
        if self.workspace_id.is_empty() {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        validate_digest("workspaceCatalogDigest", &self.workspace_catalog_digest)?;
        validate_digest("snapshotDigest", &self.snapshot_digest)?;
        validate_digest("sourceRootDigest", &self.source_root_digest)
    }
}

impl SourceGenerationReference {
    fn validate(&self) -> Result<(), ContentBindingError> {
        if self.source_generation_id.is_empty() || self.provider_id.is_empty() {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        validate_digest("sourceSnapshotDigest", &self.source_snapshot_digest)?;
        validate_digest("sourceIndexDigest", &self.source_index_digest)?;
        validate_digest("providerContractDigest", &self.provider_contract_digest)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("SourceGenerationReference is serializable")
    }

    pub fn digest(&self) -> String {
        format!(
            "{DIGEST_PREFIX}{}",
            blake3::hash(&self.canonical_bytes()).to_hex()
        )
    }
}

impl ContentBinding {
    pub fn new(
        identity: ContentIdentity,
        authority_stamp: AuthorityStamp,
    ) -> Result<Self, ContentBindingError> {
        identity.validate()?;
        authority_stamp.validate_for(&identity.digest())?;
        Ok(Self {
            schema_id: CONTENT_BINDING_SCHEMA_ID.to_owned(),
            schema_version: CONTENT_BINDING_SCHEMA_VERSION.to_owned(),
            identity,
            authority_stamp,
        })
    }

    pub fn validate(&self) -> Result<(), ContentBindingError> {
        if self.schema_id != CONTENT_BINDING_SCHEMA_ID
            || self.schema_version != CONTENT_BINDING_SCHEMA_VERSION
        {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        self.identity.validate()?;
        self.authority_stamp.validate_for(&self.identity.digest())
    }
}

impl AuthorityStamp {
    pub fn validate_for(&self, canonical_digest: &str) -> Result<(), ContentBindingError> {
        if self.key_id.is_empty() || self.signature.is_empty() {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        validate_digest("canonicalDigest", &self.canonical_digest)?;
        if self.canonical_digest != canonical_digest {
            return Err(ContentBindingError::ContentMismatch);
        }
        Ok(())
    }
}

impl ContentPublicationCommit {
    pub fn linearize(
        identity: ContentIdentity,
        authority_stamp: AuthorityStamp,
    ) -> Result<Self, ContentBindingError> {
        Self::linearize_with_expected(identity, authority_stamp, None)
    }

    pub fn linearize_with_expected(
        identity: ContentIdentity,
        authority_stamp: AuthorityStamp,
        expected_digest: Option<&str>,
    ) -> Result<Self, ContentBindingError> {
        identity.validate()?;
        authority_stamp.validate_for(&identity.digest())?;
        let commit_digest = identity.digest();
        if let Some(expected_digest) = expected_digest {
            validate_digest("expectedDigest", expected_digest)?;
        }
        let fence_identity = expected_digest.unwrap_or("genesis");
        Ok(Self {
            authority_stamp,
            mutation_id: format!("mutation:{commit_digest}:{fence_identity}"),
            lease_id: format!("lease:{commit_digest}:{fence_identity}"),
            expected_digest: expected_digest.map(str::to_owned),
            identity,
            commit_digest,
            durable: true,
        })
    }

    pub fn validate(&self) -> Result<(), ContentBindingError> {
        if !self.durable {
            return Err(ContentBindingError::NonDurableCommit);
        }
        self.identity.validate()?;
        if self.commit_digest != self.identity.digest() {
            return Err(ContentBindingError::CommitDigestMismatch);
        }
        self.authority_stamp.validate_for(&self.identity.digest())?;
        if self.mutation_id.is_empty() || self.lease_id.is_empty() {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        let fence_suffix = self.expected_digest.as_deref().unwrap_or("genesis");
        let expected_mutation_id = format!("mutation:{}:{}", self.commit_digest, fence_suffix);
        let expected_lease_id = format!("lease:{}:{}", self.commit_digest, fence_suffix);
        if self.mutation_id != expected_mutation_id || self.lease_id != expected_lease_id {
            return Err(ContentBindingError::InvalidCommitFence);
        }
        if let Some(expected_digest) = self.expected_digest.as_deref() {
            validate_digest("expectedDigest", expected_digest)?;
        }
        Ok(())
    }

    pub fn admit_exact(&self, requested: &ContentIdentity) -> Result<(), ContentBindingError> {
        self.validate()?;
        requested.validate()?;
        if &self.identity != requested {
            return Err(ContentBindingError::ContentMismatch);
        }
        Ok(())
    }

    pub fn rollback_without_authority(&self) -> Result<(), ContentBindingError> {
        Err(ContentBindingError::RollbackRequiresAuthority)
    }
}

impl ActivationObservation {
    pub fn is_product_authority(&self) -> bool {
        false
    }

    pub fn matches_commit(&self, commit: &ContentPublicationCommit) -> bool {
        commit.validate().is_ok() && self.commit_digest == commit.commit_digest
    }
}

fn is_digest(value: &str) -> bool {
    let hex = value.strip_prefix(DIGEST_PREFIX).unwrap_or_default();
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_digest(field: &'static str, value: &str) -> Result<(), ContentBindingError> {
    if is_digest(value) {
        Ok(())
    } else {
        Err(ContentBindingError::InvalidDigest { field })
    }
}

#[cfg(test)]
#[path = "../tests/unit/content_binding.rs"]
mod tests;
