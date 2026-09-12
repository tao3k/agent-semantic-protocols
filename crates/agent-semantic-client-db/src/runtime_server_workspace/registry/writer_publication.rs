// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
    let generation = staged.materialize_generation(
        base.generation(),
        base.generation().projection_capability.clone(),
    )?;
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

pub(super) async fn publish_owner_delta(
    target: &WorkspaceWriteTarget,
    request_id: String,
    workspace_identity: &str,
    delta: crate::runtime_server_workspace::WorkspaceGenerationDelta,
    counters: &RuntimeDataPlaneCounterState,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let base = current_generation(&target.current, workspace_identity)?;
    delta.validate()?;
    if delta.base_generation_digest != base.generation().generation_digest {
        return Err("workspace generation delta base generation digest mismatch".to_owned());
    }
    let staged = target.overlays.publish_owner_delta(
        base.generation(),
        delta.owners,
        delta.tombstones,
        delta.relations,
    )?;
    publish_staged_overlay_generation(
        target,
        request_id,
        staged,
        base.generation().active_epoch,
        counters,
    )
    .await
}

#[derive(Debug)]
pub(super) struct PublishOwnerDeltaCommand {
    pub(super) target: super::core::WorkspaceWriteTarget,
    pub(super) request_id: String,
    pub(super) workspace_identity: String,
    pub(super) delta: crate::runtime_server_workspace::WorkspaceGenerationDelta,
    pub(super) reply: tokio::sync::oneshot::Sender<
        Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String>,
    >,
}

pub(super) async fn publish_owner_delta_command(
    command: PublishOwnerDeltaCommand,
    counters: &super::core::RuntimeDataPlaneCounterState,
) -> Option<(
    String,
    crate::runtime_server_workspace::WorkspaceRecoveryReceipt,
)> {
    let scope_key = command.target.scope_key.clone();
    let result = publish_owner_delta(
        &command.target,
        command.request_id,
        &command.workspace_identity,
        command.delta,
        counters,
    )
    .await;
    let committed = result.as_ref().ok().cloned();
    let _ = command.reply.send(result);
    committed.map(|receipt| (scope_key, receipt))
}

#[derive(Debug)]
pub(super) struct PublishResidentOwnerDeltaCommand {
    pub(super) target: super::core::WorkspaceWriteTarget,
    pub(super) workspace_identity: String,
    pub(super) delta: crate::runtime_server_workspace::WorkspaceGenerationDelta,
    pub(super) reply: tokio::sync::oneshot::Sender<Result<String, String>>,
}

/// Commit a semantic owner delta to the resident read model only.
///
/// Parser artifacts are already persisted under their content identity before
/// this command is admitted. Re-encoding the entire workspace generation here
/// would turn one candidate parser miss into an O(workspace bytes) Search
/// barrier, so restart recovery deliberately rehydrates this overlay from the
/// parser artifact store instead.
pub(super) async fn publish_resident_owner_delta_command(
    command: PublishResidentOwnerDeltaCommand,
) {
    let PublishResidentOwnerDeltaCommand {
        target,
        workspace_identity,
        delta,
        reply,
    } = command;
    let result = (|| {
        let base = current_generation(&target.current, &workspace_identity)?;
        delta.validate()?;
        if delta.base_generation_digest != base.generation().generation_digest {
            return Err("workspace generation delta base generation digest mismatch".to_owned());
        }
        let staged = target.overlays.publish_semantic_owner_delta(
            base.generation(),
            delta.owners,
            delta.tombstones,
            delta.relations,
        )?;
        let resident_generation_digest = staged.generation_digest().to_owned();
        target.overlays.commit(staged);
        Ok(resident_generation_digest)
    })();
    let _ = reply.send(result);
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
    if generation.workspace_generation.leaf_count > 0 && generation.owners.is_empty() {
        return Err(format!(
            "source-index completeness gate rejected publication: matchingSourceCount={} ownerCount=0 leafCount={} reasonKind=source-index-resident-index-missing",
            generation.workspace_generation.leaf_count, generation.workspace_generation.leaf_count,
        ));
    }
    let target_epoch = generation.active_epoch;
    let workspace_identity = generation.workspace_identity.clone();
    let generation_digest = generation.generation_digest.clone();
    let source_root_digest = generation.source_snapshot.root_digest.clone();
    let projection_capability = generation.projection_capability_receipt(target_epoch)?;
    let generation = Arc::new(generation);
    let prepared_index =
        WorkspaceMemoryBackend::prepare_index(&generation.owners, &generation.relations);
    let backend_generation = Arc::clone(&generation);
    let backend = tokio::task::spawn_blocking(move || {
        WorkspaceMemoryBackend::from_validated_generation_with_index(
            backend_generation,
            prepared_index,
        )
        .map(Arc::new)
    })
    .await
    .map_err(|error| format!("workspace Search data-plane admission task failed: {error}"))??;
    publisher.publish(generation, active_epoch != 0).await?;
    counters.filesystem_writes.fetch_add(1, Ordering::Relaxed);
    current.send_replace(Some(backend));
    let receipt = WorkspaceRecoveryReceipt {
        projection_capability,
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
    let projection_capability = backend.generation().projection_capability.clone();
    current.send_replace(Some(backend));
    let receipt = WorkspaceRecoveryReceipt {
        projection_capability: projection_capability.into_ready_receipt(
            workspace_identity.clone(),
            generation_digest.clone(),
            source_root_digest.clone(),
            target_epoch,
        )?,
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
