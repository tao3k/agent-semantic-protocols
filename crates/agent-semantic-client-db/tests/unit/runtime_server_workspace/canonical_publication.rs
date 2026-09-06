// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::sync::Arc;

use super::{DurabilityTask, RuntimeDataPlaneCounterState, publish_new_generation};
use crate::runtime_server_workspace::{
    WorkspaceGenerationBuild, WorkspaceGenerationDataPlaneClient, WorkspaceGenerationDataPlaneOpen,
    WorkspaceMemoryBackend, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
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
            native_syntax_diagnostic: None,
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

    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
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
        agent_semantic_artifacts::runtime_provider_execution_binding::RuntimeProviderExecutionBinding::build(
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
                native_syntax_diagnostic: None,
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
    let resident = crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
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
    let restored =
        crate::runtime_resident_read::RuntimeResidentReadClient::open(&pointer_path, &project_root)
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
    let (current, durability, mut tasks, _) = publish_with_held_durability(publisher_path).await;
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
    let resident = crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
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
