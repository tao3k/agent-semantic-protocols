//! Resident Runtime Server generation reuse regressions.

use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::runtime_server::{RuntimeServer, RuntimeServerExit};
use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRecoverySource, WorkspaceRuntimeSelectorOverlay, WorkspaceRuntimeSelectorRead,
    WorkspaceSearchGenerationAuthority, WorkspaceSelectorSnapshot,
    workspace_generation_pointer_path,
};
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession;
use std::sync::Arc;
use tempfile::tempdir;

use super::runtime_server_control::fixture_endpoint;

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

fn owner_with_range(
    path: &str,
    selector: &str,
    bytes: &[u8],
    byte_start: usize,
    byte_end: usize,
) -> WorkspaceOwnerSnapshot {
    let mut owner = owner(path, selector, bytes);
    owner.selectors[0].byte_start = byte_start;
    owner.selectors[0].byte_end = byte_end;
    owner
}

fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    owner: WorkspaceOwnerSnapshot,
) -> WorkspaceMemoryGeneration {
    generation_with_owners(workspace_identity, project_root, epoch, vec![owner])
}

fn generation_with_owners(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    owners: Vec<WorkspaceOwnerSnapshot>,
) -> WorkspaceMemoryGeneration {
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        owners
            .iter()
            .map(|owner| (owner.owner_path.clone(), owner.content_digest.clone())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"resident-ready-fixture-provider").to_hex()
        ),
    );
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
    projection_capability: agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector("blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(), "rust://fixture/src/lib.rs#item/function/fixture".to_owned(), "src/lib.rs".to_owned(), std::collections::BTreeSet::from([agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source])).expect("test projection capability manifest"),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: epoch,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"resident-ready-fixture-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners,
        },
    )
    .expect("typed resident ready generation")
}

#[tokio::test(flavor = "current_thread")]
async fn search_generation_authority_wire_size_is_constant_in_owner_count() {
    let temporary = tempdir().expect("temporary runtime root");
    let project_root = temporary.path().join("authority-wire-size");
    let registry = RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime"))
        .expect("resident registry");
    let owners = (0..512)
        .map(|index| {
            let path = format!("src/generated-{index}.rs");
            let selector = format!("rust://{path}#item/function/generated_{index}");
            owner(&path, &selector, b"fn generated() {}")
        })
        .collect();
    registry
        .publish(
            "authority-wire-size",
            WorkspaceRecoverySource::TursoGeneration,
            generation_with_owners("workspace-authority-wire-size", &project_root, 1, owners),
        )
        .await
        .expect("publish large resident generation");
    let lease = registry
        .lease("workspace-authority-wire-size", &project_root)
        .expect("large generation lease");
    let authority = WorkspaceSearchGenerationAuthority::from_lease(&lease);
    let wire = serde_json::to_vec(&authority).expect("encode compact authority");
    assert!(
        wire.len() < 2_048,
        "search generation authority must remain O(1) and below 2KiB: bytes={}",
        wire.len()
    );
    assert!(
        !wire
            .windows(b"src/generated-511.rs".len())
            .any(|window| window == b"src/generated-511.rs"),
        "search generation authority leaked workspace leaf paths"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn one_workspace_handle_isolates_same_owner_path_across_project_resolutions() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry = Arc::new(
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry"),
    );
    let workspace_identity = "workspace-monorepo";
    let first_root = temporary.path().join("packages/first");
    let second_root = temporary.path().join("packages/second");
    let selector = "rust://src/lib.rs#item/function/run";
    let first_bytes = b"fn run() { first_package() }";
    let second_bytes = b"fn run() { second_package() }";

    registry
        .publish(
            "publish-first-scope",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                workspace_identity,
                &first_root,
                1,
                owner("src/lib.rs", selector, first_bytes),
            ),
        )
        .await
        .expect("publish first project scope");
    registry
        .publish(
            "publish-second-scope",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                workspace_identity,
                &second_root,
                1,
                owner("src/lib.rs", selector, second_bytes),
            ),
        )
        .await
        .expect("publish second project scope");

    let mut readers = Vec::with_capacity(128);
    for index in 0..128 {
        let registry = Arc::clone(&registry);
        let first_root = first_root.clone();
        let second_root = second_root.clone();
        readers.push(tokio::spawn(async move {
            let (root, expected) = if index % 2 == 0 {
                (&first_root, first_bytes.as_slice())
            } else {
                (&second_root, second_bytes.as_slice())
            };
            for _ in 0..256 {
                let lease = registry
                    .lease(workspace_identity, root)
                    .expect("scoped generation lease");
                assert_eq!(
                    lease
                        .project(selector)
                        .expect("scoped exact projection")
                        .bytes(),
                    expected
                );
                tokio::task::yield_now().await;
            }
        }));
    }
    for reader in readers {
        reader.await.expect("join scoped reader");
    }

    assert_eq!(registry.workspace_count(), 1);
    let receipt = registry
        .shutdown()
        .await
        .expect("drain workspace writer lane");
    assert_eq!(receipt.workspace_count, 1);
    assert_eq!(receipt.writer_lane_count, 1);
    receipt.validate().expect("single-writer shutdown receipt");
}

#[tokio::test(flavor = "multi_thread")]
async fn ready_recovery_receipt_reuses_resident_generation_without_resetting_overlays() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let source = b"fn first() {}\nfn second() {}";
    let selector = "rust://src/lib.rs#item/function/second";

    registry
        .publish(
            "resident-generation",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                temporary.path(),
                4,
                owner_with_range(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/second",
                    source,
                    14,
                    source.len(),
                ),
            ),
        )
        .await
        .expect("publish resident generation");
    registry
        .lease("workspace-a", temporary.path())
        .expect("resident generation lease before overlay")
        .project(selector)
        .expect("admitted owner declares selector before overlay");
    registry
        .publish_selector_overlay(
            "workspace-a",
            temporary.path(),
            WorkspaceRuntimeSelectorOverlay {
                projection_kind:
                    agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                structural_selector: selector.to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
                byte_start: 14,
                byte_end: source.len(),
                projection_bytes: b"fn second() {}".to_vec(),
            },
        )
        .await
        .expect("publish selector overlay");

    let receipt = registry
        .ready_recovery_receipt("warm-ready", "workspace-a", temporary.path())
        .expect("resident ready receipt");
    assert_eq!(receipt.active_epoch, 3);
    assert_eq!(receipt.target_epoch, 4);
    assert!(receipt.old_generation_readable);
    assert_eq!(receipt.counters.database_opens, 0);
    assert_eq!(receipt.counters.provider_spawns, 0);
    assert!(matches!(
        registry
            .read_runtime_selector(
                "workspace-a",
                temporary.path(),
                agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                selector,
            )
            .expect("read selector after ready receipt"),
        WorkspaceRuntimeSelectorRead::Projection { .. }
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn runtime_shutdown_drains_every_workspace_writer_lane() {
    let temporary = tempdir().expect("temporary runtime root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().join("runtime")).expect("registry");
    registry
        .publish(
            "publish-before-shutdown",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-a",
                temporary.path(),
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
    let receipt = registry.shutdown().await.expect("drain writer lane");
    receipt.validate().expect("shutdown receipt");
    assert_eq!(receipt.writer_lane_count, 1);
    assert_eq!(receipt.forced_abort_count, 0);
    let error = registry
        .publish_owner_overlay(
            "publish-after-shutdown",
            "workspace-a",
            temporary.path(),
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

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_sessions_reuse_one_resident_search_generation_authority() {
    let temporary = tempdir().expect("temporary Runtime Server root");
    let project_root = temporary.path().join("resident-search-authority");
    tokio::fs::create_dir_all(&project_root)
        .await
        .expect("create search authority project root");
    let (endpoint, artifact_catalog) = fixture_endpoint(&temporary, 41).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind Runtime Server");
    let registry = Arc::clone(server.workspace_registry());
    registry
        .publish(
            "resident-search-authority",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-search-authority",
                &project_root,
                1,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/search_authority",
                    b"fn search_authority() {}",
                ),
            ),
        )
        .await
        .expect("publish resident search generation");
    let generation_pointer_path = workspace_generation_pointer_path(
        std::path::Path::new(&endpoint.workspace_store_path),
        "workspace-search-authority",
        &project_root,
    )
    .expect("resolve resident generation pointer");
    let compact_authority_path = generation_pointer_path
        .parent()
        .expect("resident generation directory")
        .join("search-authority-1.json");
    tokio::fs::remove_file(&compact_authority_path)
        .await
        .expect("remove compact authority to prove resident IPC ownership");
    let shutdown = server.shutdown_handle();
    let server = tokio::spawn(server.serve());

    let ready = Arc::new(tokio::sync::Barrier::new(64));
    let mut readers = Vec::with_capacity(64);
    for _ in 0..64 {
        let endpoint = endpoint.clone();
        let project_root = project_root.clone();
        let ready = Arc::clone(&ready);
        readers.push(tokio::spawn(async move {
            let mut cold_latencies = Vec::with_capacity(16);
            let mut session = None;
            for _ in 0..16 {
                let candidate = WorkspaceDbIpcSession::for_runtime_server(
                    &endpoint,
                    "workspace-search-authority".to_owned(),
                    project_root.clone(),
                );
                let cold_started = tokio::time::Instant::now();
                candidate
                    .runtime_search_generation_authority()
                    .await
                    .expect("read session-cold resident search generation authority");
                cold_latencies.push(cold_started.elapsed());
                session = Some(candidate);
            }
            let session = session.expect("fresh authority session");
            ready.wait().await;
            let mut latencies = Vec::with_capacity(32);
            for _ in 0..32 {
                let started = tokio::time::Instant::now();
                let authority = session
                    .runtime_search_generation_authority()
                    .await
                    .expect("read resident search generation authority");
                latencies.push(started.elapsed());
                assert_eq!(authority.workspace_identity, "workspace-search-authority");
                assert_eq!(authority.project_root, project_root.display().to_string());
                assert_eq!(
                    authority.source_snapshot.root_digest,
                    authority.workspace_generation.root_digest
                );
            }
            (cold_latencies, latencies)
        }));
    }
    let mut cold_latencies = Vec::with_capacity(64 * 16);
    let mut latencies = Vec::with_capacity(64 * 32);
    for reader in readers {
        let (reader_cold_latencies, warm_latencies) = reader.await.expect("join authority reader");
        cold_latencies.extend(reader_cold_latencies);
        latencies.extend(warm_latencies);
    }
    cold_latencies.sort_unstable();
    latencies.sort_unstable();
    let cold_p99 = cold_latencies[cold_latencies.len() * 99 / 100];
    let concurrent_p99 = latencies[latencies.len() * 99 / 100];
    eprintln!(
        "resident-search-authority-precheck coldP99Micros={} concurrentP99Micros={}",
        cold_p99.as_micros(),
        concurrent_p99.as_micros(),
    );
    assert!(
        cold_p99 < std::time::Duration::from_millis(10),
        "resident search generation authority cold IPC p99 exceeded 10ms: {cold_p99:?}"
    );
    assert!(
        concurrent_p99 < std::time::Duration::from_millis(1),
        "resident search generation authority concurrent p99 exceeded 1ms: {concurrent_p99:?}"
    );
    let warm_session = WorkspaceDbIpcSession::for_runtime_server(
        &endpoint,
        "workspace-search-authority".to_owned(),
        project_root.clone(),
    );
    let initial_authority = warm_session
        .runtime_search_generation_authority()
        .await
        .expect("prewarm sequential authority lane");
    assert_eq!(initial_authority.active_epoch, 1);
    registry
        .publish(
            "resident-search-authority-epoch-2",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                "workspace-search-authority",
                &project_root,
                2,
                owner(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/search_authority",
                    b"fn search_authority() { epoch_2() }",
                ),
            ),
        )
        .await
        .expect("publish next resident search generation");
    assert_eq!(
        warm_session
            .runtime_search_generation_authority()
            .await
            .expect("observe next generation through stable pointer")
            .active_epoch,
        2,
        "mapped authority pointer must observe publication without reopening IPC"
    );
    let hot_counter_baseline = registry.data_plane_counters();
    let mut service_latencies = Vec::with_capacity(1_000);
    for _ in 0..1_000 {
        let started = tokio::time::Instant::now();
        warm_session
            .runtime_search_generation_authority()
            .await
            .expect("read sequential resident search generation authority");
        service_latencies.push(started.elapsed());
    }
    service_latencies.sort_unstable();
    let service_p50 = service_latencies[service_latencies.len() * 50 / 100];
    let service_p95 = service_latencies[service_latencies.len() * 95 / 100];
    let service_p99 = service_latencies[service_latencies.len() * 99 / 100];
    let service_max = service_latencies[service_latencies.len() - 1];
    eprintln!(
        "resident-search-authority sessions=64 coldSamples={} requests={} coldP99Micros={} concurrentP99Micros={} serviceP50Micros={} serviceP95Micros={} serviceP99Micros={} serviceMaxMicros={} workspaceCount={} counters={:?}",
        cold_latencies.len(),
        latencies.len(),
        cold_p99.as_micros(),
        concurrent_p99.as_micros(),
        service_p50.as_micros(),
        service_p95.as_micros(),
        service_p99.as_micros(),
        service_max.as_micros(),
        registry.workspace_count(),
        registry.data_plane_counters(),
    );
    assert!(
        service_p99 < std::time::Duration::from_millis(1),
        "resident search generation authority service p99 exceeded 1ms: {service_p99:?}"
    );
    registry
        .data_plane_counters()
        .delta_since(&hot_counter_baseline)
        .validate_zero_io()
        .expect("warm authority pointer reads must perform zero runtime data-plane I/O");
    assert_eq!(registry.workspace_count(), 1);

    shutdown.shutdown();
    assert_eq!(
        server.await.expect("join Runtime Server").expect("serve"),
        RuntimeServerExit::ShutdownRequested
    );
}
