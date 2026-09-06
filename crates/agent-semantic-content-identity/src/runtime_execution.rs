// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Binds Runtime execution to exact project, workspace, artifact, and generation identities.

use serde::Deserialize;
use serde::Serialize;

use crate::Blake3DigestV1;
use crate::content_binding::ContentBinding;
use crate::content_binding::ContentBindingError;

/// Schema identifier for an exact Runtime execution binding.
pub const RUNTIME_EXECUTION_SCHEMA_ID: &str = "asp.runtime-execution-binding";
/// Schema version for Runtime execution bindings.
pub const RUNTIME_EXECUTION_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Exact identities that a Runtime operation must preserve end to end.
pub struct RuntimeExecutionBinding {
    pub schema_id: String,
    pub schema_version: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub workspace_id: String,
    #[serde(default)]
    pub publication_nonce: String,
    pub content_binding: ContentBinding,
    pub runtime_artifact_digest: Blake3DigestV1,
    pub evaluator_policy_digest: Blake3DigestV1,
    pub active_artifact_receipt_digest: Blake3DigestV1,
    pub evaluator_abi_digest: Blake3DigestV1,
}

/// Named inputs required to construct a Runtime execution binding.
pub struct RuntimeExecutionBindingInput {
    pub project_id: String,
    pub workspace_id: String,
    pub publication_nonce: String,
    pub content_binding: ContentBinding,
    pub runtime_artifact_digest: Blake3DigestV1,
    pub evaluator_policy_digest: Blake3DigestV1,
    pub active_artifact_receipt_digest: Blake3DigestV1,
    pub evaluator_abi_digest: Blake3DigestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Deterministic mismatch in a Runtime execution binding.
pub enum RuntimeExecutionBindingError {
    Binding(ContentBindingError),
    SchemaMismatch,
    MissingIdentity { field: &'static str },
    InvalidDigest { field: &'static str },
    RuntimeArtifactMismatch,
    ContentMismatch,
}

impl RuntimeExecutionBinding {
    pub fn new(input: RuntimeExecutionBindingInput) -> Result<Self, RuntimeExecutionBindingError> {
        let RuntimeExecutionBindingInput {
            project_id,
            workspace_id,
            publication_nonce,
            content_binding,
            runtime_artifact_digest,
            evaluator_policy_digest,
            active_artifact_receipt_digest,
            evaluator_abi_digest,
        } = input;
        let binding = Self {
            schema_id: RUNTIME_EXECUTION_SCHEMA_ID.into(),
            schema_version: RUNTIME_EXECUTION_SCHEMA_VERSION.into(),
            project_id,
            workspace_id,
            publication_nonce,
            content_binding,
            runtime_artifact_digest,
            evaluator_policy_digest,
            active_artifact_receipt_digest,
            evaluator_abi_digest,
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
            ("projectId", self.project_id.as_str()),
            ("workspaceId", self.workspace_id.as_str()),
            ("publicationNonce", self.publication_nonce.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(RuntimeExecutionBindingError::MissingIdentity { field });
            }
        }
        for (field, value) in [
            (
                "runtimeArtifactDigest",
                self.runtime_artifact_digest.as_str(),
            ),
            (
                "evaluatorPolicyDigest",
                self.evaluator_policy_digest.as_str(),
            ),
            (
                "activeArtifactReceiptDigest",
                self.active_artifact_receipt_digest.as_str(),
            ),
            ("evaluatorAbiDigest", self.evaluator_abi_digest.as_str()),
        ] {
            if !is_content_digest(value) {
                return Err(RuntimeExecutionBindingError::InvalidDigest { field });
            }
        }
        if self.runtime_artifact_digest.as_str()
            != self.content_binding.identity.runtime_artifact_digest
        {
            return Err(RuntimeExecutionBindingError::RuntimeArtifactMismatch);
        }
        Ok(())
    }

    /// Returns whether a decodable V1 predecessor needs canonical Runtime
    /// publication before it can be admitted for execution.
    pub fn refresh_required(&self) -> bool {
        self.project_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.publication_nonce.trim().is_empty()
    }

    pub fn digest(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        for value in [
            self.project_id.clone(),
            self.workspace_id.clone(),
            self.publication_nonce.clone(),
            self.content_binding.identity.digest(),
            self.runtime_artifact_digest.to_string(),
            self.evaluator_policy_digest.to_string(),
            self.active_artifact_receipt_digest.to_string(),
            self.evaluator_abi_digest.to_string(),
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
