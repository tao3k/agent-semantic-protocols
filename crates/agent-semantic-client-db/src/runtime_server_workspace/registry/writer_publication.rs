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

#[derive(Debug)]
pub(super) struct PublishResidentOwnerSymbolRebindCommand {
    pub(super) target: super::core::WorkspaceWriteTarget,
    pub(super) workspace_identity: String,
    pub(super) rebind: crate::runtime_server_workspace::WorkspaceOwnerSymbolRebindV1,
    pub(super) prepared: super::super::resident_overlay::PreparedOwnerSymbolRebind,
    pub(super) reply: tokio::sync::oneshot::Sender<
        Result<crate::runtime_server_workspace::WorkspaceOwnerSymbolRebindReceiptV1, String>,
    >,
}

/// Commit a semantic owner delta to the resident read model only.
///
/// Parser artifacts are already persisted under their content identity before
/// this command is admitted. Re-encoding the entire workspace generation here
/// would turn one candidate parser miss into an O(workspace bytes) Search
/// barrier, so restart recovery deliberately rehydrates this overlay from the
/// parser artifact store instead.
pub(super) async fn publish_resident_owner_symbol_rebind_command(
    command: PublishResidentOwnerSymbolRebindCommand,
) {
    let PublishResidentOwnerSymbolRebindCommand {
        target,
        workspace_identity,
        rebind,
        prepared,
        reply,
    } = command;
    let result = (|| {
        let base = current_generation(&target.current, &workspace_identity)?;
        rebind.validate()?;
        if rebind.base_generation_digest != base.generation().generation_digest {
            return Err("workspace owner symbol rebind base generation digest mismatch".to_owned());
        }
        let rebind_id = rebind.rebind_id.clone();
        let base_generation_digest = rebind.base_generation_digest.clone();
        let rebound_owner_count = rebind.owners.len();
        let relation_count = rebind.relations.len();
        let (staged, symbol_count) =
            target
                .overlays
                .publish_owner_symbol_rebind(base.generation(), rebind, prepared)?;
        let resident_generation_digest = staged.generation_digest().to_owned();
        target.overlays.commit(staged);
        let receipt = crate::runtime_server_workspace::WorkspaceOwnerSymbolRebindReceiptV1 {
            schema_id:
                crate::runtime_server_workspace::WORKSPACE_OWNER_SYMBOL_REBIND_RECEIPT_SCHEMA_ID
                    .to_owned(),
            schema_version: "1".to_owned(),
            rebind_id,
            workspace_identity,
            base_generation_digest,
            resident_generation_digest,
            rebound_owner_count,
            symbol_count,
            relation_count,
        };
        receipt.validate()?;
        Ok(receipt)
    })();
    let _ = reply.send(result);
}

#[derive(Debug)]
pub(super) struct PublishOwnerContentMutationCommand {
    pub(super) target: super::core::WorkspaceWriteTarget,
    pub(super) workspace_identity: String,
    pub(super) mutation: crate::runtime_server_workspace::WorkspaceOwnerContentMutationV1,
    pub(super) prepared_search_delta:
        super::super::resident_overlay::PreparedOwnerContentSearchDelta,
    pub(super) reply: tokio::sync::oneshot::Sender<
        Result<crate::runtime_server_workspace::WorkspaceOwnerContentMutationReceiptV1, String>,
    >,
}

pub(super) async fn publish_owner_content_mutation_command(
    command: PublishOwnerContentMutationCommand,
) {
    let PublishOwnerContentMutationCommand {
        target,
        workspace_identity,
        mutation,
        prepared_search_delta,
        reply,
    } = command;
    let result = async {
        let base = current_generation(&target.current, &workspace_identity)?;
        let mutation_id = mutation.mutation_id.clone();
        let base_generation_digest = mutation.base_generation_digest.clone();
        let upserted_owner_count = mutation.upserts.len();
        let removed_owner_count = mutation.removals.len();
        let identity_delta = mutation
            .upserts
            .iter()
            .map(
                |upsert| super::super::owner_identity_journal::RuntimeOwnerIdentityEntry {
                    owner_path: upsert.owner.owner_path.clone(),
                    state: super::super::owner_identity_journal::RuntimeOwnerIdentityState::Present,
                    content_digest: Some(upsert.owner.content_digest.clone()),
                    mutation_id: None,
                },
            )
            .chain(mutation.removals.iter().map(|removal| {
                super::super::owner_identity_journal::RuntimeOwnerIdentityEntry {
                    owner_path: removal.owner_path.clone(),
                    state: super::super::owner_identity_journal::RuntimeOwnerIdentityState::Missing,
                    content_digest: None,
                    mutation_id: None,
                }
            }))
            .collect();
        let (staged, metrics) = target.overlays.publish_content_owner_mutation(
            base.generation(),
            mutation,
            prepared_search_delta,
        )?;
        target
            .publisher
            .publish_owner_identity_delta(
                &workspace_identity,
                &base_generation_digest,
                &mutation_id,
                identity_delta,
            )
            .await?;
        let receipt = crate::runtime_server_workspace::WorkspaceOwnerContentMutationReceiptV1 {
            schema_id:
                crate::runtime_server_workspace::WORKSPACE_OWNER_CONTENT_MUTATION_RECEIPT_SCHEMA_ID
                    .to_owned(),
            schema_version: "1".to_owned(),
            mutation_id,
            workspace_identity,
            base_generation_digest,
            resident_generation_digest: staged.generation_digest().to_owned(),
            owner_identity_root_digest: staged.owner_identity_root_digest().to_owned(),
            upserted_owner_count,
            removed_owner_count,
            touched_leaf_count: metrics.touched_leaf_count,
            written_node_count: metrics.written_node_count,
            reused_node_count: metrics.reused_node_count,
            full_merkle_rebuilds: metrics.full_merkle_rebuilds,
        };
        receipt.validate()?;
        target.overlays.commit(staged);
        Ok(receipt)
    }
    .await;
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
