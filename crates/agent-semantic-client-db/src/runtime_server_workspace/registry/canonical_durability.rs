use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::watch;

use super::core::RuntimeDataPlaneCounterState;
use crate::runtime_server_workspace::{
    WorkspaceGenerationDurabilityReceipt, WorkspaceGenerationDurabilityState,
    WorkspaceGenerationPublisher, WorkspaceMemoryGeneration,
};

pub(super) async fn commit_canonical_generation(
    publisher: &WorkspaceGenerationPublisher,
    generation: Arc<WorkspaceMemoryGeneration>,
    previous_epoch_readable: bool,
    durability: &watch::Sender<Option<WorkspaceGenerationDurabilityReceipt>>,
    workspace_identity: &str,
    generation_digest: &str,
    target_epoch: u64,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<(), String> {
    if let Err(error) = publisher.publish(generation, previous_epoch_readable).await {
        durability.send_replace(Some(WorkspaceGenerationDurabilityReceipt::new(
            workspace_identity.to_owned(),
            generation_digest.to_owned(),
            target_epoch,
            WorkspaceGenerationDurabilityState::Failed,
            Some(error.clone()),
        )?));
        return Err(error);
    }

    counters.filesystem_writes.fetch_add(1, Ordering::Relaxed);
    durability.send_replace(Some(WorkspaceGenerationDurabilityReceipt::new(
        workspace_identity.to_owned(),
        generation_digest.to_owned(),
        target_epoch,
        WorkspaceGenerationDurabilityState::DurableReady,
        None,
    )?));
    Ok(())
}
