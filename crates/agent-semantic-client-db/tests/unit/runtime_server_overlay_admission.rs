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

fn generation(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    bytes: &[u8],
) -> WorkspaceMemoryGeneration {
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
        project_root: project_root.display().to_string(),
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
    use sha2::Digest as _;

    let project_root = fixture_root();
    std::fs::create_dir_all(&project_root).expect("create canonical materialization project root");
    let import = agent_semantic_client_db::build_source_index_import(
        agent_semantic_client_db::ClientDbSourceIndexImportRequest {
            generation_id: agent_semantic_client_core::CacheGenerationId::from(
                "canonical-materialization-generation",
            ),
            project_root: project_root.clone(),
            schema_id: agent_semantic_client_core::SemanticSchemaId::from(
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
            ),
            schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
            ),
            selector_source: agent_semantic_client_db::ClientDbSourceIndexSource::from(
                agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID,
            ),
            file_hashes: vec![agent_semantic_client_core::ClientCacheFileHash {
                path: "src/lib.rs".to_owned(),
                sha256: format!("{:x}", sha2::Sha256::digest(b"source")),
                byte_len: 6,
                mtime_ms: 1,
            }],
            files: vec![agent_semantic_client_db::ClientDbSourceIndexImportFile {
                relative_path: "src/lib.rs".to_owned(),
                language_id: "rust".into(),
                provider_id: "rs-harness".into(),
                text: "source".to_owned(),
                selectors: Vec::new(),
            }],
        },
    )
    .expect("build canonical materialization source-index import");
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
        Vec::new(),
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
    assert_eq!(generation.workspace_generation.leaf_count, 1);
    assert_eq!(generation.workspace_generation.owner_count, 1);
    let _ = std::fs::remove_dir_all(project_root);
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
            &root,
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
        registry
            .lease("workspace-overlay-admission", &root)
            .is_err(),
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
async fn stale_owner_is_reconciled_before_a_resident_projection_can_serve() {
    let root = fixture_root();
    tokio::fs::create_dir_all(root.join("src"))
        .await
        .expect("create source root");
    let root = tokio::fs::canonicalize(root)
        .await
        .expect("canonical source root");
    let current_bytes = b"fn current() {}\n";
    tokio::fs::write(root.join("src/lib.rs"), current_bytes)
        .await
        .expect("write current owner");
    let resolved = agent_semantic_client_core::state_core::ResolvedState::resolve(&root)
        .expect("resolve root");
    let workspace_identity = resolved.workspace.workspace_id.to_string();
    let registry = std::sync::Arc::new(
        RuntimeServerWorkspaceRegistry::new(root.join("runtime"))
            .expect("create workspace registry"),
    );
    let stale_selector = "rust://src/lib.rs#item/function/stale";
    let current_selector = "rust://src/lib.rs#item/function/current";
    let mut stale_generation = generation(&workspace_identity, &root, 1, b"fn stale() {}\n");
    stale_generation.owners[0].selectors = vec![
        agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
            selector: stale_selector.to_owned(),
            byte_start: 0,
            byte_end: b"fn stale() {}\n".len(),
            derived_projections: Vec::new(),
        },
    ];
    registry
        .publish(
            "publish-stale-generation",
            WorkspaceRecoverySource::TursoGeneration,
            stale_generation,
        )
        .await
        .expect("publish stale canonical generation");

    let build_started = std::sync::Arc::new(tokio::sync::Notify::new());
    let release_build = std::sync::Arc::new(tokio::sync::Notify::new());
    let build_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let builder: agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerProjectionBuilder = {
        let build_started = std::sync::Arc::clone(&build_started);
        let release_build = std::sync::Arc::clone(&release_build);
        let build_count = std::sync::Arc::clone(&build_count);
        std::sync::Arc::new(move |_language_id, _project_root, mut owner| {
            let build_started = std::sync::Arc::clone(&build_started);
            let release_build = std::sync::Arc::clone(&release_build);
            let build_count = std::sync::Arc::clone(&build_count);
            Box::pin(async move {
                build_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                build_started.notify_one();
                release_build.notified().await;
                owner.selectors = vec![
                    agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                        selector: current_selector.to_owned(),
                        byte_start: 0,
                        byte_end: owner.bytes.len(),
                        derived_projections: Vec::new(),
                    },
                ];
                Ok(owner)
            })
        })
    };
    let refresh_registry = std::sync::Arc::clone(&registry);
    let refresh_workspace_identity = workspace_identity.clone();
    let refresh_root = root.clone();
    let refresh_builder = std::sync::Arc::clone(&builder);
    let refresh = tokio::spawn(async move {
        refresh_registry
            .ensure_runtime_owner_freshness(
                "refresh-stale-owner",
                &refresh_workspace_identity,
                &refresh_root,
                "rust",
                "src/lib.rs",
                Some(&refresh_builder),
            )
            .await
    });
    build_started.notified().await;

    for _ in 0..128 {
        let lease = registry
            .lease(&workspace_identity, &root)
            .expect("old generation remains readable while projection builds");
        assert_eq!(
            lease.owner("src/lib.rs").as_deref(),
            Some(b"fn stale() {}\n".as_slice())
        );
        let selectors = &lease.generation().owners[0].selectors;
        assert!(
            selectors
                .iter()
                .any(|entry| entry.selector == stale_selector)
        );
        assert!(
            !selectors
                .iter()
                .any(|entry| entry.selector == current_selector)
        );
    }
    let concurrent_registry = std::sync::Arc::clone(&registry);
    let concurrent_workspace_identity = workspace_identity.clone();
    let concurrent_root = root.clone();
    let concurrent_builder = std::sync::Arc::clone(&builder);
    let concurrent = tokio::spawn(async move {
        concurrent_registry
            .ensure_runtime_owner_freshness(
                "concurrent-refresh-same-owner",
                &concurrent_workspace_identity,
                &concurrent_root,
                "rust",
                "src/lib.rs",
                Some(&concurrent_builder),
            )
            .await
    });
    release_build.notify_waiters();
    let refreshed = refresh
        .await
        .expect("join primary refresh")
        .expect("refresh stale owner");
    let concurrent = concurrent
        .await
        .expect("join concurrent refresh")
        .expect("reuse atomically published owner");
    assert!(refreshed.changed);
    assert!(!refreshed.removed);
    assert!(!concurrent.changed);
    assert_eq!(build_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    let (_, owner) = registry
        .runtime_owner_snapshot(&workspace_identity, &root, "src/lib.rs")
        .expect("read refreshed owner");
    assert_eq!(owner.bytes, current_bytes);
    assert_eq!(owner.selectors.len(), 1);
    assert_eq!(owner.selectors[0].selector, current_selector);
    assert_eq!(owner.selectors[0].byte_end, current_bytes.len());
    let lease = registry
        .lease(&workspace_identity, &root)
        .expect("lease refreshed generation");
    let selectors = &lease.generation().owners[0].selectors;
    assert!(
        !selectors
            .iter()
            .any(|entry| entry.selector == stale_selector)
    );
    assert!(
        selectors
            .iter()
            .any(|entry| entry.selector == current_selector)
    );

    let warm = registry
        .ensure_runtime_owner_freshness(
            "verify-current-owner",
            &workspace_identity,
            &root,
            "rust",
            "src/lib.rs",
            Some(&builder),
        )
        .await
        .expect("verify current owner");
    assert!(!warm.changed);
    assert!(!warm.removed);
    assert_eq!(warm.generation_digest, refreshed.generation_digest);
    assert!(
        registry
            .ensure_runtime_owner_freshness(
                "reject-owner-escape",
                &workspace_identity,
                &root,
                "rust",
                "../outside.rs",
                Some(&builder),
            )
            .await
            .expect_err("escaping owner path must fail")
            .contains("normalized relative owner path")
    );

    registry.shutdown().await.expect("shutdown registry");
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
            generation(workspace_identity, &root, 1, b"fn moved() {}\n"),
        )
        .await
        .expect("publish canonical generation");
    let moved_bytes = b"fn moved() { println!(\"moved\"); }\n";
    registry
        .relocate_owner_overlay(
            "relocate-owner-atomically",
            workspace_identity,
            &root,
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
        .lease(workspace_identity, &root)
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
    tokio::fs::create_dir_all(&root)
        .await
        .expect("create concurrent cold restore project root");
    let root = tokio::fs::canonicalize(root)
        .await
        .expect("canonicalize concurrent cold restore project root");
    let registry = std::sync::Arc::new(
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry"),
    );
    let workspace_identity = "workspace-concurrent-cold-restore";
    let source = b"pub fn restored() {}\n";
    let import = agent_semantic_client_db::ClientDbSourceIndexImport {
        generation_id: agent_semantic_client_core::CacheGenerationId::from(
            "concurrent-cold-restore",
        ),
        project_root: root.clone(),
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
        Vec::new(),
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
        .lease(workspace_identity, &root)
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
            generation(workspace_identity, &root, 1, b"epoch-one"),
        )
        .await
        .expect("publish initial canonical generation");
    let old_lease = registry
        .lease(workspace_identity, &root)
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
            generation(workspace_identity, &root, 2, b"epoch-two"),
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
        .lease(workspace_identity, &root)
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
            generation(workspace_identity, &root, 1, b"canonical"),
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
            &root,
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
        .lease(workspace_identity, &root)
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
