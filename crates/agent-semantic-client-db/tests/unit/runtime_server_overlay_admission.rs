use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WorkspaceCanonicalMaterialization, WorkspaceGenerationState,
    WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot, WorkspaceRecoverySource,
};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn fixture_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "asp-runtime-server-overlay-admission-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn generation(workspace_identity: &str, epoch: u64, bytes: &[u8]) -> WorkspaceMemoryGeneration {
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "runtime-overlay-fixture".to_owned(),
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
        blake3::hash(format!("{workspace_identity}\0{epoch}\0{content_digest}").as_bytes())
            .to_hex()
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
        owners: vec![WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            content_digest,
            bytes: bytes.to_vec(),
            selectors: Vec::new(),
        }],
    }
}

#[test]
fn canonical_materialization_binds_snapshot_import_and_complete_owner_count() {
    let import = agent_semantic_client_db::ClientDbSourceIndexImport {
        generation_id: agent_semantic_client_core::CacheGenerationId::from(
            "canonical-materialization-generation",
        ),
        project_root: std::path::PathBuf::from("/canonical-materialization-fixture"),
        schema_id: agent_semantic_client_core::SemanticSchemaId::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
        ),
        schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
        ),
        file_hashes: vec![
            agent_semantic_client_core::ClientCacheFileHash {
                path: "src/lib.rs".to_owned(),
                sha256: "a".repeat(64),
                byte_len: 6,
                mtime_ms: 1,
            },
            agent_semantic_client_core::ClientCacheFileHash {
                path: "Cargo.toml".to_owned(),
                sha256: "b".repeat(64),
                byte_len: 0,
                mtime_ms: 1,
            },
        ],
        owners: vec![agent_semantic_client_db::ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: Some("rust".into()),
            provider_id: Some("rs-harness".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: Vec::new(),
        }],
        selectors: Vec::new(),
    };
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        import
            .file_hashes
            .iter()
            .map(|file| (file.path.clone(), file.sha256.clone())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "provider-digest".to_owned(),
    );
    let materialization = WorkspaceCanonicalMaterialization::new(
        "workspace-canonical-materialization",
        source_snapshot.clone(),
        &import,
        [1, 0],
        vec![WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            content_digest: format!("blake3-256:{}", blake3::hash(b"source").to_hex()),
            bytes: b"source".to_vec(),
            selectors: Vec::new(),
        }],
    )
    .expect("build complete materialization");

    materialization
        .validate_against(
            "workspace-canonical-materialization",
            &source_snapshot,
            &import,
            1,
        )
        .expect("validate matching durable request");
    materialization
        .validate_refresh_request(
            "workspace-canonical-materialization",
            &agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
                import: import.clone(),
                file_count: 9,
                source_snapshot: source_snapshot.clone(),
            },
        )
        .expect("file leaf count may differ from complete owner count");
    let mut drifted_snapshot = source_snapshot.clone();
    drifted_snapshot.root_digest = "c".repeat(64);
    assert!(
        materialization
            .validate_against(
                "workspace-canonical-materialization",
                &drifted_snapshot,
                &import,
                1,
            )
            .expect_err("snapshot drift must fail")
            .contains("source snapshot drift")
    );
    assert!(
        materialization
            .validate_against(
                "workspace-canonical-materialization",
                &source_snapshot,
                &import,
                2,
            )
            .expect_err("partial owner materialization must fail")
            .contains("is incomplete")
    );
    let mut drifted_owner = materialization.clone();
    drifted_owner.owners[0].bytes = b"different-source".to_vec();
    assert!(
        drifted_owner
            .validate_against(
                "workspace-canonical-materialization",
                &source_snapshot,
                &import,
                1,
            )
            .expect_err("owner byte drift must fail")
            .contains("owner digest drift")
    );

    let generation = materialization
        .into_generation(4)
        .expect("build next immutable generation");
    assert_eq!(generation.active_epoch, 5);
    assert_eq!(generation.root_depth, [1, 0]);
    assert_eq!(generation.owners.len(), 1);
    assert_eq!(
        generation.workspace_snapshot.root_digest(),
        generation.source_snapshot.root_digest
    );
    assert_eq!(
        generation.workspace_generation.root_digest,
        generation.source_snapshot.root_digest
    );
    assert_eq!(generation.workspace_generation.leaf_count, 2);
    assert_eq!(generation.workspace_generation.owner_count, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_overlay_cannot_manufacture_a_canonical_generation() {
    let root = fixture_root();
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");

    let error = registry
        .publish_owner_overlay(
            "overlay-before-canonical-generation",
            "workspace-overlay-admission",
            WorkspaceOwnerSnapshot {
                owner_path: "src/live.rs".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(b"live").to_hex()),
                bytes: b"live".to_vec(),
                selectors: Vec::new(),
            },
        )
        .await
        .expect_err("an owner overlay must not manufacture a complete generation");

    assert!(
        error.contains("requires an admitted canonical generation"),
        "unexpected admission error: {error}"
    );
    assert!(
        registry.lease("workspace-overlay-admission").is_err(),
        "failed overlay admission must not publish a readable generation"
    );

    let shutdown = registry.shutdown().await.expect("drain writer lane");
    assert_eq!(shutdown.workspace_count, 1);
    assert_eq!(shutdown.writer_lane_count, 1);
    assert!(shutdown.queued_publications_drained);
    assert_eq!(shutdown.forced_abort_count, 0);
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn moved_owner_overlay_publishes_one_atomic_relocation_epoch() {
    let root = fixture_root();
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let workspace_identity = "workspace-owner-move";
    registry
        .publish(
            "publish-canonical-before-move",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, 1, b"fn moved() {}\n"),
        )
        .await
        .expect("publish canonical generation");
    let moved_bytes = b"fn moved() { println!(\"moved\"); }\n";
    registry
        .relocate_owner_overlay(
            "relocate-owner-atomically",
            workspace_identity,
            "src/lib.rs",
            WorkspaceOwnerSnapshot {
                owner_path: "src/moved.rs".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(moved_bytes).to_hex()),
                bytes: moved_bytes.to_vec(),
                selectors: Vec::new(),
            },
        )
        .await
        .expect("relocate owner in one writer epoch");

    let lease = registry
        .lease(workspace_identity)
        .expect("lease moved generation");
    assert_eq!(lease.epoch(), 2);
    assert!(lease.owner("src/lib.rs").is_none());
    assert_eq!(
        lease.owner("src/moved.rs").as_deref(),
        Some(moved_bytes.as_slice())
    );
    assert_eq!(lease.generation().root_depth, [1, 0]);
    assert_eq!(lease.generation().source_snapshot.leaf_count, 1);
    assert_eq!(lease.generation().workspace_generation.owner_count, 1);
    assert_eq!(
        lease.generation().workspace_generation.root_digest,
        lease.generation().source_snapshot.root_digest
    );

    registry
        .shutdown()
        .await
        .expect("drain moved owner writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_cold_restore_publishes_one_canonical_epoch() {
    let root = fixture_root();
    let registry = std::sync::Arc::new(
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry"),
    );
    let workspace_identity = "workspace-concurrent-cold-restore";
    let source = b"pub fn restored() {}\n";
    let import = agent_semantic_client_db::ClientDbSourceIndexImport {
        generation_id: agent_semantic_client_core::CacheGenerationId::from(
            "concurrent-cold-restore",
        ),
        project_root: std::path::PathBuf::from("/concurrent-cold-restore"),
        schema_id: agent_semantic_client_core::SemanticSchemaId::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
        ),
        schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
        ),
        file_hashes: vec![agent_semantic_client_core::ClientCacheFileHash {
            path: "src/lib.rs".to_owned(),
            sha256: "c".repeat(64),
            byte_len: source.len() as u64,
            mtime_ms: 1,
        }],
        owners: vec![agent_semantic_client_db::ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: Some("rust".into()),
            provider_id: Some("rs-harness".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: Vec::new(),
        }],
        selectors: Vec::new(),
    };
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        import
            .file_hashes
            .iter()
            .map(|file| (file.path.clone(), file.sha256.clone())),
    )
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "concurrent-cold-restore-provider".to_owned(),
    );
    let materialization = WorkspaceCanonicalMaterialization::new(
        workspace_identity,
        source_snapshot,
        &import,
        [1, 0],
        vec![WorkspaceOwnerSnapshot {
            owner_path: "src/lib.rs".to_owned(),
            content_digest: format!("blake3-256:{}", blake3::hash(source).to_hex()),
            bytes: source.to_vec(),
            selectors: vec![],
        }],
    )
    .expect("build canonical cold-restore materialization");
    let mut restores = tokio::task::JoinSet::new();
    for request in 0..64 {
        let registry = std::sync::Arc::clone(&registry);
        let materialization = materialization.clone();
        restores.spawn(async move {
            registry
                .ensure_canonical_generation(
                    format!("cold-restore-{request}"),
                    workspace_identity,
                    materialization,
                )
                .await
        });
    }
    let mut receipts = Vec::new();
    while let Some(receipt) = restores.join_next().await {
        receipts.push(
            receipt
                .expect("join cold restore")
                .expect("ensure canonical generation"),
        );
    }
    assert_eq!(receipts.len(), 64);
    assert!(
        receipts.iter().all(|receipt| receipt.target_epoch == 1),
        "concurrent cold restore must reuse the first published epoch: {receipts:?}"
    );
    let lease = registry
        .lease(workspace_identity)
        .expect("lease restored canonical generation");
    assert_eq!(lease.epoch(), 1);
    assert_eq!(lease.generation().root_depth, [1, 0]);
    assert_eq!(
        lease.generation().workspace_snapshot.root_digest(),
        lease.generation().source_snapshot.root_digest
    );
    assert_eq!(
        lease.generation().workspace_generation.root_digest,
        lease.generation().source_snapshot.root_digest
    );
    assert_eq!(
        lease.owner("src/lib.rs").expect("restored owner").as_ref(),
        source
    );
    let counters = registry.data_plane_counters();
    assert_eq!(counters.filesystem_reads, 1);
    assert_eq!(counters.filesystem_writes, 1);
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn atomic_epoch_switch_keeps_old_reader_leases_on_the_old_generation() {
    let root = fixture_root();
    let registry = std::sync::Arc::new(
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry"),
    );
    let workspace_identity = "workspace-epoch-switch";
    registry
        .publish(
            "publish-epoch-1",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, 1, b"epoch-one"),
        )
        .await
        .expect("publish initial canonical generation");
    let old_lease = registry
        .lease(workspace_identity)
        .expect("lease initial generation");
    let old_root = old_lease.generation().source_snapshot.root_digest.clone();
    let start = std::sync::Arc::new(tokio::sync::Barrier::new(65));
    let mut readers = tokio::task::JoinSet::new();
    for _ in 0..64 {
        let lease = old_lease.clone();
        let start = std::sync::Arc::clone(&start);
        readers.spawn(async move {
            start.wait().await;
            assert_eq!(lease.epoch(), 1);
            assert_eq!(
                lease.owner("src/lib.rs").expect("old owner bytes").as_ref(),
                b"epoch-one"
            );
        });
    }
    start.wait().await;

    let receipt = registry
        .publish(
            "publish-epoch-2",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, 2, b"epoch-two"),
        )
        .await
        .expect("atomically publish next canonical generation");
    assert_eq!(receipt.active_epoch, 1);
    assert_eq!(receipt.target_epoch, 2);
    assert!(receipt.old_generation_readable);
    while let Some(result) = readers.join_next().await {
        result.expect("old reader task");
    }

    let current = registry
        .lease(workspace_identity)
        .expect("lease current generation");
    assert_eq!(current.epoch(), 2);
    assert_ne!(current.generation().source_snapshot.root_digest, old_root);
    assert_eq!(
        current
            .owner("src/lib.rs")
            .expect("current owner bytes")
            .as_ref(),
        b"epoch-two"
    );
    assert_eq!(old_lease.epoch(), 1);
    assert_eq!(old_lease.generation().source_snapshot.root_digest, old_root);
    assert_eq!(
        old_lease
            .owner("src/lib.rs")
            .expect("retained old owner bytes")
            .as_ref(),
        b"epoch-one"
    );

    registry.shutdown().await.expect("drain writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restarted_registry_restores_mmap_before_admitting_owner_overlay() {
    let root = fixture_root();
    let workspace_identity = "workspace-mmap-restore";
    let first =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create first workspace registry");
    first
        .publish(
            "publish-before-restart",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, 1, b"canonical"),
        )
        .await
        .expect("publish canonical generation");
    first.shutdown().await.expect("drain first writer lane");
    drop(first);

    let restarted = RuntimeServerWorkspaceRegistry::new(root.clone())
        .expect("create restarted workspace registry");
    let receipt = restarted
        .publish_owner_overlay(
            "overlay-after-restart",
            workspace_identity,
            WorkspaceOwnerSnapshot {
                owner_path: "src/lib.rs".to_owned(),
                content_digest: format!("blake3-256:{}", blake3::hash(b"live").to_hex()),
                bytes: b"live".to_vec(),
                selectors: Vec::new(),
            },
        )
        .await
        .expect("restore mmap generation before applying overlay");
    assert_eq!(
        receipt.source,
        WorkspaceRecoverySource::ProviderOwnerOverlay
    );
    assert_eq!(receipt.active_epoch, 1);
    assert_eq!(receipt.target_epoch, 2);
    assert!(receipt.old_generation_readable);
    let current = restarted
        .lease(workspace_identity)
        .expect("lease restored and overlaid generation");
    assert_eq!(current.epoch(), 2);
    assert_eq!(
        current
            .owner("src/lib.rs")
            .expect("overlaid owner bytes")
            .as_ref(),
        b"live"
    );

    restarted
        .shutdown()
        .await
        .expect("drain restarted writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}
