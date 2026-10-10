// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Immutable publication joining one source generation to one Runtime product.

use serde::Deserialize;
use serde::Serialize;

use crate::Blake3DigestV1;
use crate::content_binding::ContentBindingError;
use crate::content_binding::ContentPublicationCommit;
use crate::runtime_execution::RuntimeExecutionBinding;
use crate::runtime_execution::RuntimeExecutionBindingError;

/// Schema identifier for the workspace execution publication sidecar.
pub const RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-workspace-execution-publication";
/// Schema version for the workspace execution publication sidecar.
pub const RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Content-addressed product admitted by the canonical workspace pointer.
pub struct RuntimeWorkspaceExecutionPublication {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: Blake3DigestV1,
    pub source_root_digest: Blake3DigestV1,
    pub content_publication_commit: ContentPublicationCommit,
    pub runtime_execution_binding: RuntimeExecutionBinding,
    pub runtime_bundle_digest: Blake3DigestV1,
    pub publication_digest: Blake3DigestV1,
}

/// Named inputs whose canonical digest mints one immutable publication.
pub struct RuntimeWorkspaceExecutionPublicationInput {
    pub workspace_identity: String,
    pub generation_digest: Blake3DigestV1,
    pub source_root_digest: Blake3DigestV1,
    pub content_publication_commit: ContentPublicationCommit,
    pub runtime_execution_binding: RuntimeExecutionBinding,
    pub runtime_bundle_digest: Blake3DigestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Deterministic validation failure for the source/Runtime product join.
pub enum RuntimeWorkspaceExecutionPublicationError {
    RuntimeBinding(RuntimeExecutionBindingError),
    ContentCommit(ContentBindingError),
    SchemaMismatch,
    MissingIdentity { field: &'static str },
    InvalidDigest { field: &'static str },
    SourceGenerationMismatch,
    SourceRootMismatch,
    ContentCommitMismatch,
    PublicationDigestMismatch,
    ContentMismatch,
}

impl RuntimeWorkspaceExecutionPublication {
    /// Builds and content-addresses one exact source/Runtime product.
    pub fn new(
        input: RuntimeWorkspaceExecutionPublicationInput,
    ) -> Result<Self, RuntimeWorkspaceExecutionPublicationError> {
        let RuntimeWorkspaceExecutionPublicationInput {
            workspace_identity,
            generation_digest,
            source_root_digest,
            content_publication_commit,
            runtime_execution_binding,
            runtime_bundle_digest,
        } = input;
        content_publication_commit
            .validate()
            .map_err(RuntimeWorkspaceExecutionPublicationError::ContentCommit)?;
        runtime_execution_binding
            .validate()
            .map_err(RuntimeWorkspaceExecutionPublicationError::RuntimeBinding)?;
        let publication_digest = compute_publication_digest(
            &workspace_identity,
            generation_digest.as_str(),
            source_root_digest.as_str(),
            &content_publication_commit,
            &runtime_execution_binding,
            runtime_bundle_digest.as_str(),
        );
        let publication = Self {
            schema_id: RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_ID.into(),
            schema_version: RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_VERSION.into(),
            workspace_identity,
            generation_digest,
            source_root_digest,
            content_publication_commit,
            runtime_execution_binding,
            runtime_bundle_digest,
            publication_digest: publication_digest.into(),
        };
        publication.validate()?;
        Ok(publication)
    }

    /// Validates schema, every component identity, and the publication digest.
    pub fn validate(&self) -> Result<(), RuntimeWorkspaceExecutionPublicationError> {
        if self.schema_id != RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_ID
            || self.schema_version != RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_VERSION
        {
            return Err(RuntimeWorkspaceExecutionPublicationError::SchemaMismatch);
        }
        self.validate_product()?;
        if !is_content_digest(self.publication_digest.as_str()) {
            return Err(RuntimeWorkspaceExecutionPublicationError::InvalidDigest {
                field: "publicationDigest",
            });
        }
        if self.publication_digest.as_str() != self.digest() {
            return Err(RuntimeWorkspaceExecutionPublicationError::PublicationDigestMismatch);
        }
        Ok(())
    }

    /// Computes the domain-separated identity of this publication's inputs.
    pub fn digest(&self) -> String {
        compute_publication_digest(
            &self.workspace_identity,
            self.generation_digest.as_str(),
            self.source_root_digest.as_str(),
            &self.content_publication_commit,
            &self.runtime_execution_binding,
            self.runtime_bundle_digest.as_str(),
        )
    }

    /// Requires exact equality with the product selected by the canonical pointer.
    pub fn admits(&self, observed: &Self) -> Result<(), RuntimeWorkspaceExecutionPublicationError> {
        self.validate()?;
        observed.validate()?;
        if self != observed {
            return Err(RuntimeWorkspaceExecutionPublicationError::ContentMismatch);
        }
        Ok(())
    }

    fn validate_product(&self) -> Result<(), RuntimeWorkspaceExecutionPublicationError> {
        self.content_publication_commit
            .validate()
            .map_err(RuntimeWorkspaceExecutionPublicationError::ContentCommit)?;
        self.runtime_execution_binding
            .validate()
            .map_err(RuntimeWorkspaceExecutionPublicationError::RuntimeBinding)?;
        if self.workspace_identity.trim().is_empty() {
            return Err(RuntimeWorkspaceExecutionPublicationError::MissingIdentity {
                field: "workspaceIdentity",
            });
        }
        for (field, value) in [
            ("generationDigest", self.generation_digest.as_str()),
            ("sourceRootDigest", self.source_root_digest.as_str()),
            ("runtimeBundleDigest", self.runtime_bundle_digest.as_str()),
        ] {
            if !is_content_digest(value) {
                return Err(RuntimeWorkspaceExecutionPublicationError::InvalidDigest { field });
            }
        }
        let content_identity = &self.runtime_execution_binding.content_binding.identity;
        if self.content_publication_commit.content_binding
            != self.runtime_execution_binding.content_binding
        {
            return Err(RuntimeWorkspaceExecutionPublicationError::ContentCommitMismatch);
        }
        if self.generation_digest.as_str() != content_identity.source_generation_digest {
            return Err(RuntimeWorkspaceExecutionPublicationError::SourceGenerationMismatch);
        }
        if self.source_root_digest.as_str() != content_identity.workspace_snapshot_digest {
            return Err(RuntimeWorkspaceExecutionPublicationError::SourceRootMismatch);
        }
        Ok(())
    }
}

fn compute_publication_digest(
    workspace_identity: &str,
    generation_digest: &str,
    source_root_digest: &str,
    content_publication_commit: &ContentPublicationCommit,
    runtime_execution_binding: &RuntimeExecutionBinding,
    runtime_bundle_digest: &str,
) -> String {
    let runtime_digest = runtime_execution_binding.digest();
    let mut hasher = blake3::Hasher::new();
    for value in [
        RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_ID,
        RUNTIME_WORKSPACE_EXECUTION_PUBLICATION_SCHEMA_VERSION,
        workspace_identity,
        generation_digest,
        source_root_digest,
        content_publication_commit.commit_digest.as_str(),
        runtime_digest.as_str(),
        runtime_bundle_digest,
    ] {
        update_digest_field(&mut hasher, value);
    }
    format!("blake3-256:{}", hasher.finalize().to_hex())
}

fn update_digest_field(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(
        &u64::try_from(value.len())
            .expect("publication identity field length fits u64")
            .to_le_bytes(),
    );
    hasher.update(value.as_bytes());
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
