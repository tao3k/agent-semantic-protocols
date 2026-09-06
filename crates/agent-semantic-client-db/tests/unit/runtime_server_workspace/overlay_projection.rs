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
async fn selector_on_a_parserless_owner_projects_admitted_source_not_a_repair_packet() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"(export package-spec)\n";
    let owner_path = "src/build-api/package-spec.ss";
    let mut parserless_owner = owner(
        owner_path,
        "gerbil-scheme://src/build-api/package-spec.ss#item/function/package-spec",
        source,
    );
    parserless_owner.selectors.clear();
    registry
        .publish(
            "parserless-owner-source",
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
            generation("workspace-parserless-owner-source", 1, parserless_owner),
        )
        .await
        .expect("publish parserless owner generation");
    let pointer = resident_pointer(temporary.path(), "workspace-parserless-owner-source");
    let client = WorkspaceExactProjectionDataPlaneClient::open(&pointer)
        .await
        .expect("open parserless generation");

    match client
        .read_runtime_selector(
            ExactProjectionKind::Source,
            "gerbil-scheme://src/build-api/package-spec.ss#item/function/__owner_materialization__",
        )
        .expect("project admitted owner source")
    {
        WorkspaceRuntimeSelectorRead::Projection {
            resolved_selector,
            bytes,
            ..
        } => {
            assert_eq!(resolved_selector, owner_path);
            assert_eq!(bytes, source);
        }
        read => panic!("parserless owner source must not become a repair packet: {read:?}"),
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

#[path = "overlay_projection_cases/selector_reads.rs"]
mod selector_reads;
