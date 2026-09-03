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
    let (resident_published, await_resident_publication) = tokio::sync::oneshot::channel();
    durability_tasks
        .send(Box::pin(async move {
            if await_resident_publication.await.is_err() {
                return;
            }
            let _ = super::canonical_durability::commit_canonical_generation(
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
        }))
        .map_err(|_| "workspace durability attachment lane is unavailable".to_owned())?;
    durability.send_replace(Some(
        crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt::new(
            workspace_identity.clone(),
            generation_digest.clone(),
            target_epoch,
            crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::ResidentReady,
            None,
        )?,
    ));
    current.send_replace(Some(Arc::clone(&backend)));
    overlays.reset(backend.generation());
    let _ = resident_published.send(());
    Ok(receipt)
}

fn elapsed_micros(started: tokio::time::Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_server_workspace::{
        WorkspaceGenerationBuild, WorkspaceGenerationDataPlaneClient,
        WorkspaceGenerationDataPlaneOpen, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
    };

    fn generation(
        project_root: &std::path::Path,
    ) -> Arc<crate::runtime_server_workspace::WorkspaceMemoryGeneration> {
        let bytes = b"pub fn resident_before_durable() {}\n".to_vec();
        generation_from_owners(
            project_root,
            vec![WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: "src/lib.rs".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
                selectors: vec![WorkspaceSelectorSnapshot {
                    selector: "rust://src/lib.rs#item/function/resident_before_durable".to_owned(),
                    byte_start: 0,
                    byte_end: bytes.len(),
                    query_keys: vec!["durable".to_owned(), "resident".to_owned()],
                    derived_projections: Vec::new(),
                }],
                bytes,
            }],
        )
    }

    fn generation_from_owners(
        project_root: &std::path::Path,
        owners: Vec<WorkspaceOwnerSnapshot>,
    ) -> Arc<crate::runtime_server_workspace::WorkspaceMemoryGeneration> {
        use agent_semantic_search::{
            ContentSearchGenerationReceipt, SearchGenerationConstructionStage,
            SearchGenerationIdentity, SearchGenerationStageReceipt,
        };

        let workspace_snapshot =
            agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
                owners
                    .iter()
                    .map(|owner| (owner.owner_path.clone(), owner.content_digest.clone())),
            );
        let source_snapshot = workspace_snapshot.evidence(
            agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
            format!("blake3-256:{}", blake3::hash(b"resident-provider").to_hex()),
        );
        let identity = SearchGenerationIdentity {
            project_id: "repo-0000000000000001".to_owned(),
            workspace_id: "workspace-resident-durability".to_owned(),
            source_root_digest: format!(
                "blake3-256:{}",
                source_snapshot
                    .root_digest
                    .trim_start_matches("blake3-256:")
            ),
            provider_digest: format!(
                "blake3-256:{}",
                source_snapshot
                    .provider_digest
                    .trim_start_matches("blake3-256:")
            ),
            schema_digest: format!("blake3-256:{}", "0".repeat(64)),
            generation_candidate_digest: format!("blake3-256:{}", "1".repeat(64)),
        };
        let content_search_generation =
            ContentSearchGenerationReceipt::new(SearchGenerationStageReceipt {
                stage: SearchGenerationConstructionStage::SourceByteAcquisition,
                identity,
                artifact_digest: format!("blake3-256:{}", "2".repeat(64)),
                worker_id: "resident-publication-test".to_owned(),
                complete: true,
            })
            .expect("content generation receipt");
        let module_graph_digest = format!("blake3-256:{}", "3".repeat(64));
        let runtime_provider_execution_binding =
            agent_semantic_artifacts::installed_provider_binding::RuntimeProviderExecutionBinding::build(
                "repo-0000000000000001".to_owned(),
                "workspace-resident-durability".to_owned(),
                format!("blake3-256:{}", "4".repeat(64)),
                format!("blake3-256:{}", "5".repeat(64)),
                format!("blake3-256:{}", "6".repeat(64)),
                source_snapshot
                    .root_integrity_reference()
                    .expect("source snapshot integrity"),
                module_graph_digest.clone(),
            )
            .expect("Runtime provider execution binding");
        let projection_capability = crate::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector(
            format!("blake3-256:{}", "0".repeat(64)),
            "rust://src/lib.rs#item/function/resident_before_durable".to_owned(),
            "src/lib.rs".to_owned(),
            std::collections::BTreeSet::from([
                crate::active_generation_projection_capability::ActiveGenerationProjectionMode::Source,
            ]),
        )
        .expect("projection capability");
        Arc::new(
            crate::runtime_server_workspace::WorkspaceMemoryGeneration::try_from_build(
                WorkspaceGenerationBuild {
                    projection_capability,
                    workspace_identity: "workspace-resident-durability".to_owned(),
                    project_root: project_root.to_string_lossy().into_owned(),
                    active_epoch: 1,
                    workspace_snapshot,
                    source_snapshot,
                    module_graph_digest,
                    runtime_provider_execution_binding: Some(runtime_provider_execution_binding),
                    content_search_generation,
                    project_resolutions: Vec::new(),
                    owners,
                    relations: Vec::new(),
                },
            )
            .expect("resident generation"),
        )
    }

    fn large_generation(
        project_root: &std::path::Path,
        owner_count: usize,
    ) -> Arc<crate::runtime_server_workspace::WorkspaceMemoryGeneration> {
        let owners = (0..owner_count)
            .map(|index| {
                let owner_path = if index == 0 {
                    "src/lib.rs".to_owned()
                } else {
                    format!("src/generated/owner_{index:04}.rs")
                };
                let bytes = format!("pub fn owner_{index:04}() {{}}\n").into_bytes();
                WorkspaceOwnerSnapshot {
                    authority: None,
                    content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
                    selectors: vec![WorkspaceSelectorSnapshot {
                        selector: if index == 0 {
                            "rust://src/lib.rs#item/function/resident_before_durable".to_owned()
                        } else {
                            format!(
                                "rust://src/generated/owner_{index:04}.rs#item/function/owner_{index:04}"
                            )
                        },
                        byte_start: 0,
                        byte_end: bytes.len(),
                        query_keys: vec![format!("owner_{index:04}")],
                        derived_projections: Vec::new(),
                    }],
                    owner_path,
                    bytes,
                }
            })
            .collect();
        generation_from_owners(project_root, owners)
    }

    async fn publish_with_held_durability(
        publisher_directory: std::path::PathBuf,
    ) -> (
        tokio::sync::watch::Sender<Option<Arc<WorkspaceMemoryBackend>>>,
        tokio::sync::watch::Receiver<
            Option<crate::runtime_server_workspace::WorkspaceGenerationDurabilityReceipt>,
        >,
        tokio::sync::mpsc::UnboundedReceiver<DurabilityTask>,
        std::path::PathBuf,
    ) {
        let project_root = publisher_directory
            .parent()
            .expect("publisher parent")
            .join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let generation = generation(&project_root);
        let prepared_index =
            WorkspaceMemoryBackend::prepare_index(&generation.owners, &generation.relations);
        let (current, _) = tokio::sync::watch::channel(None);
        let (durability, durability_observer) = tokio::sync::watch::channel(None);
        let overlays = crate::runtime_server_workspace::ResidentOverlayStore::new(None);
        let publisher = Arc::new(
            crate::runtime_server_workspace::WorkspaceGenerationPublisher::new(publisher_directory)
                .await
                .expect("generation publisher"),
        );
        let pointer_path = publisher.pointer_path().to_path_buf();
        let (durability_tasks, durability_requests) = tokio::sync::mpsc::unbounded_channel();
        publish_new_generation(
            &current,
            &durability,
            &overlays,
            publisher,
            "resident-ready".to_owned(),
            "workspace-resident-durability".to_owned(),
            generation,
            prepared_index,
            0,
            tokio::time::Instant::now(),
            Arc::new(RuntimeDataPlaneCounterState::default()),
            &durability_tasks,
        )
        .await
        .expect("resident publication");
        (
            current,
            durability_observer,
            durability_requests,
            pointer_path,
        )
    }

    #[tokio::test]
    async fn resident_read_is_ready_while_durability_is_blocked_then_restart_restore_succeeds() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let project_root = temporary.path().join("project");
        let (current, durability, mut tasks, pointer_path) =
            publish_with_held_durability(temporary.path().join("generation")).await;

        assert!(!pointer_path.exists(), "durability must still be blocked");
        assert_eq!(
            durability
                .borrow()
                .as_ref()
                .expect("durability receipt")
                .state,
            crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::ResidentReady,
        );
        let backend = current.borrow().clone().expect("resident generation");
        let resident =
            crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
                crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(backend),
            )
            .expect("resident read client");
        assert!(
            resident
                .owner_snapshot("src/lib.rs")
                .expect("resident owner")
                .is_some()
        );

        tasks.recv().await.expect("durability task").await;
        assert_eq!(
            durability
                .borrow()
                .as_ref()
                .expect("durability receipt")
                .state,
            crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady,
        );
        assert!(matches!(
            WorkspaceGenerationDataPlaneClient::open_state(&pointer_path)
                .await
                .expect("restart restore state"),
            WorkspaceGenerationDataPlaneOpen::Ready(_),
        ));
        let restored = crate::runtime_resident_read::RuntimeResidentReadClient::open(
            &pointer_path,
            &project_root,
        )
        .await
        .expect("durable read client");
        assert_eq!(resident.generation_digest(), restored.generation_digest());
        assert_eq!(
            resident.owner_merkle_root_digest(),
            restored.owner_merkle_root_digest()
        );
        assert_eq!(
            resident.indexed_owner_paths(),
            restored.indexed_owner_paths()
        );
        assert_eq!(
            resident
                .read_runtime_owner_search("src/lib.rs", &["resident".to_owned()], 8)
                .expect("resident owner search"),
            restored
                .read_runtime_owner_search("src/lib.rs", &["resident".to_owned()], 8)
                .expect("restored owner search")
        );
        assert_eq!(
            resident
                .read_runtime_selector(
                    crate::runtime_server_workspace::ExactProjectionKind::Source,
                    "rust://src/lib.rs#item/function/resident_before_durable",
                )
                .expect("resident selector"),
            restored
                .read_runtime_selector(
                    crate::runtime_server_workspace::ExactProjectionKind::Source,
                    "rust://src/lib.rs#item/function/resident_before_durable",
                )
                .expect("restored selector")
        );
    }

    #[tokio::test]
    async fn failed_durability_keeps_the_resident_generation_queryable() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let publisher_path = temporary.path().join("not-a-directory");
        std::fs::write(&publisher_path, b"blocks directory creation")
            .expect("durability failure fixture");
        let (current, durability, mut tasks, _) =
            publish_with_held_durability(publisher_path).await;
        tasks.recv().await.expect("durability task").await;

        assert_eq!(
            durability
                .borrow()
                .as_ref()
                .expect("durability receipt")
                .state,
            crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::Failed,
        );
        let backend = current
            .borrow()
            .clone()
            .expect("resident generation retained");
        let resident =
            crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
                crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(backend),
            )
            .expect("resident read client");
        assert!(
            resident
                .owner_snapshot("src/lib.rs")
                .expect("resident owner")
                .is_some()
        );
    }

    #[tokio::test]
    async fn unavailable_durability_lane_has_no_visible_resident_side_effect() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let project_root = temporary.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let generation = generation(&project_root);
        let prepared_index =
            WorkspaceMemoryBackend::prepare_index(&generation.owners, &generation.relations);
        let (current, _) = tokio::sync::watch::channel(None);
        let (durability, _) = tokio::sync::watch::channel(None);
        let overlays = crate::runtime_server_workspace::ResidentOverlayStore::new(None);
        let publisher = Arc::new(
            crate::runtime_server_workspace::WorkspaceGenerationPublisher::new(
                temporary.path().join("generation"),
            )
            .await
            .expect("generation publisher"),
        );
        let (durability_tasks, durability_requests) = tokio::sync::mpsc::unbounded_channel();
        drop(durability_requests);
        let error = publish_new_generation(
            &current,
            &durability,
            &overlays,
            publisher,
            "queue-closed".to_owned(),
            "workspace-resident-durability".to_owned(),
            generation,
            prepared_index,
            0,
            tokio::time::Instant::now(),
            Arc::new(RuntimeDataPlaneCounterState::default()),
            &durability_tasks,
        )
        .await
        .expect_err("closed durability lane");
        assert!(error.contains("durability attachment lane is unavailable"));
        assert!(current.borrow().is_none());
        assert!(durability.borrow().is_none());
    }

    #[test]
    fn direct_resident_projection_for_4096_owners_has_subsecond_cold_p95() {
        let temporary = tempfile::tempdir().expect("temporary runtime root");
        let generation = large_generation(&temporary.path().join("project"), 4_096);
        let mut samples = Vec::with_capacity(5);
        for _ in 0..5 {
            let started = std::time::Instant::now();
            let projection =
                crate::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient::from_generation(
                    Arc::clone(&generation),
                )
                .expect("direct resident projection");
            assert_eq!(projection.indexed_owner_count(), 4_096);
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[4];
        eprintln!("resident direct projection 4096-owner cold p95={p95:?}");
        assert!(
            p95 < std::time::Duration::from_secs(1),
            "direct resident projection must remain subsecond: p95={p95:?}"
        );
    }
}
