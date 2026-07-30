//! Runtime Server workspace MemoryBackend and generation publication gates.

use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceDataPlanePerformanceReceipt,
    WorkspaceGenerationPointerReader, WorkspaceGenerationPublisher, WorkspaceGenerationState,
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceSelectorSnapshot,
};
use std::{sync::Arc, time::Instant};
use tempfile::tempdir;

fn owner(path: &str, selector: &str, bytes: &[u8]) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        owner_path: path.to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
        bytes: bytes.to_vec(),
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: bytes.len(),
        }],
    }
}

fn generation(
    workspace_identity: &str,
    epoch: u64,
    owner: WorkspaceOwnerSnapshot,
) -> WorkspaceMemoryGeneration {
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [(owner.owner_path.clone(), owner.content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "runtime-workspace-fixture".to_owned(),
    );
    let workspace_generation =
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
            root_digest: source_snapshot.root_digest.clone(),
            root_depth: 1,
            leaf_count: 1,
            owner_count: 1,
        };
    let generation_digest = format!(
        "blake3-256:{}",
        blake3::hash(format!("{workspace_identity}:{epoch}").as_bytes()).to_hex()
    );
    WorkspaceMemoryGeneration {
        workspace_identity: workspace_identity.to_owned(),
        state: WorkspaceGenerationState::Ready,
        active_epoch: epoch,
        generation_digest: generation_digest.clone(),
        root_depth: [1, 0],
        workspace_snapshot,
        source_snapshot,
        workspace_generation,
        memory_backend_digest: generation_digest,
        owners: vec![owner],
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn owner_overlay_cannot_manufacture_an_absent_canonical_generation() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let error = registry
        .publish_owner_overlay(
            "recover-1",
            "workspace-a",
            owner(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/run",
                b"fn run() {}",
            ),
        )
        .await
        .expect_err("owner overlay must require a canonical generation");

    assert!(error.contains("requires an admitted canonical generation"));
    assert!(registry.lease("workspace-a").is_err());
    assert_eq!(registry.data_plane_counters().database_opens, 0);
    assert_eq!(registry.data_plane_counters().provider_spawns, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn same_owner_overlay_accumulates_exact_selectors_without_provider_replay() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn first() {}\nfn second() {}";
    let first_selector = "rust://src/lib.rs#item/function/first";
    let second_selector = "rust://src/lib.rs#item/function/second";

    registry
        .publish(
            "first-selector",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                1,
                owner("src/lib.rs", first_selector, source),
            ),
        )
        .await
        .expect("publish first selector");
    registry
        .publish_owner_overlay(
            "second-selector",
            "workspace-a",
            owner("src/lib.rs", second_selector, source),
        )
        .await
        .expect("publish second selector");

    let lease = registry.lease("workspace-a").expect("generation lease");
    assert!(lease.project(first_selector).is_some());
    assert!(lease.project(second_selector).is_some());
    assert_eq!(lease.generation().root_depth, [1, 0]);
    assert_eq!(lease.generation().owners.len(), 1);
    assert_eq!(lease.generation().owners[0].selectors.len(), 2);
    assert_eq!(registry.data_plane_counters().provider_spawns, 0);
    assert_eq!(registry.data_plane_counters().database_opens, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn epoch_publication_keeps_the_previous_generation_readable() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    registry
        .publish(
            "publish-1",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/run",
                    b"fn run() {}",
                ),
            ),
        )
        .await
        .expect("publish first generation");
    let old_lease = registry.lease("workspace-a").expect("old generation lease");
    let old_projection = old_lease
        .project("rust://src/lib.rs#item/function/run")
        .expect("old projection");

    let receipt = registry
        .publish_owner_overlay(
            "publish-2",
            "workspace-a",
            owner(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/run",
                b"fn run() { changed() }",
            ),
        )
        .await
        .expect("publish second generation");
    assert!(receipt.old_generation_readable);
    assert_eq!(old_projection.bytes(), b"fn run() {}");
    let current = registry
        .lease("workspace-a")
        .expect("current generation lease");
    assert_eq!(current.epoch(), 2);
    assert_eq!(
        current
            .project("rust://src/lib.rs#item/function/run")
            .expect("current projection")
            .bytes(),
        b"fn run() { changed() }"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_sessions_share_one_writer_lane_and_monotonic_epochs() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry = Arc::new(
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry"),
    );
    let session_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(8)
        .max(16);
    registry
        .publish(
            "canonical-base",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/base",
                    b"fn base() {}",
                ),
            ),
        )
        .await
        .expect("publish canonical base generation");
    let mut tasks = Vec::with_capacity(session_count);
    for session in 0..session_count {
        let registry = Arc::clone(&registry);
        tasks.push(tokio::spawn(async move {
            let path = format!("src/session_{session}.rs");
            let selector = format!("rust://{path}#item/function/run");
            registry
                .publish_owner_overlay(
                    format!("session-{session}"),
                    "workspace-a",
                    owner(&path, &selector, b"fn run() {}"),
                )
                .await
        }));
    }
    let mut epochs = Vec::with_capacity(session_count);
    for task in tasks {
        epochs.push(
            task.await
                .expect("join concurrent session")
                .expect("publish concurrent overlay")
                .target_epoch,
        );
    }
    epochs.sort_unstable();
    assert_eq!(epochs, (2..=session_count as u64 + 1).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread")]
async fn warm_exact_projection_is_zero_io_and_sub_millisecond_at_p99() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry = Arc::new(
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry"),
    );
    let selector = "rust://src/lib.rs#item/function/run";
    let parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let workspace_count = parallelism.isqrt().max(2).min(parallelism.max(2));
    let sessions_per_workspace = parallelism.div_ceil(workspace_count).max(2);
    let samples_per_session = parallelism.saturating_mul(64).clamp(256, 2_048);
    for workspace in 0..workspace_count {
        registry
            .publish(
                format!("warm-{workspace}"),
                agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
                generation(
                    &format!("workspace-{workspace}"),
                    1,
                    owner("src/lib.rs", selector, b"fn run() {}"),
                ),
            )
            .await
            .expect("publish warm generation");
    }
    let baseline_counters = registry.data_plane_counters();

    let mut tasks = Vec::with_capacity(workspace_count * sessions_per_workspace);
    for workspace in 0..workspace_count {
        for _ in 0..sessions_per_workspace {
            let registry = Arc::clone(&registry);
            tasks.push(tokio::spawn(async move {
                let workspace_identity = format!("workspace-{workspace}");
                let mut samples = Vec::with_capacity(samples_per_session);
                for _ in 0..samples_per_session {
                    let started = Instant::now();
                    let lease = registry
                        .lease(&workspace_identity)
                        .expect("warm generation lease");
                    let projection = lease.project(selector).expect("warm exact projection");
                    assert_eq!(projection.bytes(), b"fn run() {}");
                    samples.push(u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX));
                }
                samples
            }));
        }
    }
    let mut samples = Vec::new();
    for task in tasks {
        samples.extend(task.await.expect("join warm query session"));
    }
    samples.sort_unstable();
    let receipt = WorkspaceDataPlanePerformanceReceipt {
        schema_id:
            "agent.semantic-protocols.runtime-server-workspace-data-plane-performance-receipt.v1"
                .to_owned(),
        schema_version: "1".to_owned(),
        workspace_count,
        session_count: workspace_count * sessions_per_workspace,
        sample_count: samples.len(),
        p50_micros: samples[samples.len() / 2],
        p99_micros: samples[(samples.len() * 99 / 100).min(samples.len() - 1)],
        max_micros: *samples.last().expect("latency sample"),
        counters: registry
            .data_plane_counters()
            .delta_since(&baseline_counters),
    };
    receipt.validate().expect("workspace data-plane gate");
    println!(
        "{}",
        serde_json::to_string(&receipt).expect("serialize performance receipt")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn checkpoint_restore_loads_once_then_serves_the_memory_backend() {
    let temporary = tempdir().expect("temporary runtime root");
    let publisher = WorkspaceGenerationPublisher::new(temporary.path().join("published"))
        .await
        .expect("generation publisher");
    let source = generation(
        "workspace-a",
        7,
        owner(
            "src/lib.rs",
            "rust://src/lib.rs#item/function/run",
            b"fn run() {}",
        ),
    );
    let (snapshot, _) = publisher
        .publish(&source, false)
        .await
        .expect("publish checkpoint");
    let pointer = WorkspaceGenerationPointerReader::open(publisher.pointer_path())
        .await
        .expect("open generation pointer");
    assert_eq!(pointer.read().expect("read generation pointer"), snapshot);
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime")).expect("registry");

    let receipt = registry
        .restore_checkpoint(
            "restore-1",
            "workspace-a",
            snapshot.mmap_segment_path.into(),
        )
        .await
        .expect("restore checkpoint");
    assert_eq!(receipt.counters.filesystem_reads, 1);
    assert_eq!(receipt.counters.database_opens, 0);
    assert_eq!(receipt.counters.provider_spawns, 0);
    assert_eq!(
        registry
            .lease("workspace-a")
            .expect("restored lease")
            .epoch(),
        7
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn generation_pointer_never_exposes_a_torn_epoch_during_publication() {
    let temporary = tempdir().expect("temporary runtime root");
    let publisher = Arc::new(
        WorkspaceGenerationPublisher::new(temporary.path().join("published"))
            .await
            .expect("generation publisher"),
    );
    publisher
        .publish(
            &generation(
                "workspace-a",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/run",
                    b"fn run() {}",
                ),
            ),
            false,
        )
        .await
        .expect("publish initial generation");
    let reader = WorkspaceGenerationPointerReader::open(publisher.pointer_path())
        .await
        .expect("open generation pointer");
    let writer = {
        let publisher = Arc::clone(&publisher);
        tokio::spawn(async move {
            for epoch in 2..=32 {
                publisher
                    .publish(
                        &generation(
                            "workspace-a",
                            epoch,
                            owner(
                                "src/lib.rs",
                                "rust://src/lib.rs#item/function/run",
                                format!("fn run() {{ epoch_{epoch}() }}").as_bytes(),
                            ),
                        ),
                        true,
                    )
                    .await
                    .expect("publish next generation");
            }
        })
    };
    for _ in 0..4_096 {
        let snapshot = reader.read().expect("consistent generation pointer");
        assert!((1..=32).contains(&snapshot.active_epoch));
        assert_eq!(snapshot.workspace_identity, "workspace-a");
        tokio::task::yield_now().await;
    }
    writer.await.expect("join generation publisher");
    assert_eq!(
        reader
            .read()
            .expect("read final generation pointer")
            .active_epoch,
        32
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_truncated_next_checkpoint_never_replaces_the_readable_generation() {
    let temporary = tempdir().expect("temporary runtime root");
    let publisher = WorkspaceGenerationPublisher::new(temporary.path().join("published"))
        .await
        .expect("generation publisher");
    let source = generation(
        "workspace-a",
        7,
        owner(
            "src/lib.rs",
            "rust://src/lib.rs#item/function/run",
            b"fn run() {}",
        ),
    );
    let (snapshot, _) = publisher
        .publish(&source, false)
        .await
        .expect("publish checkpoint");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime")).expect("registry");

    let receipt = registry
        .restore_checkpoint(
            "restore-1",
            "workspace-a",
            snapshot.mmap_segment_path.into(),
        )
        .await
        .expect("restore checkpoint");
    assert_eq!(receipt.counters.filesystem_reads, 1);
    assert_eq!(receipt.counters.database_opens, 0);
    assert_eq!(receipt.counters.provider_spawns, 0);
    assert_eq!(
        registry
            .lease("workspace-a")
            .expect("restored lease")
            .epoch(),
        7
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_shutdown_drains_every_workspace_writer_lane() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime")).expect("registry");
    registry
        .publish(
            "publish-before-shutdown",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/run",
                    b"fn run() {}",
                ),
            ),
        )
        .await
        .expect("publish before shutdown");
    let receipt = registry.shutdown().await.expect("drain writer lanes");
    receipt.validate().expect("shutdown receipt");
    assert_eq!(receipt.writer_lane_count, 1);
    assert_eq!(receipt.forced_abort_count, 0);
    let error = registry
        .publish_owner_overlay(
            "publish-after-shutdown",
            "workspace-a",
            owner(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/run",
                b"fn run() {}",
            ),
        )
        .await
        .expect_err("shutdown writer lane must reject new work");
    assert!(error.contains("writer lane is unavailable"));
}
