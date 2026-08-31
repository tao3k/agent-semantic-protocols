use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceCanonicalMaterialization,
    WorkspaceExactProjectionDataPlaneClient, WorkspaceExactProjectionDataPlaneOpen,
    WorkspaceGenerationDataPlaneClient, WorkspaceGenerationDataPlaneOpen,
    WorkspaceGenerationPointerReader, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceRecoverySource,
};

fn project_root(workspace_identity: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("/runtime-server-recovery-fixture").join(workspace_identity)
}

fn generation(workspace_identity: &str) -> WorkspaceMemoryGeneration {
    let bytes = b"pub fn recovered() {}\n";
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"resident-recovery-fixture-provider").to_hex()
        ),
    );
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
    projection_capability: agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionCapabilityManifest::single_selector("blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(), "rust://fixture/src/lib.rs#item/function/fixture".to_owned(), "src/lib.rs".to_owned(), std::collections::BTreeSet::from([agent_semantic_client_db::active_generation_projection_capability::ActiveGenerationProjectionMode::Source])).expect("test projection capability manifest"),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root(workspace_identity).display().to_string(),
            active_epoch: 1,
            workspace_snapshot,
            source_snapshot,
            module_graph_digest: format!(
                "blake3-256:{}",
                blake3::hash(b"resident-recovery-fixture-module-graph").to_hex()
            ),
            project_resolutions: Vec::new(),
            owners: vec![WorkspaceOwnerSnapshot {
                                authority: None,
                owner_path: "src/lib.rs".to_owned(),
                content_digest,
                bytes: bytes.to_vec(),
                selectors: Vec::new(),
            }],
        },
    )
    .expect("typed resident recovery generation")
}

fn canonical_materialization(
    workspace_identity: &str,
    project_root: &std::path::Path,
) -> WorkspaceCanonicalMaterialization {
    let bytes = b"pub fn recovered() {}\n";
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_bytes([(
        "src/lib.rs",
        bytes.as_slice(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"resident-recovery-fixture-provider").to_hex()
        ),
    );
    let import = agent_semantic_client_db::ClientDbSourceIndexImport {
        source_blobs: Default::default(),
        relations: Vec::new(),
        generation_id: agent_semantic_client_core::CacheGenerationId::from(
            "resident-recovery-fixture",
        ),
        project_root: project_root.to_path_buf(),
        schema_id: agent_semantic_client_core::SemanticSchemaId::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
        ),
        schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
        ),
        file_hashes: vec![agent_semantic_client_core::ClientCacheFileHash {
            path: "src/lib.rs".to_owned(),
            sha256: "0".repeat(64),
            byte_len: bytes.len() as u64,
            mtime_ms: 1,
        }],
        owners: Vec::new(),
        selectors: Vec::new(),
    };
    WorkspaceCanonicalMaterialization::new(
        workspace_identity,
        source_snapshot,
        &import,
        [1, 0],
        vec![WorkspaceOwnerSnapshot {
            authority: None,
            owner_path: "src/lib.rs".to_owned(),
            content_digest,
            bytes: bytes.to_vec(),
            selectors: Vec::new(),
        }],
        Vec::new(),
    )
    .expect("canonical recovery materialization")
}

#[tokio::test]
async fn absent_generation_pointer_is_a_typed_missing_state() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let pointer = temporary.path().join("active-generation.pointer");

    let state = WorkspaceGenerationDataPlaneClient::open_state(&pointer)
        .await
        .expect("inspect absent generation pointer");
    assert!(matches!(state, WorkspaceGenerationDataPlaneOpen::Missing));
}

#[tokio::test]
async fn corrupt_generation_pointer_requires_resident_recovery() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let pointer = temporary.path().join("active-generation.pointer");
    tokio::fs::write(&pointer, b"not-a-generation-pointer")
        .await
        .expect("write corrupt generation pointer fixture");

    let state = WorkspaceGenerationDataPlaneClient::open_state(&pointer)
        .await
        .expect("classify corrupt generation pointer");
    let WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason } = state else {
        panic!("corrupt generation pointer must require resident recovery");
    };
    assert!(!reason.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resident_writer_replaces_a_corrupt_pointer_with_one_complete_generation() {
    let _performance = crate::test_support::performance_lock();
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let workspace_identity = "workspace-recovery";
    let generations = temporary
        .path()
        .join(workspace_identity)
        .join("scopes")
        .join(format!(
            "scope-{}",
            &blake3::hash(
                project_root(workspace_identity)
                    .to_string_lossy()
                    .as_bytes()
            )
            .to_hex()[..16]
        ))
        .join("generations");
    tokio::fs::create_dir_all(&generations)
        .await
        .expect("create generation directory");
    let pointer = generations.join("active-generation.pointer");
    tokio::fs::write(&pointer, b"stale-generation-pointer")
        .await
        .expect("write stale generation pointer");

    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let receipt = registry
        .publish(
            "recover-stale-generation",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity),
        )
        .await
        .expect("resident writer should replace stale generation");
    assert_eq!(receipt.target_epoch, 1);
    let expected_generation_digest = receipt.generation_digest.clone();

    let state = WorkspaceGenerationDataPlaneClient::open_state(&pointer)
        .await
        .expect("open recovered generation");
    let WorkspaceGenerationDataPlaneOpen::Ready(client) = state else {
        panic!("resident writer must publish one complete readable generation");
    };
    assert_eq!(client.lease().epoch(), 1);
    assert_eq!(
        client
            .lease()
            .owner("src/lib.rs")
            .expect("recovered owner")
            .as_ref(),
        b"pub fn recovered() {}\n"
    );
    let exact_open_started = std::time::Instant::now();
    let exact_state = WorkspaceExactProjectionDataPlaneClient::open_state(&pointer)
        .await
        .expect("open recovered exact generation");
    let WorkspaceExactProjectionDataPlaneOpen::Ready(exact_client) = exact_state else {
        panic!("resident writer must publish one compact exact generation");
    };
    let owner = exact_client
        .owner_snapshot("src/lib.rs")
        .expect("lookup compact owner")
        .expect("compact owner is present");
    assert_eq!(owner.bytes, b"pub fn recovered() {}\n");
    assert!(!exact_client.generation_digest().is_empty());
    assert!(!exact_client.root_digest().is_empty());
    let mut exact_open_samples = vec![exact_open_started.elapsed()];
    for _ in 1..30 {
        let started = std::time::Instant::now();
        let state = WorkspaceExactProjectionDataPlaneClient::open_state(&pointer)
            .await
            .expect("reopen recovered exact generation");
        let WorkspaceExactProjectionDataPlaneOpen::Ready(client) = state else {
            panic!("recovered compact generation must remain readable");
        };
        assert!(
            client
                .owner_snapshot("src/lib.rs")
                .expect("lookup reopened compact owner")
                .is_some()
        );
        exact_open_samples.push(started.elapsed());
    }
    exact_open_samples.sort_unstable();
    let p75 = exact_open_samples[(exact_open_samples.len() * 75).div_ceil(100) - 1];
    let max = *exact_open_samples.last().expect("exact open samples");
    assert!(
        p75 <= std::time::Duration::from_millis(5),
        "compact owner open and lookup p75 exceeded 5ms: {p75:?}"
    );
    assert!(
        max <= std::time::Duration::from_millis(50),
        "compact owner open and lookup maximum exceeded 50ms: {max:?}"
    );
    let counters = registry.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);
    registry.shutdown().await.expect("drain writer lane");
    drop(registry);

    let restarted =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let restored = restarted
        .restore_published_generation(
            "restore-without-turso-materialization",
            workspace_identity.to_owned(),
            &project_root(workspace_identity),
        )
        .await
        .expect("active generation pointer and checkpoint are the restore authority");
    assert_eq!(restored.source, WorkspaceRecoverySource::MmapCheckpoint);
    assert_eq!(restored.target_epoch, 1);
    assert_eq!(restored.generation_digest, expected_generation_digest);
    let counters = restarted.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);
    restarted
        .shutdown()
        .await
        .expect("drain restored writer lane");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_sessions_share_one_load_once_generation_backend() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let workspace_identity = "workspace-load-once";
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    registry
        .publish(
            "publish-load-once-generation",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity),
        )
        .await
        .expect("publish load-once generation");
    let pointer =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            temporary.path(),
            workspace_identity,
            &project_root(workspace_identity),
        )
        .expect("workspace generation pointer path");

    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(65));
    let mut sessions = tokio::task::JoinSet::new();
    for _ in 0..64 {
        let pointer = pointer.clone();
        let barrier = std::sync::Arc::clone(&barrier);
        sessions.spawn(async move {
            barrier.wait().await;
            let WorkspaceGenerationDataPlaneOpen::Ready(client) =
                WorkspaceGenerationDataPlaneClient::open_state(&pointer)
                    .await
                    .expect("open shared generation")
            else {
                panic!("published generation must be ready");
            };
            assert_eq!(client.lease().epoch(), 1);
        });
    }
    barrier.wait().await;
    while let Some(session) = sessions.join_next().await {
        session.expect("load-once session");
    }

    let mut warm_samples = Vec::with_capacity(2_048);
    for _ in 0..2_048 {
        let started = std::time::Instant::now();
        let state = WorkspaceGenerationDataPlaneClient::open_state(&pointer)
            .await
            .expect("warm generation open");
        assert!(matches!(state, WorkspaceGenerationDataPlaneOpen::Ready(_)));
        warm_samples.push(started.elapsed());
    }
    warm_samples.sort_unstable();
    let warm_p99 = warm_samples[warm_samples.len() * 99 / 100];
    let receipt = WorkspaceGenerationDataPlaneClient::cache_receipt(&pointer);
    assert_eq!(receipt.cold_open_count, 1, "receipt={receipt:?}");
    assert_eq!(receipt.refresh_open_count, 0, "receipt={receipt:?}");
    assert!(receipt.warm_hit_count >= 2_048, "receipt={receipt:?}");
    assert!(
        warm_p99 < std::time::Duration::from_millis(1),
        "load-once generation warm p99 exceeded 1ms: {warm_p99:?} receipt={receipt:?}"
    );
    let counters = registry.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);
    println!(
        "[workspace-generation-load-once] sessions=64 warmSamples={} coldOpenCount={} coalescedOpenCount={} warmHitCount={} refreshOpenCount={} databaseOpenCount=0 providerProcessCount=0 controlRoundtripCount=0 p99Nanos={} budgetNanos=1000000",
        warm_samples.len(),
        receipt.cold_open_count,
        receipt.coalesced_open_count,
        receipt.warm_hit_count,
        receipt.refresh_open_count,
        warm_p99.as_nanos(),
    );
    registry.shutdown().await.expect("drain writer lane");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn obsolete_exact_segment_format_requires_typed_rebuild() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let workspace_identity = "workspace-obsolete-exact-format";
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    registry
        .publish(
            "publish-current-exact-format",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity),
        )
        .await
        .expect("publish current exact projection segment");
    let pointer =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            temporary.path(),
            workspace_identity,
            &project_root(workspace_identity),
        )
        .expect("workspace generation pointer path");
    let pointer_reader = WorkspaceGenerationPointerReader::open(&pointer)
        .await
        .expect("open generation pointer");
    let snapshot = pointer_reader.read().expect("read generation pointer");
    let exact_path = std::path::Path::new(&snapshot.mmap_segment_path).with_extension("exact.mmap");
    let mut bytes = tokio::fs::read(&exact_path)
        .await
        .expect("read exact segment");
    bytes[..16].copy_from_slice(b"ASPEXACTMMAPV1__");
    tokio::fs::write(&exact_path, bytes)
        .await
        .expect("write obsolete exact segment identity");

    let state = WorkspaceExactProjectionDataPlaneClient::open_state(&pointer)
        .await
        .expect("classify obsolete exact segment");
    let WorkspaceExactProjectionDataPlaneOpen::RecoveryRequired { reason } = state else {
        panic!("obsolete exact segment format must require a typed rebuild");
    };
    assert!(reason.contains("header is invalid"), "reason={reason}");
    let published = registry
        .published_generation_state(workspace_identity, &project_root(workspace_identity))
        .await
        .expect("classify complete published generation");
    let agent_semantic_client_db::runtime_server_workspace::PublishedWorkspaceGenerationState::RecoveryRequired {
        reason,
    } = published
    else {
        panic!("obsolete exact segment must make the complete generation recoverable");
    };
    assert!(reason.contains("header is invalid"), "reason={reason}");
    registry.shutdown().await.expect("drain writer lane");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn canonical_ready_reuse_republishes_an_obsolete_exact_layout() {
    let temporary = tempfile::tempdir().expect("temporary runtime root");
    let workspace_identity = "workspace-obsolete-exact-republication";
    let project_root = temporary.path().join("project");
    std::fs::create_dir_all(&project_root).expect("create canonical project root");
    let canonical_project_root =
        std::fs::canonicalize(project_root).expect("canonicalize project root");
    let registry =
        RuntimeServerWorkspaceRegistry::new(temporary.path().to_path_buf()).expect("registry");
    let materialization = canonical_materialization(workspace_identity, &canonical_project_root);
    registry
        .ensure_canonical_generation(
            "publish-current-layout",
            workspace_identity,
            materialization
                .clone()
                .into_validated(workspace_identity)
                .expect("validate initial materialization"),
        )
        .await
        .expect("publish initial generation");
    let pointer =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            temporary.path(),
            workspace_identity,
            &canonical_project_root,
        )
        .expect("workspace generation pointer path");
    let snapshot = WorkspaceGenerationPointerReader::open(&pointer)
        .await
        .expect("open generation pointer")
        .read()
        .expect("read generation pointer");
    let exact_path = std::path::Path::new(&snapshot.mmap_segment_path).with_extension("exact.mmap");
    let mut bytes = tokio::fs::read(&exact_path)
        .await
        .expect("read exact segment");
    bytes[..16].copy_from_slice(b"ASPEXACTMMAPV1__");
    tokio::fs::write(&exact_path, bytes)
        .await
        .expect("write obsolete exact segment identity");
    registry
        .ensure_canonical_generation(
            "republish-obsolete-layout",
            workspace_identity,
            materialization
                .into_validated(workspace_identity)
                .expect("validate replay materialization"),
        )
        .await
        .expect("Runtime republishes obsolete exact layout");
    let snapshot = WorkspaceGenerationPointerReader::open(&pointer)
        .await
        .expect("reopen generation pointer")
        .read()
        .expect("read republished generation pointer");
    let exact_path = std::path::Path::new(&snapshot.mmap_segment_path).with_extension("exact.mmap");
    let bytes = tokio::fs::read(exact_path)
        .await
        .expect("read republished exact segment");
    assert_eq!(&bytes[..16], b"ASPEXACTMMAP0004");
    assert!(matches!(
        registry
            .published_generation_state(workspace_identity, &canonical_project_root)
            .await
            .expect("complete published generation state"),
        agent_semantic_client_db::runtime_server_workspace::PublishedWorkspaceGenerationState::Ready
    ));
    registry.shutdown().await.expect("drain writer lane");
}
