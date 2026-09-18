// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic pointer identity joining a durable source generation and Runtime sidecar.

use serde::Deserialize;
use serde::Serialize;

use crate::Blake3DigestV1;
use crate::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublication;
use crate::runtime_workspace_execution_publication::RuntimeWorkspaceExecutionPublicationError;

pub const RUNTIME_WORKSPACE_EXECUTION_POINTER_SCHEMA_ID: &str =
    "agent.semantic-protocols.runtime-workspace-execution-pointer";
pub const RUNTIME_WORKSPACE_EXECUTION_POINTER_SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// The complete digest tuple replaced by the single canonical pointer write.
pub struct RuntimeWorkspaceExecutionPointer {
    pub schema_id: String,
    pub schema_version: String,
    pub workspace_identity: String,
    pub generation_digest: Blake3DigestV1,
    pub source_root_digest: Blake3DigestV1,
    pub execution_publication_digest: Blake3DigestV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeWorkspaceExecutionPointerError {
    Publication(RuntimeWorkspaceExecutionPublicationError),
    SchemaMismatch,
    MissingWorkspaceIdentity,
    InvalidDigest { field: &'static str },
    PublicationMismatch,
}

impl RuntimeWorkspaceExecutionPointer {
    /// Projects the only pointer shape that may expose this immutable sidecar.
    pub fn from_publication(
        publication: &RuntimeWorkspaceExecutionPublication,
    ) -> Result<Self, RuntimeWorkspaceExecutionPointerError> {
        publication
            .validate()
            .map_err(RuntimeWorkspaceExecutionPointerError::Publication)?;
        let pointer = Self {
            schema_id: RUNTIME_WORKSPACE_EXECUTION_POINTER_SCHEMA_ID.into(),
            schema_version: RUNTIME_WORKSPACE_EXECUTION_POINTER_SCHEMA_VERSION.into(),
            workspace_identity: publication.workspace_identity.clone(),
            generation_digest: publication.generation_digest.clone(),
            source_root_digest: publication.source_root_digest.clone(),
            execution_publication_digest: publication.publication_digest.clone(),
        };
        pointer.validate()?;
        Ok(pointer)
    }

    pub fn validate(&self) -> Result<(), RuntimeWorkspaceExecutionPointerError> {
        if self.schema_id != RUNTIME_WORKSPACE_EXECUTION_POINTER_SCHEMA_ID
            || self.schema_version != RUNTIME_WORKSPACE_EXECUTION_POINTER_SCHEMA_VERSION
        {
            return Err(RuntimeWorkspaceExecutionPointerError::SchemaMismatch);
        }
        if self.workspace_identity.trim().is_empty() {
            return Err(RuntimeWorkspaceExecutionPointerError::MissingWorkspaceIdentity);
        }
        for (field, value) in [
            ("generationDigest", self.generation_digest.as_str()),
            ("sourceRootDigest", self.source_root_digest.as_str()),
            (
                "executionPublicationDigest",
                self.execution_publication_digest.as_str(),
            ),
        ] {
            if !is_content_digest(value) {
                return Err(RuntimeWorkspaceExecutionPointerError::InvalidDigest { field });
            }
        }
        Ok(())
    }

    /// Validates both objects independently before comparing every joined field.
    pub fn admits(
        &self,
        publication: &RuntimeWorkspaceExecutionPublication,
    ) -> Result<(), RuntimeWorkspaceExecutionPointerError> {
        self.validate()?;
        publication
            .validate()
            .map_err(RuntimeWorkspaceExecutionPointerError::Publication)?;
        if self.workspace_identity != publication.workspace_identity
            || self.generation_digest != publication.generation_digest
            || self.source_root_digest != publication.source_root_digest
            || self.execution_publication_digest != publication.publication_digest
        {
            return Err(RuntimeWorkspaceExecutionPointerError::PublicationMismatch);
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
