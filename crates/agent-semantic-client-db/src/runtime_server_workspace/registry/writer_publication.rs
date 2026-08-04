//! Generation publication helpers owned by the resident workspace writer.

use super::core::{RuntimeDataPlaneCounterState, WorkspaceWriteTarget};
use crate::runtime_server_workspace::{
    MappedWorkspaceGeneration, ResidentOverlaySnapshot, RuntimeDataPlaneCounters,
    WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID, WorkspaceGenerationPublisher, WorkspaceGenerationState,
    WorkspaceMemoryBackend, WorkspaceMemoryGeneration, WorkspaceRecoveryReceipt,
    WorkspaceRecoverySource,
};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::watch;

pub(super) fn current_generation(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    workspace_identity: &str,
) -> Result<Arc<WorkspaceMemoryBackend>, String> {
    current.borrow().clone().ok_or_else(|| {
        format!(
            "runtime owner overlay requires an admitted canonical generation: workspaceIdentity={workspace_identity}"
        )
    })
}

pub(super) async fn publish_staged_overlay_generation(
    target: &WorkspaceWriteTarget,
    request_id: String,
    staged: ResidentOverlaySnapshot,
    active_epoch: u64,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let base = current_generation(&target.current, "staged-overlay")?;
    if base.generation().active_epoch != active_epoch {
        return Err(format!(
            "runtime overlay base epoch drift: expected={active_epoch} actual={}",
            base.generation().active_epoch
        ));
    }
    let generation = staged.materialize_generation(base.generation())?;
    let receipt = publish_generation(
        &target.current,
        target.publisher.as_ref(),
        request_id,
        WorkspaceRecoverySource::ProviderOwnerOverlay,
        active_epoch,
        generation,
        counters,
    )
    .await?;
    if let Some(backend) = target.current.borrow().clone() {
        target.overlays.reset(backend.generation());
    }
    Ok(receipt)
}

pub(super) fn active_epoch(current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>) -> u64 {
    current
        .borrow()
        .as_ref()
        .map_or(0, |backend| backend.generation().active_epoch)
}

pub(super) async fn publish_generation(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    publisher: &WorkspaceGenerationPublisher,
    request_id: String,
    source: WorkspaceRecoverySource,
    active_epoch: u64,
    generation: WorkspaceMemoryGeneration,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let target_epoch = generation.active_epoch;
    let workspace_identity = generation.workspace_identity.clone();
    let generation_digest = generation.generation_digest.clone();
    let source_root_digest = generation.source_snapshot.root_digest.clone();
    let generation = Arc::new(generation);
    let prepared_index =
        WorkspaceMemoryBackend::prepare_index(&generation.owners, &generation.relations);
    let backend = Arc::new(
        WorkspaceMemoryBackend::from_validated_generation_with_index(
            Arc::clone(&generation),
            prepared_index,
        )?,
    );
    publisher.publish(generation, active_epoch != 0).await?;
    counters.filesystem_writes.fetch_add(1, Ordering::Relaxed);
    current.send_replace(Some(backend));
    let receipt = WorkspaceRecoveryReceipt {
        schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id,
        workspace_identity,
        source,
        state: WorkspaceGenerationState::Ready,
        active_epoch,
        target_epoch,
        generation_digest,
        source_root_digest,
        old_generation_readable: active_epoch != 0,
        resident_publication_elapsed_micros: 0,
        counters: RuntimeDataPlaneCounters {
            filesystem_writes: 1,
            ..RuntimeDataPlaneCounters::default()
        },
    };
    receipt.validate()?;
    Ok(receipt)
}

pub(super) async fn restore_checkpoint(
    current: &watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    request_id: String,
    workspace_identity: String,
    path: PathBuf,
    active_epoch: u64,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let mapped = MappedWorkspaceGeneration::open(&path).await?;
    counters.filesystem_reads.fetch_add(1, Ordering::Relaxed);
    let backend = mapped.backend();
    if backend.generation().workspace_identity != workspace_identity {
        return Err(format!(
            "workspace checkpoint identity mismatch: expected={workspace_identity} actual={}",
            backend.generation().workspace_identity
        ));
    }
    let target_epoch = backend.generation().active_epoch;
    if target_epoch <= active_epoch {
        return Err(format!(
            "workspace checkpoint epoch must advance: activeEpoch={active_epoch} targetEpoch={target_epoch}"
        ));
    }
    let generation_digest = backend.generation().generation_digest.clone();
    let source_root_digest = backend.generation().source_snapshot.root_digest.clone();
    current.send_replace(Some(backend));
    let receipt = WorkspaceRecoveryReceipt {
        schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id,
        workspace_identity,
        source: WorkspaceRecoverySource::MmapCheckpoint,
        state: WorkspaceGenerationState::Ready,
        active_epoch,
        target_epoch,
        generation_digest,
        source_root_digest,
        old_generation_readable: active_epoch != 0,
        resident_publication_elapsed_micros: 0,
        counters: RuntimeDataPlaneCounters {
            filesystem_reads: 1,
            ..RuntimeDataPlaneCounters::default()
        },
    };
    receipt.validate()?;
    Ok(receipt)
}
