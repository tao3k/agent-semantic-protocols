//! Exact ProjectId and WorkspaceId partition for Runtime query generations.

use agent_semantic_client_protocol::ClientProjectId;
use agent_semantic_client_protocol::ClientWorkspaceIdentity;

use crate::runtime_query_generation::RuntimeQueryGeneration;

/// The sole resident-generation partition key.
///
/// A WorkspaceId is meaningful only inside its ProjectId. Keeping the pair as
/// one HashMap key makes cross-project aliasing unrepresentable in the Runtime
/// query authority and in its per-key single-flight lanes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RuntimeProjectWorkspaceKey {
    project_id: ClientProjectId,
    workspace_id: ClientWorkspaceIdentity,
}

impl RuntimeProjectWorkspaceKey {
    pub fn new(project_id: ClientProjectId, workspace_id: ClientWorkspaceIdentity) -> Self {
        Self {
            project_id,
            workspace_id,
        }
    }

    pub fn project_id(&self) -> &ClientProjectId {
        &self.project_id
    }

    pub fn workspace_id(&self) -> &ClientWorkspaceIdentity {
        &self.workspace_id
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
            "state=stale-generation reasonKind=cache-state-content-binding-mismatch projectId={} workspaceId={} expectedGenerationDigest={expected_generation_digest} actualGenerationDigest={} expectedRootDigest={expected_root_digest} actualRootDigest={actual_root_digest}",
            key.project_id().as_str(),
            key.workspace_id().as_str(),
            generation.generation_digest()
        ));
    }
    Ok(())
}
