use super::core::WorkspaceWriteTarget;
use crate::runtime_server_workspace::owner_identity_journal::RuntimeOwnerIdentityEntry;

pub(super) async fn publish_delta(
    target: WorkspaceWriteTarget,
    source_mutation_id: String,
    workspace_identity: String,
    delta: Vec<RuntimeOwnerIdentityEntry>,
) -> Result<(), String> {
    let Some(generation) = target.current.borrow().clone() else {
        return Ok(());
    };
    let base_generation_digest = generation.generation().generation_digest.clone();
    target
        .publisher
        .publish_owner_identity_delta(
            &workspace_identity,
            &base_generation_digest,
            &source_mutation_id,
            delta,
        )
        .await
}
