// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use super::{DurabilityTask, RuntimeDataPlaneCounterState, publish_new_generation};
use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceGenerationBuild, WorkspaceGenerationDataPlaneClient,
    WorkspaceGenerationDataPlaneOpen, WorkspaceMemoryBackend, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot,
};

fn execution_bundle_binding()
-> agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding {
    let digest = |byte: char| {
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(&format!(
            "blake3-256:{}",
            std::iter::repeat_n(byte, 64).collect::<String>()
        ))
        .expect("canonical fixture digest")
    };
    agent_semantic_artifacts::runtime_artifact_slots::RuntimeArtifactBundleBinding::new(
        digest('a'),
        digest('b'),
        digest('c'),
        digest('d'),
        digest('e'),
    )
}

fn execution_activation()
-> agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent {
    let artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(&format!(
            "blake3-256:{}",
            "1".repeat(64)
        ))
        .expect("canonical artifact digest");
    agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactActivationEvent {
        schema_id: "agent.semantic-protocols.runtime-artifact-activation".into(),
        schema_version: 1,
        activation_generation: 42,
        bundle_digest:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::parse(
                &format!("blake3-256:{}", "2".repeat(64)),
            )
            .expect("canonical bundle digest"),
        artifact_digest: artifact_digest.clone(),
        artifact_path: "/runtime/asp".into(),
        candidate_slot_path: "/runtime/candidate".into(),
        previous_artifact_digest: None,
        artifact_mode: "release".into(),
        published_at_unix_millis: 123,
        publication_nonce: "publication-1".into(),
        candidate_identity:
            agent_semantic_artifacts::runtime_artifact_activation::RuntimeArtifactCandidateIdentityReceipt {
                artifact_digest,
                artifact_path: "/runtime/asp".into(),
                stable_path: "/runtime/stable/asp".into(),
                artifact_mode: "release".into(),
                publication_nonce: "publication-1".into(),
            },
    }
}

fn execution_host_workspace() -> agent_semantic_content_identity::HostWorkspaceInitializationBinding
{
    let project_workspace = agent_semantic_content_identity::ProjectWorkspaceBinding::new(
        "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/main",
        ".",
        "cross-machine",
        Vec::new(),
    )
    .expect("valid Project Workspace");
    agent_semantic_content_identity::HostWorkspaceInitializationBinding::new(
        project_workspace,
        "worktree-main",
    )
    .expect("valid Host workspace binding")
}

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
    generation_from_owners_and_relations(project_root, owners, Vec::new())
}

fn generation_from_owners_and_relations(
    project_root: &std::path::Path,
    owners: Vec<WorkspaceOwnerSnapshot>,
    relations: Vec<crate::ClientDbSourceIndexOwnedRelation>,
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
            format!("blake3-256:{}", "2".repeat(64)),
            format!("blake3-256:{}", "e".repeat(64)),
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
                auxiliary_owners: Vec::new(),
                owners,
                relations,
            },
        )
        .expect("resident generation"),
    )
}

#[tokio::test]
async fn topology_source_segments_preserve_owner_attribution_across_resident_and_mmap() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let project_root = temporary.path().join("project");
    let first_selector = "rust://src/lib.rs#item/function/refresh";
    let second_selector = "rust://src/lib.rs#item/function/publish";
    let bytes = b"fn refresh() {}\nfn publish() {}\n".to_vec();
    let generation = generation_from_owners_and_relations(
        &project_root,
        vec![WorkspaceOwnerSnapshot {
            authority: None,
            owner_path: "src/lib.rs".to_owned(),
            content_digest: format!("blake3-256:{}", blake3::hash(&bytes).to_hex()),
            native_syntax_diagnostic: None,
            selectors: vec![
                WorkspaceSelectorSnapshot {
                    selector: first_selector.to_owned(),
                    byte_start: 0,
                    byte_end: 15,
                    query_keys: vec!["refresh".to_owned()],
                    derived_projections: Vec::new(),
                },
                WorkspaceSelectorSnapshot {
                    selector: second_selector.to_owned(),
                    byte_start: 16,
                    byte_end: bytes.len(),
                    query_keys: vec!["publish".to_owned()],
                    derived_projections: Vec::new(),
                },
            ],
            bytes,
        }],
        vec![crate::ClientDbSourceIndexOwnedRelation {
            owner_path: "src/lib.rs".into(),
            relation: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation {
                from: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                    kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                    id: first_selector.to_owned(),
                },
                kind: "calls".into(),
                to: agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint {
                    kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Item,
                    id: second_selector.to_owned(),
                },
            },
        }],
    );
    let resident =
        crate::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient::from_generation(
            Arc::clone(&generation),
        )
        .expect("resident search projection");
    let resident_segments = resident
        .topology_source_segments()
        .expect("resident topology source segments");

    let publisher = crate::runtime_server_workspace::WorkspaceGenerationPublisher::new(
        temporary.path().join("runtime"),
    )
    .await
    .expect("workspace generation publisher");
    publisher
        .publish(Arc::clone(&generation), false)
        .await
        .expect("durably publish generation");
    let restored = crate::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient::open(
        publisher.pointer_path(),
        &project_root,
    )
    .await
    .expect("mapped search projection");
    let restored_segments = restored
        .topology_source_segments()
        .expect("mapped topology source segments");

    assert_eq!(restored.resident_grep_corpus().corpus_heap_bytes(), 0);
    assert_eq!(restored.resident_grep_index_stats().artifact_heap_bytes, 0);
    for client in [&resident, &restored] {
        assert_eq!(
            client
                .read_admitted_selector_slice(first_selector, 3..10)
                .unwrap(),
            b"refresh"
        );
        assert!(
            client
                .read_admitted_selector_slice(first_selector, 14..18)
                .is_err()
        );
        assert!(
            client
                .read_admitted_selector_slice(first_selector, 8..8)
                .is_err()
        );
        assert!(
            client
                .read_admitted_selector_slice("rust://src/lib.rs#item/function/missing", 3..10)
                .is_err()
        );
    }
    assert_eq!(resident_segments, restored_segments);
    assert_eq!(resident_segments.len(), 1);
    assert_eq!(resident_segments[0].owner_path, "src/lib.rs");
    assert_eq!(
        resident_segments[0].selectors,
        [second_selector, first_selector]
    );
    assert_eq!(resident_segments[0].relations.len(), 1);
    assert_eq!(
        resident_segments[0].relations[0].owner_path.as_str(),
        "src/lib.rs"
    );
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
    tokio::task::JoinHandle<
        Result<crate::runtime_server_workspace::WorkspaceRecoveryReceipt, String>,
    >,
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
    let publication_current = current.clone();
    let publication_durability = durability.clone();
    let publication = tokio::spawn(async move {
        publish_new_generation(
            &publication_current,
            &publication_durability,
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
    });
    (
        current,
        durability_observer,
        durability_requests,
        pointer_path,
        publication,
    )
}

#[tokio::test]
async fn resident_read_publishes_before_durable_commit_then_restart_restore_succeeds() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let project_root = temporary.path().join("project");
    let (current, durability, mut tasks, pointer_path, publication) =
        publish_with_held_durability(temporary.path().join("generation")).await;

    let publication_receipt = tokio::time::timeout(std::time::Duration::from_secs(2), publication)
        .await
        .expect("ResidentReady must not await canonical durability")
        .expect("publication task")
        .expect("resident publication");
    assert_eq!(
        publication_receipt.state,
        crate::runtime_server_workspace::WorkspaceGenerationState::Ready
    );
    assert!(!pointer_path.exists(), "durability must still be blocked");
    let backend = current
        .borrow()
        .clone()
        .expect("resident generation before durability");
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
    assert_eq!(
        durability
            .borrow()
            .as_ref()
            .expect("resident durability receipt")
            .state,
        crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::ResidentReady,
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
    for owner in resident.indexed_owner_paths() {
        assert!(resident.contains_indexed_owner(&owner));
        assert!(restored.contains_indexed_owner(&owner));
    }
    assert!(!resident.contains_indexed_owner("not-admitted.rs"));
    assert!(!restored.contains_indexed_owner("not-admitted.rs"));
    assert_eq!(
        resident.indexed_owner_count(),
        restored.indexed_owner_count()
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
async fn failed_durability_preserves_resident_but_rejects_restart_restore() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let publisher_path = temporary.path().join("not-a-directory");
    std::fs::write(&publisher_path, b"blocks directory creation")
        .expect("durability failure fixture");
    let (current, durability, mut tasks, _, publication) =
        publish_with_held_durability(publisher_path).await;
    publication
        .await
        .expect("publication task")
        .expect("resident publication succeeds independently");
    assert!(
        current.borrow().is_some(),
        "validated resident generation survives attachment failure"
    );
    tasks.recv().await.expect("durability task").await;

    assert_eq!(
        durability
            .borrow()
            .as_ref()
            .expect("durability receipt")
            .state,
        crate::runtime_server_workspace::WorkspaceGenerationDurabilityState::Failed,
    );
    assert!(current.borrow().is_some());
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

#[tokio::test]
async fn execution_product_from_durable_pointer_binds_logical_source_index_exactly() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let project_root = temporary.path().join("project");
    std::fs::create_dir_all(&project_root).expect("project root");
    let generation_directory = temporary.path().join("generation");
    let execution_directory = temporary.path().join("execution");
    let publisher =
        crate::runtime_server_workspace::WorkspaceGenerationPublisher::new(generation_directory)
            .await
            .expect("generation publisher");
    let generation = generation(&project_root);
    let activation = execution_activation();
    let bundle = execution_bundle_binding();

    let before_source =
        crate::runtime_server_workspace::publish_runtime_workspace_execution_product(
            publisher.pointer_path(),
            &execution_directory,
            execution_host_workspace(),
            &activation,
            &activation.bundle_digest,
            &bundle,
        )
        .await
        .expect_err("execution publication cannot precede source durability");
    assert!(before_source.contains("active-workspace-generation-required"));

    let source_snapshot = publisher
        .publish(Arc::clone(&generation), false)
        .await
        .expect("durably publish source generation");
    let execution_publication =
        crate::runtime_server_workspace::publish_runtime_workspace_execution_product(
            publisher.pointer_path(),
            &execution_directory,
            execution_host_workspace(),
            &activation,
            &activation.bundle_digest,
            &bundle,
        )
        .await
        .expect("publish source-bound execution product");

    assert_eq!(
        execution_publication.generation_digest.as_str(),
        source_snapshot.generation_digest
    );
    assert_eq!(
        execution_publication.source_root_digest.as_str(),
        source_snapshot.source_root_digest
    );
    assert_eq!(
        execution_publication
            .content_publication_commit
            .identity()
            .source_index_digest,
        generation
            .runtime_provider_execution_binding
            .as_ref()
            .expect("provider execution binding")
            .source_index_digest
    );
    assert_eq!(
        crate::runtime_server_workspace::RuntimeWorkspaceExecutionPublicationStore::read_active(
            &execution_directory,
        )
        .await
        .expect("read exact source-bound execution product"),
        execution_publication
    );
}

#[test]
fn resident_execution_product_does_not_require_a_durable_pointer() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let project_root = temporary.path().join("project");
    let generation = generation(&project_root);
    let logical_source_index_digest = generation
        .runtime_provider_execution_binding
        .as_ref()
        .expect("provider execution binding")
        .source_index_digest
        .clone();
    let resident = crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
        crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(Arc::new(
            WorkspaceMemoryBackend::from_validated_generation(generation.as_ref().clone())
                .expect("resident backend"),
        )),
    )
    .expect("resident read client");
    let activation = execution_activation();
    let bundle = execution_bundle_binding();
    let publication =
        crate::runtime_server_workspace::compose_runtime_workspace_execution_product_from_resident(
            &resident,
            execution_host_workspace(),
            &activation,
            &activation.bundle_digest,
            &bundle,
        )
        .expect("resident execution publication");

    assert_eq!(
        publication
            .content_publication_commit
            .identity()
            .source_index_digest,
        logical_source_index_digest
    );
    assert!(
        !temporary.path().join("generation.pointer").exists(),
        "logical composition must not manufacture physical durability"
    );
}

#[test]
fn resident_read_handles_share_the_generation_admitted_search_data_plane() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let generation = generation(&temporary.path().join("project"));
    let backend = Arc::new(
        WorkspaceMemoryBackend::from_validated_generation(generation.as_ref().clone())
            .expect("resident backend"),
    );
    let first = crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
        crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(Arc::clone(
            &backend,
        )),
    )
    .expect("first resident read client");
    let second = crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
        crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(backend),
    )
    .expect("second resident read client");

    assert!(
        first.shares_search_data_plane_with(&second),
        "opening another read handle must clone the admitted Arc, not rebuild the generation"
    );
}

#[test]
fn resident_read_handle_open_for_4096_owners_has_submillisecond_p99() {
    const SAMPLE_COUNT: usize = 1_024;
    const P99_BUDGET_NANOS: u128 = 1_000_000;
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let generation = large_generation(&temporary.path().join("project"), 4_096);
    let backend = Arc::new(
        WorkspaceMemoryBackend::from_validated_generation(generation.as_ref().clone())
            .expect("resident backend"),
    );
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        let resident =
            crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
                crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(
                    Arc::clone(&backend),
                ),
            )
            .expect("resident read client");
        std::hint::black_box(resident.generation_digest());
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p99 = samples[(SAMPLE_COUNT * 99 / 100).saturating_sub(1)];
    eprintln!(
        "resident-read-handle ownerCount=4096 samples={SAMPLE_COUNT} p99Nanos={p99} budgetNanos={P99_BUDGET_NANOS}"
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "resident read-handle open p99 exceeded 1ms: p99Nanos={p99}"
    );
}

#[test]
fn resident_query_uses_the_exact_index_for_the_last_of_4096_owners() {
    const SAMPLE_COUNT: usize = 1_024;
    const P99_BUDGET_NANOS: u128 = 1_000_000;
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let generation = large_generation(&temporary.path().join("project"), 4_096);
    let backend = Arc::new(
        WorkspaceMemoryBackend::from_validated_generation(generation.as_ref().clone())
            .expect("resident backend"),
    );
    let resident = crate::runtime_resident_read::RuntimeResidentReadClient::from_resident_lease(
        crate::runtime_server_workspace::WorkspaceGenerationLease::from_backend(backend),
    )
    .expect("resident read client");
    let target = "rust://src/generated/owner_4095.rs#item/function/owner_4095";
    let expected = b"pub fn owner_4095() {}\n";
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = std::time::Instant::now();
        let read = resident
            .read_runtime_selector(ExactProjectionKind::Source, target)
            .expect("indexed resident Query");
        samples.push(started.elapsed().as_nanos());
        match read {
            WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
                assert_eq!(bytes, expected);
            }
            other => panic!("expected exact resident projection, got {other:?}"),
        }
    }
    samples.sort_unstable();
    let p50 = samples[SAMPLE_COUNT * 50 / 100 - 1];
    let p95 = samples[SAMPLE_COUNT * 95 / 100 - 1];
    let p99 = samples[SAMPLE_COUNT * 99 / 100 - 1];
    eprintln!(
        "resident-query-exact-index ownerCount=4096 targetOwnerOrdinal=4095 samples={SAMPLE_COUNT} p50Nanos={p50} p95Nanos={p95} p99Nanos={p99} budgetNanos={P99_BUDGET_NANOS}"
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "resident Query exact-index p99 exceeded 1ms: p99Nanos={p99}"
    );
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
