// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-specific V1 proof for safe process-cold durable reuse.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity;
use crate::runtime_server_workspace::WorkspaceGenerationSnapshot;

const SCHEMA_ID: &str = "agent.semantic-protocols.workspace-generation-admission-binding";
const SCHEMA_VERSION: &str = "1";
const FILE_NAME: &str = "generation-admission-binding.v1.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WorkspaceGenerationAdmissionBindingV1 {
    schema_id: String,
    schema_version: String,
    workspace_identity: String,
    project_root: PathBuf,
    candidate: WorkspaceGenerationCandidateIdentity,
    generation_digest: String,
    source_root_digest: String,
}

impl WorkspaceGenerationAdmissionBindingV1 {
    pub(crate) fn new(
        workspace_identity: String,
        project_root: PathBuf,
        candidate: WorkspaceGenerationCandidateIdentity,
        generation_digest: String,
        source_root_digest: String,
    ) -> Result<Self, String> {
        let binding = Self {
            schema_id: SCHEMA_ID.to_owned(),
            schema_version: SCHEMA_VERSION.to_owned(),
            workspace_identity,
            project_root,
            candidate,
            generation_digest,
            source_root_digest,
        };
        binding.validate()?;
        Ok(binding)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_id != SCHEMA_ID || self.schema_version != SCHEMA_VERSION {
            return Err(
                "workspace generation admission binding schema identity mismatch".to_owned(),
            );
        }
        if self.workspace_identity.trim().is_empty() || !self.project_root.is_absolute() {
            return Err("workspace generation admission binding scope is incomplete".to_owned());
        }
        self.candidate.validate()?;
        validate_digest("generationDigest", &self.generation_digest)?;
        validate_digest("sourceRootDigest", &self.source_root_digest)
    }

    pub(crate) fn admits(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        candidate: &WorkspaceGenerationCandidateIdentity,
        snapshot: &WorkspaceGenerationSnapshot,
    ) -> bool {
        self.admits_identity(
            workspace_identity,
            project_root,
            candidate,
            &snapshot.generation_digest,
            &snapshot.source_root_digest,
            &snapshot.workspace_identity,
        )
    }

    fn admits_identity(
        &self,
        workspace_identity: &str,
        project_root: &Path,
        candidate: &WorkspaceGenerationCandidateIdentity,
        generation_digest: &str,
        source_root_digest: &str,
        snapshot_workspace_identity: &str,
    ) -> bool {
        self.validate().is_ok()
            && candidate_is_restart_verifiable(candidate)
            && self.workspace_identity == workspace_identity
            && self.project_root == project_root
            && &self.candidate == candidate
            && self.generation_digest == generation_digest
            && self.source_root_digest == source_root_digest
            && self.workspace_identity == snapshot_workspace_identity
    }
}

pub(crate) fn candidate_is_restart_verifiable(
    candidate: &WorkspaceGenerationCandidateIdentity,
) -> bool {
    candidate.candidate_generation.algorithm == "blake3-worktree-state-v1"
        && candidate
            .candidate_generation
            .authorities
            .iter()
            .any(|authority| {
                matches!(
                    authority,
                    agent_semantic_runtime::git::RepositoryCandidateAuthority::GitIndex
                        | agent_semantic_runtime::git::RepositoryCandidateAuthority::GitWorktree
                )
            })
        && !candidate
            .candidate_generation
            .authorities
            .iter()
            .any(|authority| {
                matches!(
                    authority,
                    agent_semantic_runtime::git::RepositoryCandidateAuthority::ServerResident
                )
            })
}

pub(crate) async fn publish(
    generation_directory: &Path,
    binding: &WorkspaceGenerationAdmissionBindingV1,
) -> Result<(), String> {
    binding.validate()?;
    let final_path = generation_directory.join(FILE_NAME);
    let pending_path = generation_directory.join(format!(".{FILE_NAME}.pending"));
    let bytes = serde_json::to_vec(binding)
        .map_err(|error| format!("encode workspace generation admission binding: {error}"))?;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&pending_path)
        .await
        .map_err(|error| format!("create workspace generation admission binding: {error}"))?;
    file.write_all(&bytes)
        .await
        .map_err(|error| format!("write workspace generation admission binding: {error}"))?;
    file.sync_all()
        .await
        .map_err(|error| format!("sync workspace generation admission binding: {error}"))?;
    drop(file);
    tokio::fs::rename(&pending_path, &final_path)
        .await
        .map_err(|error| format!("publish workspace generation admission binding: {error}"))?;
    tokio::fs::File::open(generation_directory)
        .await
        .map_err(|error| format!("open workspace generation directory: {error}"))?
        .sync_all()
        .await
        .map_err(|error| format!("sync workspace generation directory: {error}"))
}

pub(crate) async fn read(
    generation_directory: &Path,
) -> Result<WorkspaceGenerationAdmissionBindingV1, String> {
    let path = generation_directory.join(FILE_NAME);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| format!("read workspace generation admission binding: {error}"))?;
    let binding: WorkspaceGenerationAdmissionBindingV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("decode workspace generation admission binding: {error}"))?;
    binding.validate()?;
    Ok(binding)
}

pub(crate) async fn invalidate(generation_directory: &Path) -> Result<(), String> {
    let path = generation_directory.join(FILE_NAME);
    match tokio::fs::remove_file(&path).await {
        Ok(()) => tokio::fs::File::open(generation_directory)
            .await
            .map_err(|error| format!("open workspace generation directory: {error}"))?
            .sync_all()
            .await
            .map_err(|error| format!("sync workspace generation directory: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "invalidate workspace generation admission binding: {error}"
        )),
    }
}

fn validate_digest(label: &str, digest: &str) -> Result<(), String> {
    let value = digest
        .strip_prefix("blake3-256:")
        .or_else(|| digest.strip_prefix("blake3:"))
        .unwrap_or(digest);
    let valid = value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    valid
        .then_some(())
        .ok_or_else(|| format!("workspace generation admission binding {label} is invalid"))
}

#[cfg(test)]
#[path = "../tests/unit/runtime_server_admission_binding.rs"]
mod tests;
