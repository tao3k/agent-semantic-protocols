//! Runtime Server generation overlays and process-cold exact projection gates.

use agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind;
use agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceExactProjectionDataPlaneClient;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceExactProjectionDataPlaneOpen;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceMemoryGeneration;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot;
use std::time::Instant;
use tempfile::tempdir;

#[path = "overlay_projection_fixture.rs"]
mod fixture;
use fixture::generation;
use fixture::generation_with_owners;
use fixture::owner;
use fixture::project_root;
use fixture::resident_pointer;

#[path = "selector_query_keys.rs"]
mod selector_query_keys;

#[tokio::test(flavor = "multi_thread")]
async fn process_cold_exact_projection_relocates_by_canonical_item_identity() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let live = owner(
        "src/new_owner.rs",
        "rust://src/new_owner.rs#item/function/run",
        b"fn run() { relocated(); }",
    );
    registry
        .publish(
            "exact-relocation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation("workspace-exact-relocation", 1, live),
        )
        .await
        .expect("publish relocated generation");
    let pointer = resident_pointer(temporary.path(), "workspace-exact-relocation");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open relocated exact index");

    match client
        .read_runtime_selector(
            ExactProjectionKind::Source,
            "rust://src/old_owner.rs#item/function/run",
        )
        .expect("resolve moved canonical identity")
    {
        WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
            assert_eq!(bytes, b"fn run() { relocated(); }");
        }
        read => panic!("moved selector must resolve from the active generation: {read:?}"),
    }

    match client
        .read_runtime_selector(
            ExactProjectionKind::CallableSkeleton,
            "rust://src/old_owner.rs#item/function/run",
        )
        .expect("resolve moved item before projection availability")
    {
        WorkspaceRuntimeSelectorRead::ProjectionMissing {
            resolved_selector, ..
        } => assert_eq!(
            resolved_selector,
            "rust://src/new_owner.rs#item/function/run"
        ),
        read => panic!("missing projection must retain the relocated item identity: {read:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn normalized_owner_path_projects_complete_source_from_the_admitted_generation() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"(package: gerbil/reasoning/core)\n(export reason)";
    let owner_path = "scheme/reasoning/core.ss";
    registry
        .publish(
            "owner-path-source",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-owner-path-source",
                1,
                owner(
                    owner_path,
                    "gerbil-scheme://scheme/reasoning/core.ss#item/function/reason",
                    source,
                ),
            ),
        )
        .await
        .expect("publish Gerbil owner generation");
    let pointer = resident_pointer(temporary.path(), "workspace-owner-path-source");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open exact generation");

    match client
        .read_runtime_selector(ExactProjectionKind::Source, owner_path)
        .expect("project owner source")
    {
        WorkspaceRuntimeSelectorRead::Projection {
            resolved_selector,
            bytes,
            ..
        } => {
            assert_eq!(resolved_selector, owner_path);
            assert_eq!(bytes, source);
        }
        read => panic!("owner path must project complete committed source: {read:?}"),
    }

    match client
        .read_runtime_selector(ExactProjectionKind::CallableSkeleton, owner_path)
        .expect("fail closed for owner-level derived projection")
    {
        WorkspaceRuntimeSelectorRead::ProjectionMissing {
            resolved_selector, ..
        } => assert_eq!(resolved_selector, owner_path),
        read => panic!("owner-level derived projection must fail closed: {read:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn process_cold_exact_projection_relocates_scoped_impl_and_method_identity() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"impl Parse for CliOptions { fn parse(&self) {} }";
    let mut live = owner(
        "src/current.rs",
        "rust://src/current.rs#item/impl/CliOptions/scope/implementation-owner/type/CliOptions/scope/trait-owner/trait/Parse",
        source,
    );
    live.selectors.push(WorkspaceSelectorSnapshot {
        selector: "rust://src/current.rs#item/method/parse/scope/implementation-owner/type/CliOptions/scope/trait-owner/trait/Parse".to_owned(),
        byte_start: 0,
        byte_end: source.len(),
        query_keys: Vec::new(),
        derived_projections: Vec::new(),
    });
    registry
        .publish(
            "exact-scoped-relocation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation("workspace-exact-scoped-relocation", 1, live),
        )
        .await
        .expect("publish scoped relocation generation");
    let pointer = resident_pointer(temporary.path(), "workspace-exact-scoped-relocation");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open scoped relocation index");

    for stale in [
        "rust://src/previous.rs#item/impl/CliOptions/scope/implementation-owner/type/CliOptions/scope/trait-owner/trait/Parse",
        "rust://src/previous.rs#item/method/parse/scope/implementation-owner/type/CliOptions/scope/trait-owner/trait/Parse",
    ] {
        match client
            .read_runtime_selector(ExactProjectionKind::Source, stale)
            .expect("resolve scoped canonical identity")
        {
            WorkspaceRuntimeSelectorRead::Projection { bytes, .. } => {
                assert_eq!(bytes, source);
            }
            read => panic!("scoped moved selector must resolve from active generation: {read:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn process_cold_exact_projection_rejects_ambiguous_relocation() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let owners = vec![
        owner(
            "src/first.rs",
            "rust://src/first.rs#item/function/run",
            b"fn run() { first(); }",
        ),
        owner(
            "src/second.rs",
            "rust://src/second.rs#item/function/run",
            b"fn run() { second(); }",
        ),
    ];
    registry
        .publish(
            "exact-relocation-ambiguous",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation_with_owners("workspace-exact-relocation-ambiguous", 1, owners),
        )
        .await
        .expect("publish ambiguous generation");
    let pointer = resident_pointer(temporary.path(), "workspace-exact-relocation-ambiguous");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open ambiguous exact index");

    match client
        .read_runtime_selector(
            ExactProjectionKind::Source,
            "rust://src/old.rs#item/function/run",
        )
        .expect("classify ambiguous canonical identity")
    {
        WorkspaceRuntimeSelectorRead::RelocationAmbiguous { candidates, .. } => assert_eq!(
            candidates,
            vec![
                "rust://src/first.rs#item/function/run",
                "rust://src/second.rs#item/function/run",
            ]
        ),
        read => panic!("duplicate canonical identity must be ambiguous: {read:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn warm_canonical_relocation_has_sub_250us_p95_and_bounded_p99() {
    let _performance = crate::test_support::performance_lock();
    const UNRELATED_SELECTOR_COUNT: usize = 32_768;
    const SAMPLE_COUNT: usize = 2_048;
    const TYPICAL_P95_BUDGET_NANOS: u128 = 250_000;
    const HARD_P99_BUDGET_NANOS: u128 = 10_000_000;

    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
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
            query_keys: Vec::new(),
            derived_projections: Vec::new(),
        })
        .collect();
    let target_source = b"fn target() { relocated(); }";
    let target = owner(
        "src/current.rs",
        "rust://src/current.rs#item/function/target",
        target_source,
    );
    registry
        .publish(
            "exact-relocation-performance",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation_with_owners(
                "workspace-exact-relocation-performance",
                1,
                vec![unrelated, target],
            ),
        )
        .await
        .expect("publish relocation performance generation");
    let pointer = resident_pointer(temporary.path(), "workspace-exact-relocation-performance");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open relocation index");
    let stale = "rust://src/previous.rs#item/function/target";
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut missing_projection_samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let read = client
            .read_runtime_selector(ExactProjectionKind::Source, stale)
            .expect("relocate selector");
        assert!(matches!(
            read,
            WorkspaceRuntimeSelectorRead::Projection { .. }
        ));
        samples.push(started.elapsed().as_nanos());

        let started = Instant::now();
        let read = client
            .read_runtime_selector(ExactProjectionKind::CallableSkeleton, stale)
            .expect("relocate selector before missing projection classification");
        assert!(matches!(
            read,
            WorkspaceRuntimeSelectorRead::ProjectionMissing { .. }
        ));
        missing_projection_samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    missing_projection_samples.sort_unstable();
    let p95 = samples[(SAMPLE_COUNT * 95).div_ceil(100) - 1];
    let p99 = samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let missing_projection_p95 = missing_projection_samples[(SAMPLE_COUNT * 95).div_ceil(100) - 1];
    let missing_projection_p99 = missing_projection_samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    eprintln!(
        "[workspace-exact-relocation-performance] unrelatedSelectors={UNRELATED_SELECTOR_COUNT} samples={SAMPLE_COUNT} sourceP95Nanos={p95} sourceP99Nanos={p99} missingProjectionP95Nanos={missing_projection_p95} missingProjectionP99Nanos={missing_projection_p99} typicalBudgetNanos={TYPICAL_P95_BUDGET_NANOS} hardBudgetNanos={HARD_P99_BUDGET_NANOS} subprocesses=0 dbOpens=0 sourceFilesRead=0"
    );
    assert!(
        p95 < TYPICAL_P95_BUDGET_NANOS,
        "canonical relocation p95 exceeded 250 microseconds: p95Nanos={p95}"
    );
    assert!(
        missing_projection_p95 < TYPICAL_P95_BUDGET_NANOS,
        "projection-independent relocation p95 exceeded 250 microseconds: p95Nanos={missing_projection_p95}"
    );
    assert!(
        p99 < HARD_P99_BUDGET_NANOS && missing_projection_p99 < HARD_P99_BUDGET_NANOS,
        "canonical relocation p99 exceeded ten milliseconds: sourceP99Nanos={p99} missingProjectionP99Nanos={missing_projection_p99}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_multi_workspace_relocation_is_lock_free_and_generation_stable() {
    let _performance = crate::test_support::performance_lock();
    const WORKSPACE_COUNT: usize = 4;
    const READERS_PER_WORKSPACE: usize = 4;
    const READS_PER_READER: usize = 2_048;
    const TOTAL_BUDGET_MILLIS: u128 = 1_000;

    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let mut clients = Vec::with_capacity(WORKSPACE_COUNT);
    for workspace_index in 0..WORKSPACE_COUNT {
        let workspace_identity = format!("workspace-relocation-pressure-{workspace_index}");
        let live = owner(
            "src/current.rs",
            "rust://src/current.rs#item/function/target",
            b"fn target() { relocated(); }",
        );
        registry
            .publish(
                format!("relocation-pressure-{workspace_index}"),
                agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
                generation(&workspace_identity, 1, live),
            )
            .await
            .expect("publish pressure generation");
        let pointer = resident_pointer(temporary.path(), &workspace_identity);
        clients.push(std::sync::Arc::new(
            WorkspaceExactProjectionDataPlaneClient::open(&pointer)
                .await
                .expect("open pressure exact index"),
        ));
    }

    let started = Instant::now();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(
        WORKSPACE_COUNT * READERS_PER_WORKSPACE,
    ));
    let mut readers = tokio::task::JoinSet::new();
    for client in clients {
        for _ in 0..READERS_PER_WORKSPACE {
            let client = std::sync::Arc::clone(&client);
            let barrier = std::sync::Arc::clone(&barrier);
            readers.spawn(async move {
                barrier.wait().await;
                let mut generation_digest = None;
                for iteration in 0..READS_PER_READER {
                    let projection_kind = if iteration % 2 == 0 {
                        ExactProjectionKind::Source
                    } else {
                        ExactProjectionKind::CallableSkeleton
                    };
                    let read = client
                        .read_runtime_selector(
                            projection_kind,
                            "rust://src/previous.rs#item/function/target",
                        )
                        .expect("concurrent relocated read");
                    let observed = match read {
                        WorkspaceRuntimeSelectorRead::Projection {
                            generation_digest, ..
                        }
                        | WorkspaceRuntimeSelectorRead::ProjectionMissing {
                            generation_digest,
                            ..
                        } => generation_digest,
                        read => panic!("concurrent relocation returned unstable state: {read:?}"),
                    };
                    match generation_digest.as_ref() {
                        Some(expected) => assert_eq!(expected, &observed),
                        None => generation_digest = Some(observed),
                    }
                }
            });
        }
    }
    while let Some(result) = readers.join_next().await {
        result.expect("concurrent relocation reader task");
    }
    let elapsed_millis = started.elapsed().as_millis();
    eprintln!(
        "[workspace-exact-relocation-concurrency] workspaces={WORKSPACE_COUNT} readersPerWorkspace={READERS_PER_WORKSPACE} readsPerReader={READS_PER_READER} totalReads={} elapsedMillis={elapsed_millis} budgetMillis={TOTAL_BUDGET_MILLIS} generationChanges=0 lockRetries=0 dbOpens=0 subprocesses=0",
        WORKSPACE_COUNT * READERS_PER_WORKSPACE * READS_PER_READER
    );
    assert!(
        elapsed_millis < TOTAL_BUDGET_MILLIS,
        "concurrent multi-workspace relocation exceeded budget: elapsedMillis={elapsed_millis}"
    );
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
                projection_kind: ExactProjectionKind::Source,
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
            .read_runtime_selector(ExactProjectionKind::Source, first_selector)
            .expect("read first selector"),
        WorkspaceRuntimeSelectorRead::Projection { .. }
    ));
    assert!(matches!(
        lease
            .read_runtime_selector(ExactProjectionKind::Source, second_selector)
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
                projection_kind: ExactProjectionKind::Source,
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
            ExactProjectionKind::Source,
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
            .read_runtime_selector(ExactProjectionKind::Source, repaired_selector)
            .expect("read canonical process-cold generation"),
        WorkspaceRuntimeSelectorRead::OwnerForRepair { .. }
    ));
    let repeated = registry
        .publish_selector_overlay(
            "workspace-selector-overlay",
            &project_root("workspace-selector-overlay"),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind: ExactProjectionKind::Source,
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
                projection_kind: ExactProjectionKind::CallableSkeleton,
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
            ExactProjectionKind::Source,
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
            ExactProjectionKind::CallableSkeleton,
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
    let process_cold_read = process_cold
        .read_runtime_selector(ExactProjectionKind::CallableSkeleton, selector)
        .expect("read canonical process-cold generation");
    assert!(
        matches!(
            &process_cold_read,
            WorkspaceRuntimeSelectorRead::ProjectionMissing { .. }
        ),
        "canonical process-cold generation leaked a resident-only selector overlay: {process_cold_read:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_projection_reads_share_one_watch_snapshot_with_bounded_warm_latency() {
    let _performance = crate::test_support::performance_lock();
    let temporary = tempdir().expect("temporary runtime root");
    let registry = std::sync::Arc::new(
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry"),
    );
    let source = b"fn projection_target() {}";
    let selector = "rust://src/lib.rs#item/function/projection_target";
    registry
        .publish(
            "projection-slot-generation",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-projection-slot",
                1,
                owner("src/lib.rs", selector, source),
            ),
        )
        .await
        .expect("publish projection generation");
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..256 {
        let registry = std::sync::Arc::clone(&registry);
        tasks.spawn(async move {
            registry
                .projection_search_generation_authority(
                    "workspace-projection-slot",
                    &project_root("workspace-projection-slot"),
                )
                .await
        });
    }
    while let Some(result) = tasks.join_next().await {
        let authority = result
            .expect("join projection read")
            .expect("read projection authority");
        assert_eq!(authority.workspace_id, "workspace-projection-slot");
    }
    assert_eq!(
        registry.workspace_count(),
        1,
        "the lifecycle publisher owns the one resident generation consumed by projection reads"
    );
    let mut samples = Vec::with_capacity(30);
    for _ in 0..30 {
        let started = Instant::now();
        registry
            .projection_search_generation_authority(
                "workspace-projection-slot",
                &project_root("workspace-projection-slot"),
            )
            .await
            .expect("read warm projection authority");
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p75 = samples[(samples.len() * 75).div_ceil(100) - 1];
    let max = *samples.last().expect("warm projection samples");
    eprintln!(
        "[runtime-resident-search-watch-performance] concurrentReads=256 writerResidents={} warmRuns=30 p75Nanos={p75} maxNanos={max} typicalBudgetNanos=1000000 hardBudgetNanos=10000000",
        registry.workspace_count(),
    );
    assert!(
        p75 < 1_000_000,
        "resident projection-slot p75 exceeded one millisecond: p75Nanos={p75}"
    );
    assert!(
        max < 10_000_000,
        "resident projection-slot maximum exceeded ten milliseconds: maxNanos={max}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn process_cold_exact_projection_has_sub_ms_p95_and_bounded_p99() {
    let _performance = crate::test_support::performance_lock();
    const SELECTOR_COUNT: usize = 2_048;
    const SAMPLE_COUNT: usize = 128;
    const P95_BUDGET_NANOS: u128 = 1_000_000;
    const P99_HARD_BUDGET_NANOS: u128 = 10_000_000;

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
            query_keys: Vec::new(),
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
            .read_runtime_selector(ExactProjectionKind::Source, target)
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
    let p95 = samples[(SAMPLE_COUNT * 95).div_ceil(100) - 1];
    let p99 = samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("at least one exact sample");
    eprintln!(
        "[workspace-exact-index-performance] selectors={SELECTOR_COUNT} samples={SAMPLE_COUNT} p95Nanos={p95} p99Nanos={p99} maxNanos={max} typicalBudgetNanos={P95_BUDGET_NANOS} hardBudgetNanos={P99_HARD_BUDGET_NANOS}"
    );
    assert!(
        p95 < P95_BUDGET_NANOS,
        "process-cold exact projection p95 exceeded one millisecond: p95Nanos={p95}"
    );
    assert!(
        p99 < P99_HARD_BUDGET_NANOS,
        "process-cold exact projection p99 exceeded ten milliseconds: p99Nanos={p99}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn process_cold_owner_snapshot_is_independent_of_unrelated_selector_volume() {
    let _performance = crate::test_support::performance_lock();
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
            query_keys: Vec::new(),
            derived_projections: Vec::new(),
        })
        .collect();
    let owners = vec![unrelated, target.clone()];
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        owners
            .iter()
            .map(|owner| (owner.owner_path.clone(), owner.content_digest.clone())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"owner-identity-performance-provider").to_hex()
        ),
    );
    let published = WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability: crate::fixture::overlay_projection_capability_manifest_fixture(),
            relations: Vec::new(),
            workspace_identity: "workspace-owner-identity-performance".to_owned(),
            project_root: project_root("workspace-owner-identity-performance")
                .display()
                .to_string(),
            active_epoch: 1,
            workspace_snapshot,
            content_search_generation: crate::fixture::content_search_generation_receipt(
                "workspace-owner-identity-performance",
                &source_snapshot,
            ),
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"owner-identity-performance-module-graph").to_hex()
            ),
            runtime_provider_execution_binding: None,
            project_resolutions: Vec::new(),
            owners,
        },
    )
    .expect("typed owner identity performance generation");
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
    let mut open_samples = Vec::with_capacity(SAMPLE_COUNT);
    let mut lookup_samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let started = Instant::now();
        let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
            .await
            .expect("open process-cold exact owner index");
        let opened = Instant::now();
        let snapshot = client
            .owner_snapshot(&target.owner_path)
            .expect("read target owner snapshot")
            .expect("target owner must exist");
        assert_eq!(snapshot, target);
        let completed = Instant::now();
        open_samples.push(opened.duration_since(started).as_nanos());
        lookup_samples.push(completed.duration_since(opened).as_nanos());
        samples.push(completed.duration_since(started).as_nanos());
    }
    samples.sort_unstable();
    open_samples.sort_unstable();
    lookup_samples.sort_unstable();
    let p99 = samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let open_p99 = open_samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let lookup_p99 = lookup_samples[(SAMPLE_COUNT * 99).div_ceil(100) - 1];
    let max = *samples.last().expect("at least one owner identity sample");
    eprintln!(
        "[workspace-owner-snapshot-performance] unrelatedSelectors={UNRELATED_SELECTOR_COUNT} samples={SAMPLE_COUNT} openP99Nanos={open_p99} lookupP99Nanos={lookup_p99} p99Nanos={p99} maxNanos={max} budgetNanos={P99_BUDGET_NANOS}"
    );
    assert!(
        p99 < P99_BUDGET_NANOS,
        "process-cold owner snapshot p99 depends on unrelated selector volume: p99Nanos={p99}"
    );
}
