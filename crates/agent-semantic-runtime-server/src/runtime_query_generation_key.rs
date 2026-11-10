// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Gix-derived repository/worktree partition for Runtime query generations.

use crate::runtime_query_generation::RuntimeQueryGeneration;

/// The sole resident-generation partition key.
///
/// Client ProjectId/WorkspaceId values are transport routing aliases and must
/// never own durable or resident state.  These digests are the canonical V1
/// State Home identities derived from Gix common-dir, worktree root, and the
/// worktree-private Git directory.
#[derive(Clone, Debug)]
pub struct RuntimeProjectWorkspaceKey {
    repository_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    workspace_digest: agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest,
    routing_project_id: agent_semantic_client_protocol::ClientProjectId,
    routing_workspace_id: agent_semantic_client_protocol::ClientWorkspaceIdentity,
}

impl PartialEq for RuntimeProjectWorkspaceKey {
    fn eq(&self, other: &Self) -> bool {
        self.repository_digest == other.repository_digest
            && self.workspace_digest == other.workspace_digest
    }
}

impl Eq for RuntimeProjectWorkspaceKey {}

impl std::hash::Hash for RuntimeProjectWorkspaceKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.repository_digest, state);
        std::hash::Hash::hash(&self.workspace_digest, state);
    }
}

impl RuntimeProjectWorkspaceKey {
    pub fn from_project_binding(
        binding: &agent_semantic_artifacts::ProjectBinding,
        routing_project_id: agent_semantic_client_protocol::ClientProjectId,
        routing_workspace_id: agent_semantic_client_protocol::ClientWorkspaceIdentity,
    ) -> Result<Self, String> {
        binding.validate()?;
        if binding.workspace.repo_digest != binding.repo.digest {
            return Err("Runtime workspace identity is not owned by its repository".to_owned());
        }
        Ok(Self {
            repository_digest: binding.repo.digest.clone(),
            workspace_digest: binding.workspace.digest.clone(),
            routing_project_id,
            routing_workspace_id,
        })
    }

    pub fn repository_digest(&self) -> &str {
        self.repository_digest.as_str()
    }

    pub fn workspace_digest(&self) -> &str {
        self.workspace_digest.as_str()
    }

    pub fn routing_project_id(&self) -> &agent_semantic_client_protocol::ClientProjectId {
        &self.routing_project_id
    }

    pub fn routing_workspace_id(&self) -> &agent_semantic_client_protocol::ClientWorkspaceIdentity {
        &self.routing_workspace_id
    }
}

pub(super) fn validate_ready_identity(
    key: &RuntimeProjectWorkspaceKey,
    generation: &RuntimeQueryGeneration,
    expected_generation_digest: &str,
    expected_root_digest: &str,
) -> Result<(), String> {
    let actual_root_digest = generation.resident().source_root_digest();
    if generation.generation_digest() != expected_generation_digest
        || actual_root_digest != expected_root_digest
    {
        return Err(format!(
            "state=stale-generation reasonKind=cache-state-content-binding-mismatch repositoryDigest={} workspaceDigest={} expectedGenerationDigest={expected_generation_digest} actualGenerationDigest={} expectedRootDigest={expected_root_digest} actualRootDigest={actual_root_digest}",
            key.repository_digest(),
            key.workspace_digest(),
            generation.generation_digest()
        ));
    }
    Ok(())
}
