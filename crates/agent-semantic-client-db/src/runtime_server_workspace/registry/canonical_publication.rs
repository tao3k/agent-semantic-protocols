// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Atomic canonical-generation publication owned by the workspace writer lane.

use std::{collections::HashMap, sync::Arc};

use tokio::sync::oneshot;

pub(super) type DurabilityTask =
    std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

use super::core::{RuntimeDataPlaneCounterState, WorkspaceWriteTarget};
use crate::runtime_server_workspace::{
    RuntimeDataPlaneCounters, WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID,
    WorkspaceCanonicalMaterialization, WorkspaceGenerationState, WorkspaceMemoryBackend,
    WorkspaceRecoveryReceipt, WorkspaceRecoverySource,
};

/// One canonical materialization command admitted by the workspace writer.
#[derive(Debug)]
pub(super) struct EnsureCanonicalGenerationCommand {
    pub(super) target: WorkspaceWriteTarget,
    pub(super) request_id: String,
    pub(super) workspace_identity: String,
    pub(super) materialization: WorkspaceCanonicalMaterialization,
    pub(super) prepared_index: Arc<super::super::memory_backend::WorkspaceMemoryIndex>,
    pub(super) reply: oneshot::Sender<Result<WorkspaceRecoveryReceipt, String>>,
}

/// Publish the immutable generation before exposing resident state or replying `Ready`.
pub(super) async fn publish_canonical_generation(
    command: EnsureCanonicalGenerationCommand,
    counters: &Arc<RuntimeDataPlaneCounterState>,
    last_receipts: &mut HashMap<String, WorkspaceRecoveryReceipt>,
    durability_tasks: &tokio::sync::mpsc::UnboundedSender<DurabilityTask>,
) {
    let resident_publication_started = tokio::time::Instant::now();
    let EnsureCanonicalGenerationCommand {
        target,
        request_id,
        workspace_identity,
        materialization,
        prepared_index,
        reply,
    } = command;
    let WorkspaceWriteTarget {
        scope_key,
        current,
        durability,
        overlays,
        publisher,
    } = target;
    let active = current.borrow().clone();
    let active_epoch = active
        .as_ref()
        .map_or(0, |backend| backend.generation().active_epoch);
    let generation_build_started = tokio::time::Instant::now();
    let generation = materialization.into_generation(active_epoch).map(Arc::new);
    record_generation_build(&workspace_identity, generation_build_started.elapsed());

    let result = match generation {
        Ok(generation)
            if active.as_ref().is_some_and(|backend| {
                backend.generation().generation_digest == generation.generation_digest
                    && backend.generation().selector_set_digest == generation.selector_set_digest
            }) && complete_published_generation_matches(
                publisher.pointer_path(),
                &generation,
            )
            .await =>
        {
            reusable_receipt(
                last_receipts,
                &scope_key,
                request_id,
                workspace_identity,
                &generation,
                resident_publication_started,
            )
        }
        Ok(generation) => {
            let progress = publishing_receipt(
                &request_id,
                &workspace_identity,
                &generation,
                active_epoch,
                active.is_some(),
                resident_publication_started,
            );
            match progress.and_then(|receipt| {
                receipt.validate()?;
                Ok(receipt)
            }) {
                Ok(_) => {
                    let result = publish_new_generation(
                        &current,
                        &durability,
                        &overlays,
                        Arc::clone(&publisher),
                        request_id,
                        workspace_identity,
                        generation,
                        prepared_index,
                        active_epoch,
                        resident_publication_started,
                        Arc::clone(counters),
                        durability_tasks,
                    )
                    .await;
                    if let Ok(receipt) = &result {
                        last_receipts.insert(scope_key, receipt.clone());
                    }
                    let _ = reply.send(result);
                    return;
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
    .and_then(|receipt| {
        receipt.validate()?;
        Ok(receipt)
    });

    if let Ok(receipt) = &result {
        if let Some(backend) = current.borrow().clone() {
            overlays.reset(backend.generation());
        }
        last_receipts.insert(scope_key, receipt.clone());
    }
    let _ = reply.send(result);
}

async fn complete_published_generation_matches(
    pointer_path: &std::path::Path,
    generation: &crate::runtime_server_workspace::WorkspaceMemoryGeneration,
) -> bool {
    if !super::super::WorkspaceGenerationPointerReader::matches_generation(pointer_path, generation)
        .await
    {
        return false;
    }
    matches!(
        super::super::WorkspaceExactProjectionDataPlaneClient::open_state(pointer_path).await,
        Ok(super::super::WorkspaceExactProjectionDataPlaneOpen::Ready(
            _
        ))
    ) && super::super::WorkspaceSearchGenerationDataPlaneClient::open(
        pointer_path,
        std::path::Path::new(&generation.project_root),
    )
    .await
    .is_ok()
}

fn record_generation_build(workspace_identity: &str, elapsed: std::time::Duration) {
    let elapsed_micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
    let budget_micros = 800_000;
    let mut observation = crate::runtime_server_opentelemetry::RuntimePerformanceObservation::new(
        "workspace-canonical-materialization",
        "generation-build",
        elapsed_micros,
        budget_micros,
        if elapsed_micros < budget_micros {
            "within-budget"
        } else {
            "budget-exceeded"
        },
    );
    observation.workspace_identity = Some(workspace_identity.to_owned());
    let _ = crate::runtime_server_opentelemetry::try_record_to_active_runtime(observation);
}

fn reusable_receipt(
    last_receipts: &HashMap<String, WorkspaceRecoveryReceipt>,
    scope_key: &str,
    request_id: String,
    workspace_identity: String,
    generation: &crate::runtime_server_workspace::WorkspaceMemoryGeneration,
    started: tokio::time::Instant,
) -> Result<WorkspaceRecoveryReceipt, String> {
    let projection_capability =
        generation.projection_capability_receipt(generation.active_epoch)?;
    last_receipts
        .get(scope_key)
        .cloned()
        .or_else(|| {
            generation
                .active_epoch
                .checked_sub(1)
                .map(|previous_epoch| WorkspaceRecoveryReceipt {
                    projection_capability: projection_capability.clone(),
                    schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
                    schema_version: "1".to_owned(),
                    request_id,
                    workspace_identity,
                    source: WorkspaceRecoverySource::MmapCheckpoint,
                    state: WorkspaceGenerationState::Ready,
                    active_epoch: previous_epoch,
                    target_epoch: generation.active_epoch,
                    generation_digest: generation.generation_digest.clone(),
                    source_root_digest: generation.source_snapshot.root_digest.clone(),
                    old_generation_readable: previous_epoch != 0,
                    resident_publication_elapsed_micros: elapsed_micros(started),
                    counters: RuntimeDataPlaneCounters::default(),
                })
        })
        .ok_or_else(|| {
            "runtime workspace canonical generation has no reusable recovery receipt".to_owned()
        })
}

fn publishing_receipt(
    request_id: &str,
    workspace_identity: &str,
    generation: &crate::runtime_server_workspace::WorkspaceMemoryGeneration,
    active_epoch: u64,
    old_generation_readable: bool,
    started: tokio::time::Instant,
) -> Result<WorkspaceRecoveryReceipt, String> {
    Ok(WorkspaceRecoveryReceipt {
        projection_capability: generation.projection_capability_receipt(generation.active_epoch)?,
        schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id: request_id.to_owned(),
        workspace_identity: workspace_identity.to_owned(),
        source: WorkspaceRecoverySource::TursoGeneration,
        state: WorkspaceGenerationState::PublishingNext,
        active_epoch,
        target_epoch: generation.active_epoch,
        generation_digest: generation.generation_digest.clone(),
        source_root_digest: generation.source_snapshot.root_digest.clone(),
        old_generation_readable,
        resident_publication_elapsed_micros: elapsed_micros(started),
        counters: RuntimeDataPlaneCounters::default(),
    })
}

async fn publish_new_generation(
    current: &tokio::sync::watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
    durability: &tokio::sync::watch::Sender<
        Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>,
    >,
    overlays: &crate::runtime_server_workspace::ResidentOverlayStore,
    publisher: Arc<crate::runtime_server_workspace::WorkspaceGenerationPublisher>,
    request_id: String,
    workspace_identity: String,
    generation: Arc<crate::runtime_server_workspace::WorkspaceMemoryGeneration>,
    prepared_index: Arc<super::super::memory_backend::WorkspaceMemoryIndex>,
    active_epoch: u64,
    started: tokio::time::Instant,
    counters: Arc<RuntimeDataPlaneCounterState>,
    durability_tasks: &tokio::sync::mpsc::UnboundedSender<DurabilityTask>,
) -> Result<WorkspaceRecoveryReceipt, String> {
    if generation.workspace_generation.leaf_count > 0 && generation.owners.is_empty() {
        return Err(format!(
            "source-index completeness gate rejected publication: matchingSourceCount={} ownerCount=0 leafCount={} reasonKind=source-index-resident-index-missing",
            generation.workspace_generation.leaf_count, generation.workspace_generation.leaf_count,
        ));
    }
    let target_epoch = generation.active_epoch;
    let generation_digest = generation.generation_digest.clone();
    let source_root_digest = generation.source_snapshot.root_digest.clone();
    let backend = Arc::new(
        WorkspaceMemoryBackend::from_validated_generation_with_index(
            Arc::clone(&generation),
            prepared_index,
        )?,
    );
    let receipt = WorkspaceRecoveryReceipt {
        projection_capability: generation.projection_capability_receipt(target_epoch)?,
        schema_id: WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id,
        workspace_identity: workspace_identity.clone(),
        source: WorkspaceRecoverySource::TursoGeneration,
        state: WorkspaceGenerationState::Ready,
        active_epoch,
        target_epoch,
        generation_digest: generation_digest.clone(),
        source_root_digest,
        old_generation_readable: active_epoch != 0,
        resident_publication_elapsed_micros: elapsed_micros(started),
        counters: RuntimeDataPlaneCounters::default(),
    };
    receipt.validate()?;
    let durability_attachment = durability.clone();
    let durability_workspace_identity = workspace_identity.clone();
    let durability_generation_digest = generation_digest.clone();
    let (durability_committed, await_durability_commit) = tokio::sync::oneshot::channel();
    durability_tasks
        .send(Box::pin(async move {
            let result = super::canonical_durability::commit_canonical_generation(
                publisher.as_ref(),
                generation,
                active_epoch != 0,
                &durability_attachment,
                &durability_workspace_identity,
                &durability_generation_digest,
                target_epoch,
                counters.as_ref(),
            )
            .await;
            let _ = durability_committed.send(result);
        }))
        .map_err(|_| "workspace durability attachment lane is unavailable".to_owned())?;
    await_durability_commit
        .await
        .map_err(|_| "workspace canonical durability task dropped before terminal".to_owned())??;
    current.send_replace(Some(Arc::clone(&backend)));
    overlays.reset(backend.generation());
    Ok(receipt)
}

fn elapsed_micros(started: tokio::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_server_workspace/canonical_publication.rs"]
mod tests;
