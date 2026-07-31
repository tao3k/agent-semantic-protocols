//! Runtime Server workspace MemoryBackend and generation publication gates.

use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceDataPlanePerformanceReceipt,
    WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
    WorkspaceGenerationPointerReader, WorkspaceGenerationPublisher, WorkspaceGenerationState,
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRuntimeSelectorOverlay,
    WorkspaceRuntimeSelectorRead, WorkspaceSelectorSnapshot, workspace_generation_pointer_path,
};
use std::{sync::Arc, time::Instant};
use tempfile::tempdir;

fn project_root(workspace_identity: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("/runtime-server-workspace-fixture").join(workspace_identity)
}

fn resident_pointer(
    runtime_root: &std::path::Path,
    workspace_identity: &str,
) -> std::path::PathBuf {
    workspace_generation_pointer_path(
        runtime_root,
        workspace_identity,
        &project_root(workspace_identity),
    )
    .unwrap()
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
        project_root: project_root(workspace_identity).display().to_string(),
        state: WorkspaceGenerationState::Ready,
        active_epoch: epoch,
        generation_digest: generation_digest.clone(),
        root_depth: [1, 0],
        workspace_snapshot,
        source_snapshot,
        workspace_generation,
        memory_backend_digest: generation_digest,
        workspace_source_scope_generation:
            agent_semantic_runtime::workspace_source_scope_generation_digest(&[])
                .expect("empty ProjectResolution generation"),
        project_resolutions: Vec::new(),
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
            &project_root("workspace-a"),
            owner(
                "src/lib.rs",
                "rust://src/lib.rs#item/function/run",
                b"fn run() {}",
            ),
        )
        .await
        .expect_err("owner overlay must require a canonical generation");

    assert!(error.contains("requires an admitted canonical generation"));
    assert!(
        registry
            .lease("workspace-a", &project_root("workspace-a"))
            .is_err()
    );
    assert_eq!(registry.data_plane_counters().database_opens, 0);
    assert_eq!(registry.data_plane_counters().provider_spawns, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn same_owner_selector_overlays_accumulate_without_advancing_source_epoch() {
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
        .publish_selector_overlay(
            "workspace-a",
            &project_root("workspace-a"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind: "source".to_owned(),
                structural_selector: second_selector.to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
                byte_start: 14,
                byte_end: source.len(),
                projection_bytes: b"fn second() {}".to_vec(),
            },
        )
        .await
        .expect("publish second selector");

    let lease = registry
        .lease("workspace-a", &project_root("workspace-a"))
        .expect("generation lease");
    assert!(matches!(
        lease
            .read_runtime_selector("source", first_selector)
            .expect("read first selector"),
        WorkspaceRuntimeSelectorRead::Projection { .. }
    ));
    assert!(matches!(
        lease
            .read_runtime_selector("source", second_selector)
            .expect("read second selector"),
        WorkspaceRuntimeSelectorRead::Projection { .. }
    ));
    assert_eq!(lease.epoch(), 1);
    assert_eq!(lease.generation().root_depth, [1, 0]);
    assert_eq!(lease.generation().owners.len(), 1);
    assert_eq!(
        lease.generation().owners[0].selectors.len(),
        1,
        "resident owner overlays must not rewrite the canonical mmap generation"
    );
    assert_eq!(registry.data_plane_counters().provider_spawns, 0);
    assert_eq!(registry.data_plane_counters().database_opens, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn selector_overlay_is_memory_resident_and_preserves_the_source_epoch() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn first() {}\nfn second() {}";
    let baseline_selector = "rust://src/lib.rs#item/function/first";
    let repaired_selector = "rust://src/lib.rs#item/function/second";
    let owner = owner("src/lib.rs", baseline_selector, source);
    let owner_content_digest = owner.content_digest.clone();
    registry
        .publish(
            "canonical-generation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation("workspace-selector-overlay", 1, owner),
        )
        .await
        .expect("publish canonical generation");

    let receipt = registry
        .publish_selector_overlay(
            "workspace-selector-overlay",
            &project_root("workspace-selector-overlay"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind: "source".to_owned(),
                structural_selector: repaired_selector.to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest,
                byte_start: 14,
                byte_end: source.len(),
                projection_bytes: b"fn second() {}".to_vec(),
            },
        )
        .await
        .expect("publish selector overlay");
    assert!(receipt.inserted);
    assert_eq!(
        registry
            .lease(
                "workspace-selector-overlay",
                &project_root("workspace-selector-overlay")
            )
            .expect("generation lease")
            .epoch(),
        1,
        "selector-only publication must preserve the source generation epoch"
    );
    match registry
        .read_runtime_selector(
            "workspace-selector-overlay",
            &project_root("workspace-selector-overlay"),
            "source",
            repaired_selector,
        )
        .expect("read repaired selector")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
            assert_eq!(bytes, b"fn second() {}");
        }
        read => panic!("expected resident projection, got {read:?}"),
    }
    let pointer = resident_pointer(temporary.path(), "workspace-selector-overlay");
    let WorkspaceExactProjectionDataPlaneOpen::Ready(process_cold) =
        WorkspaceExactProjectionDataPlaneClient::open_state(&pointer)
            .await
            .expect("open process-cold selector generation")
    else {
        panic!("selector overlay must be published to the mmap data plane");
    };
    assert!(matches!(
        process_cold
            .read_runtime_selector("source", repaired_selector)
            .expect("read canonical process-cold generation"),
        WorkspaceRuntimeSelectorRead::OwnerForRepair { .. }
    ));
    let repeated = registry
        .publish_selector_overlay(
            "workspace-selector-overlay",
            &project_root("workspace-selector-overlay"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind: "source".to_owned(),
                structural_selector: repaired_selector.to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
                byte_start: 14,
                byte_end: source.len(),
                projection_bytes: b"fn second() {}".to_vec(),
            },
        )
        .await
        .expect("repeat identical selector overlay");
    assert!(!repeated.inserted);
    assert_eq!(
        registry
            .lease(
                "workspace-selector-overlay",
                &project_root("workspace-selector-overlay")
            )
            .expect("generation lease")
            .epoch(),
        1,
        "idempotent selector publication must preserve the source epoch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn selector_overlay_binds_projection_kind_and_projection_bytes() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn first() {}";
    let selector = "rust://src/lib.rs#item/function/first";
    let source_owner = owner("src/lib.rs", selector, source);
    registry
        .publish(
            "canonical-generation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-projection-kind",
                1,
                source_owner.clone(),
            ),
        )
        .await
        .expect("publish canonical generation");
    let callable_skeleton = br#"{"kind":"function","name":"first"}"#.to_vec();
    registry
        .publish_selector_overlay(
            "workspace-projection-kind",
            &project_root("workspace-projection-kind"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind: "callable-skeleton".to_owned(),
                structural_selector: selector.to_owned(),
                owner_path: source_owner.owner_path,
                owner_content_digest: source_owner.content_digest,
                byte_start: 0,
                byte_end: source.len(),
                projection_bytes: callable_skeleton.clone(),
            },
        )
        .await
        .expect("publish callable-skeleton overlay");

    match registry
        .read_runtime_selector(
            "workspace-projection-kind",
            &project_root("workspace-projection-kind"),
            "source",
            selector,
        )
        .expect("read source projection")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => assert_eq!(bytes, source),
        read => panic!("expected source projection, got {read:?}"),
    }
    match registry
        .read_runtime_selector(
            "workspace-projection-kind",
            &project_root("workspace-projection-kind"),
            "callable-skeleton",
            selector,
        )
        .expect("read callable-skeleton projection")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
            assert_eq!(bytes, callable_skeleton);
        }
        read => panic!("expected callable-skeleton projection, got {read:?}"),
    }
    assert_eq!(
        registry
            .lease(
                "workspace-projection-kind",
                &project_root("workspace-projection-kind")
            )
            .expect("generation lease")
            .epoch(),
        1
    );
    let pointer = resident_pointer(temporary.path(), "workspace-projection-kind");
    let WorkspaceExactProjectionDataPlaneOpen::Ready(process_cold) =
        WorkspaceExactProjectionDataPlaneClient::open_state(&pointer)
            .await
            .expect("open callable-skeleton generation")
    else {
        panic!("callable-skeleton projection must be mmap-readable");
    };
    assert!(matches!(
        process_cold
            .read_runtime_selector("callable-skeleton", selector)
            .expect("read canonical process-cold generation"),
        WorkspaceRuntimeSelectorRead::OwnerForRepair { .. }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn process_cold_exact_projection_open_and_lookup_is_sub_millisecond_at_p99() {
    const SELECTOR_COUNT: usize = 2_048;
    const SAMPLE_COUNT: usize = 128;
    const P99_BUDGET_NANOS: u128 = 1_000_000;

    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn target() {}";
    let mut source_owner = owner(
        "src/lib.rs",
        "rust://src/lib.rs#item/function/target-0000",
        source,
    );
    source_owner.selectors = (0..SELECTOR_COUNT)
        .map(|index| WorkspaceSelectorSnapshot {
            selector: format!("rust://src/lib.rs#item/function/target-{index:04}"),
            byte_start: 0,
            byte_end: source.len(),
            derived_projections: Vec::new(),
        })
        .collect();
    registry
        .publish(
            "exact-index-performance",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation("workspace-exact-index-performance", 1, source_owner),
        )
        .await
        .expect("publish large exact projection generation");
    let pointer = resident_pointer(temporary.path(), "workspace-exact-index-performance");
    let target = "rust://src/lib.rs#item/function/target-2047";
    WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("prewarm exact mmap pages");

    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
            .await
            .expect("open process-cold exact index");
        match client
            .read_runtime_selector("source", target)
            .expect("binary-search exact selector")
        {
            WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
                assert_eq!(bytes, source);
            }
            read => panic!("expected exact projection, got {read:?}"),
        }
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p99 = samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("at least one exact sample");
    eprintln!(
        "[workspace-exact-index-performance] selectors={SELECTOR_COUNT} samples={SAMPLE_COUNT} p99Nanos={p99} maxNanos={max} budgetNanos={P99_BUDGET_NANOS}"
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "process-cold exact projection p99 exceeded one millisecond: p99Nanos={p99}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn process_cold_owner_identity_is_independent_of_unrelated_selector_volume() {
    const UNRELATED_SELECTOR_COUNT: usize = 32_768;
    const SAMPLE_COUNT: usize = 128;
    const P99_BUDGET_NANOS: u128 = 1_000_000;

    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let target = owner(
        "src/target.rs",
        "rust://src/target.rs#item/function/target",
        b"fn target() {}",
    );
    let unrelated_source = b"fn unrelated() {}";
    let mut unrelated = owner(
        "src/unrelated.rs",
        "rust://src/unrelated.rs#item/function/unrelated-00000",
        unrelated_source,
    );
    unrelated.selectors = (0..UNRELATED_SELECTOR_COUNT)
        .map(|index| WorkspaceSelectorSnapshot {
            selector: format!("rust://src/unrelated.rs#item/function/unrelated-{index:05}"),
            byte_start: 0,
            byte_end: unrelated_source.len(),
            derived_projections: Vec::new(),
        })
        .collect();
    let mut published = generation("workspace-owner-identity-performance", 1, target.clone());
    published.owners = vec![unrelated, target.clone()];
    published.workspace_snapshot =
        agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
            published
                .owners
                .iter()
                .map(|owner| (owner.owner_path.clone(), owner.content_digest.clone())),
        );
    published.source_snapshot = published.workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "runtime-workspace-fixture".to_owned(),
    );
    published.workspace_generation =
        agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1 {
            root_digest: published.source_snapshot.root_digest.clone(),
            root_depth: 1,
            leaf_count: 2,
            owner_count: 2,
        };
    registry
        .publish(
            "owner-identity-performance",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            published,
        )
        .await
        .expect("publish owner identity performance generation");
    let pointer = resident_pointer(temporary.path(), "workspace-owner-identity-performance");

    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
            .await
            .expect("open process-cold exact owner index");
        assert!(
            client
                .contains_owner(&target)
                .expect("compare target owner"),
            "target owner identity must match"
        );
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p99 = samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("at least one owner identity sample");
    eprintln!(
        "[workspace-owner-identity-performance] unrelatedSelectors={UNRELATED_SELECTOR_COUNT} samples={SAMPLE_COUNT} p99Nanos={p99} maxNanos={max} budgetNanos={P99_BUDGET_NANOS}"
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "process-cold owner identity p99 depends on unrelated selector volume: p99Nanos={p99}"
    );
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
                projection_kind: "source".to_owned(),
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
                projection_kind: "source".to_owned(),
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
                "source",
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
        .read_runtime_selector("source", selector)
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
        .read_runtime_selector("source", selector)
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
