//! Runtime Server generation transitions, concurrency, and checkpoint gates.

use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceDataPlanePerformanceReceipt,
    WorkspaceGenerationPointerReader, WorkspaceGenerationPublisher, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorRead,
    WorkspaceSelectorSnapshot,
};
use std::{sync::Arc, time::Instant};
use tempfile::tempdir;

fn project_root(workspace_identity: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("/runtime-server-workspace-fixture").join(workspace_identity)
}

fn owner(path: &str, selector: &str, bytes: &[u8]) -> WorkspaceOwnerSnapshot {
    WorkspaceOwnerSnapshot {
        owner_path: path.to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(bytes).to_hex()),
        bytes: bytes.to_vec(),
        selectors: vec![WorkspaceSelectorSnapshot {
            selector: selector.to_owned(),
            byte_start: 0,
            byte_end: bytes.len(),
            derived_projections: Vec::new(),
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
        format!(
            "blake3-256:{}",
            blake3::hash(b"runtime-workspace-fixture-provider").to_hex()
        ),
    );
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root(workspace_identity).display().to_string(),
            active_epoch: epoch,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"runtime-workspace-fixture-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners: vec![owner],
        },
    )
    .expect("typed runtime workspace generation")
}

#[tokio::test(flavor = "multi_thread")]
async fn selector_overlay_rejects_owner_digest_drift() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn first() {}";
    registry
        .publish(
            "canonical-generation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-selector-drift",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/first",
                    source,
                ),
            ),
        )
        .await
        .expect("publish canonical generation");
    let error = registry
        .publish_selector_overlay(
            "workspace-selector-drift",
            &project_root("workspace-selector-drift"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind:
                    agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                structural_selector: "rust://src/lib.rs#item/function/second".to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest: "blake3-256:deadbeef".to_owned(),
                byte_start: 0,
                byte_end: source.len(),
                projection_bytes: source.to_vec(),
            },
        )
        .await
        .expect_err("digest drift must fail closed");
    assert!(error.contains("digest drift"));
}

#[tokio::test(flavor = "multi_thread")]
async fn source_generation_transition_invalidates_selector_overlays() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let first_source = b"fn first() {}";
    let selector = "rust://src/lib.rs#item/function/first";
    let first_owner = owner("src/lib.rs", selector, first_source);
    registry
        .publish(
            "canonical-generation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation("workspace-selector-invalidation", 1, first_owner.clone()),
        )
        .await
        .expect("publish canonical generation");
    registry
        .publish_selector_overlay(
            "workspace-selector-invalidation",
            &project_root("workspace-selector-invalidation"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind:
                    agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                structural_selector: "rust://src/lib.rs#item/function/repaired".to_owned(),
                owner_path: first_owner.owner_path,
                owner_content_digest: first_owner.content_digest,
                byte_start: 0,
                byte_end: first_source.len(),
                projection_bytes: first_source.to_vec(),
            },
        )
        .await
        .expect("publish selector overlay");
    registry
        .publish_owner_overlay(
            "source-generation-transition",
            "workspace-selector-invalidation",
            &project_root("workspace-selector-invalidation"),
            owner("src/lib.rs", selector, b"fn first() { changed(); }"),
        )
        .await
        .expect("publish changed owner");
    assert!(matches!(
        registry
            .read_runtime_selector(
                "workspace-selector-invalidation",
                &project_root("workspace-selector-invalidation"),
                agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                "rust://src/lib.rs#item/function/repaired",
            )
            .expect("read invalidated selector"),
        WorkspaceRuntimeSelectorRead::OwnerForRepair { .. }
    ));
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
    let old_lease = registry
        .lease("workspace-a", &project_root("workspace-a"))
        .expect("old generation lease");
    let selector = "rust://src/lib.rs#item/function/run";

    let receipt = registry
        .publish_owner_overlay(
            "publish-2",
            "workspace-a",
            &project_root("workspace-a"),
            owner(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/run",
                b"fn run() { changed() }",
            ),
        )
        .await
        .expect("publish second generation");
    assert!(receipt.old_generation_readable);
    match old_lease
        .read_runtime_selector(
            agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
            selector,
        )
        .expect("read old lease")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
            assert_eq!(bytes, b"fn run() {}");
        }
        read => panic!("expected old resident projection, got {read:?}"),
    }
    let current = registry
        .lease("workspace-a", &project_root("workspace-a"))
        .expect("current generation lease");
    assert_eq!(current.epoch(), 2);
    match current
        .read_runtime_selector(
            agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
            selector,
        )
        .expect("read current lease")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
            assert_eq!(bytes, b"fn run() { changed() }");
        }
        read => panic!("expected current resident projection, got {read:?}"),
    }
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
                    &project_root("workspace-a"),
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
    let _performance = crate::test_support::performance_lock();
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
                        .lease(&workspace_identity, &project_root(&workspace_identity))
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
    let snapshot = publisher
        .publish(source.into(), false)
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
            &project_root("workspace-a"),
            snapshot.mmap_segment_path.into(),
        )
        .await
        .expect("restore checkpoint");
    assert_eq!(receipt.counters.filesystem_reads, 1);
    assert_eq!(receipt.counters.database_opens, 0);
    assert_eq!(receipt.counters.provider_spawns, 0);
    assert_eq!(
        registry
            .lease("workspace-a", &project_root("workspace-a"))
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
            generation(
                "workspace-a",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/run",
                    b"fn run() {}",
                ),
            )
            .into(),
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
                        generation(
                            "workspace-a",
                            epoch,
                            owner(
                                "src/lib.rs",
                                "rust://src/lib.rs#item/function/run",
                                format!("fn run() {{ epoch_{epoch}() }}").as_bytes(),
                            ),
                        )
                        .into(),
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
    let mut retained_segments = Vec::new();
    let mut entries = tokio::fs::read_dir(temporary.path().join("published"))
        .await
        .expect("read published generation directory");
    while let Some(entry) = entries.next_entry().await.expect("read generation entry") {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if file_name.starts_with("generation-") && file_name.ends_with(".mmap") {
            retained_segments.push(file_name);
        }
    }
    retained_segments.sort();
    assert_eq!(
        retained_segments,
        [
            "generation-31.exact.mmap",
            "generation-31.mmap",
            "generation-31.search.mmap",
            "generation-32.exact.mmap",
            "generation-32.mmap",
            "generation-32.search.mmap",
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn published_search_generation_reads_lexical_and_owner_sections_without_full_generation() {
    let temporary = tempdir().expect("temporary runtime root");
    let publisher = WorkspaceGenerationPublisher::new(temporary.path().join("published"))
        .await
        .expect("generation publisher");
    publisher
        .publish(
            generation(
                "workspace-search",
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/run_search",
                    b"fn run_search() {}",
                ),
            )
            .into(),
            false,
        )
        .await
        .expect("publish searchable generation");

    let client = agent_semantic_client_db::runtime_server_workspace::
        WorkspaceSearchGenerationDataPlaneClient::open(
            publisher.pointer_path(),
            &project_root("workspace-search"),
        )
        .await
        .expect("open zero-copy search generation");
    let lookup = client
        .read_source_index("run_search", None, 8)
        .expect("read lexical section");
    assert_eq!(lookup.candidates.len(), 1);
    assert_eq!(lookup.candidates[0].path, "src/lib.rs");

    let read = client.read_owner("src/lib.rs").expect("read owner section");
    let agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner {
        owner,
        ..
    } = read
    else {
        panic!("published owner must be readable");
    };
    assert_eq!(owner.bytes, b"fn run_search() {}");
    assert_eq!(client.authority().active_epoch, 1);
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
    let snapshot = publisher
        .publish(source.into(), false)
        .await
        .expect("publish checkpoint");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime")).expect("registry");

    let receipt = registry
        .restore_checkpoint(
            "restore-1",
            "workspace-a",
            &project_root("workspace-a"),
            snapshot.mmap_segment_path.into(),
        )
        .await
        .expect("restore checkpoint");
    assert_eq!(receipt.counters.filesystem_reads, 1);
    assert_eq!(receipt.counters.database_opens, 0);
    assert_eq!(receipt.counters.provider_spawns, 0);
    assert_eq!(
        registry
            .lease("workspace-a", &project_root("workspace-a"))
            .expect("restored lease")
            .epoch(),
        7
    );
}
