use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceExactProjectionDataPlaneClient,
    WorkspaceExactProjectionDataPlaneOpen, WorkspaceGenerationDataPlaneClient,
    WorkspaceGenerationDataPlaneOpen, WorkspaceGenerationPointerReader, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceRecoverySource,
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
                owner_path: "src/lib.rs".to_owned(),
                content_digest,
                bytes: bytes.to_vec(),
                selectors: Vec::new(),
            }],
        },
    )
    .expect("typed resident recovery generation")
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
    let counters = registry.data_plane_counters();
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);
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
    bytes[..16].copy_from_slice(b"ASPEXACTMMAP0001");
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
    registry.shutdown().await.expect("drain writer lane");
}
