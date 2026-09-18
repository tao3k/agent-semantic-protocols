// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Binds Runtime execution to exact project, workspace, artifact, and generation identities.

use serde::Deserialize;
use serde::Serialize;

use crate::Blake3DigestV1;
use crate::ProjectWorkspaceBinding;
use crate::ProjectWorkspaceBindingError;
use crate::content_binding::ContentBinding;
use crate::content_binding::ContentBindingError;

/// Schema identifier for an exact Runtime execution binding.
pub const RUNTIME_EXECUTION_SCHEMA_ID: &str = "agent.semantic-protocols.runtime-execution-binding";
/// Schema version for Runtime execution bindings.
pub const RUNTIME_EXECUTION_SCHEMA_VERSION: &str = "2";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Exact identities that a Runtime operation must preserve end to end.
pub struct RuntimeExecutionBinding {
    pub schema_id: String,
    pub schema_version: String,
    pub project_workspace: ProjectWorkspaceBinding,
    pub worktree_instance_id: String,
    pub publication_nonce: String,
    pub content_binding: ContentBinding,
    pub runtime_artifact_digest: Blake3DigestV1,
    pub evaluator_policy_digest: Blake3DigestV1,
    pub active_artifact_receipt_digest: Blake3DigestV1,
    pub evaluator_abi_digest: Blake3DigestV1,
}

/// Named inputs required to construct a Runtime execution binding.
pub struct RuntimeExecutionBindingInput {
    pub project_workspace: ProjectWorkspaceBinding,
    pub worktree_instance_id: String,
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
    ProjectWorkspace(ProjectWorkspaceBindingError),
    SchemaMismatch,
    MissingIdentity { field: &'static str },
    InvalidDigest { field: &'static str },
    RuntimeArtifactMismatch,
    ContentMismatch,
}

impl RuntimeExecutionBinding {
    pub fn new(input: RuntimeExecutionBindingInput) -> Result<Self, RuntimeExecutionBindingError> {
        let RuntimeExecutionBindingInput {
            project_workspace,
            worktree_instance_id,
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
            project_workspace,
            worktree_instance_id,
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
        self.project_workspace
            .validate()
            .map_err(RuntimeExecutionBindingError::ProjectWorkspace)?;
        for (field, value) in [
            ("worktreeInstanceId", self.worktree_instance_id.as_str()),
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

    pub fn digest(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        update_digest_field(&mut hasher, RUNTIME_EXECUTION_SCHEMA_ID);
        update_digest_field(&mut hasher, RUNTIME_EXECUTION_SCHEMA_VERSION);
        update_digest_field(
            &mut hasher,
            self.project_workspace.project_workspace_identity(),
        );
        update_digest_field(&mut hasher, self.project_workspace.workspace_root_path());
        update_digest_field(&mut hasher, self.project_workspace.portability());
        hasher.update(
            &u64::try_from(self.project_workspace.repository_aliases().len())
                .expect("repository alias count fits u64")
                .to_le_bytes(),
        );
        for alias in self.project_workspace.repository_aliases() {
            update_digest_field(&mut hasher, alias);
        }
        update_digest_field(&mut hasher, &self.worktree_instance_id);
        update_digest_field(&mut hasher, &self.publication_nonce);
        update_digest_field(&mut hasher, &self.content_binding.schema_id);
        update_digest_field(&mut hasher, &self.content_binding.schema_version);
        update_digest_field(&mut hasher, &self.content_binding.identity.digest());
        update_digest_field(&mut hasher, &self.content_binding.authority_stamp.key_id);
        update_digest_field(
            &mut hasher,
            &self.content_binding.authority_stamp.canonical_digest,
        );
        update_digest_field(&mut hasher, &self.content_binding.authority_stamp.signature);
        update_digest_field(&mut hasher, self.runtime_artifact_digest.as_str());
        update_digest_field(&mut hasher, self.evaluator_policy_digest.as_str());
        update_digest_field(&mut hasher, self.active_artifact_receipt_digest.as_str());
        update_digest_field(&mut hasher, self.evaluator_abi_digest.as_str());
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

fn update_digest_field(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(
        &u64::try_from(value.len())
            .expect("identity field length fits u64")
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
