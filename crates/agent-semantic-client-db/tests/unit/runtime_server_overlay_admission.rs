use std::sync::atomic::{AtomicU64, Ordering};

use agent_semantic_client_db::runtime_server_workspace::{
    RuntimeServerWorkspaceRegistry, WORKSPACE_GENERATION_DELTA_SCHEMA_ID,
    WorkspaceCanonicalMaterialization, WorkspaceGenerationDelta, WorkspaceMemoryGeneration,
    WorkspaceOwnerSnapshot, WorkspaceRecoverySource,
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
    generation_with_selectors(workspace_identity, project_root, epoch, bytes, Vec::new())
}

fn generation_with_selectors(
    workspace_identity: &str,
    project_root: &std::path::Path,
    epoch: u64,
    bytes: &[u8],
    selectors: Vec<agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot>,
) -> WorkspaceMemoryGeneration {
    let content_digest = format!("blake3-256:{}", blake3::hash(bytes).to_hex());
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", content_digest.clone())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        format!(
            "blake3-256:{}",
            blake3::hash(b"runtime-overlay-fixture-provider").to_hex()
        ),
    );
    let module_graph_digest = format!(
        "blake3-256:{}",
        blake3::hash(b"runtime-overlay-fixture-module-graph").to_hex()
    );
    let runtime_provider_execution_binding =
        agent_semantic_artifacts::installed_provider_binding::RuntimeProviderExecutionBinding::build(
            crate::fixture::FIXTURE_PROJECT_ID.to_owned(),
            workspace_identity.to_owned(),
            format!("blake3-256:{}", "1".repeat(64)),
            format!("blake3-256:{}", "2".repeat(64)),
            format!("blake3-256:{}", "3".repeat(64)),
            source_snapshot
                .root_integrity_reference()
                .expect("fixture source snapshot integrity reference"),
            module_graph_digest.clone(),
        )
        .expect("fixture Runtime provider execution binding");
    let content_search_generation =
        crate::fixture::content_search_generation_receipt(workspace_identity, &source_snapshot);
    WorkspaceMemoryGeneration::try_from_build(
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationBuild {
            projection_capability: crate::fixture::projection_capability_manifest_fixture(),
            relations: Vec::new(),
            workspace_identity: workspace_identity.to_owned(),
            project_root: project_root.display().to_string(),
            active_epoch: epoch,
            workspace_snapshot,
            content_search_generation,
            source_snapshot,
            module_graph_digest,
            runtime_provider_execution_binding: Some(runtime_provider_execution_binding),
            project_resolutions: Vec::new(),
            owners: vec![WorkspaceOwnerSnapshot {
                authority: None,
                owner_path: "src/lib.rs".to_owned(),
                content_digest,
                bytes: bytes.to_vec(),
                selectors,
            }],
        },
    )
    .expect("typed runtime overlay generation")
}

#[test]
fn canonical_materialization_binds_snapshot_import_and_complete_owner_count() {
    use sha2::Digest as _;

    let project_root = fixture_root();
    std::fs::create_dir_all(&project_root).expect("create canonical materialization project root");
    let import = agent_semantic_client_db::build_source_index_import(
        agent_semantic_client_db::ClientDbSourceIndexImportRequest {
            source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
                [(
                    agent_semantic_client_db::ClientDbSourceIndexPath::from("src/lib.rs"),
                    b"source".to_vec(),
                )],
            ),
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
                relations: Vec::new(),
                relative_path: "src/lib.rs".to_owned(),
                language_id: "rust".into(),
                provider_id: "asp-rust".into(),
                text: "source".to_owned(),
                selectors: vec![crate::db_engine_source_index::rust_selector_fixture(
                    "src/lib.rs",
                    "rust://src/lib.rs#item/function/fixture",
                    "fixture",
                    b"source",
                )],
            }],
        },
    )
    .expect("build canonical materialization source-index import");
    let workspace_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        [("src/lib.rs", blake3::hash(b"source").to_hex().to_string())],
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
    );
    let materialization = WorkspaceCanonicalMaterialization::new(
        "workspace-canonical-materialization",
        source_snapshot.clone(),
        &import,
        [1, 0],
        vec![WorkspaceOwnerSnapshot {
            authority: None,
            owner_path: "src/lib.rs".to_owned(),
            content_digest: format!("blake3-256:{}", blake3::hash(b"source").to_hex()),
            bytes: b"source".to_vec(),
            selectors: vec![
                agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                    selector: "rust://src/lib.rs#item/function/fixture".to_owned(),
                    byte_start: 0,
                    byte_end: b"source".len(),
                    query_keys: Vec::new(),
                    derived_projections: Vec::new(),
                },
            ],
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
                authority: None,
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selector_overlay_atomically_rebinds_a_stale_generation_owner() {
    let root = fixture_root();
    let workspace_identity = "workspace-selector-live-owner-rebind";
    let selector = "rust://src/lib.rs#item/function/live";
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let base =
        generation_with_selectors(workspace_identity, &root, 1, b"fn stale() {}", Vec::new());
    registry
        .publish(
            "selector-live-owner-base",
            WorkspaceRecoverySource::TursoGeneration,
            base,
        )
        .await
        .expect("publish canonical base generation");

    let bytes = b"fn live() {}".to_vec();
    let owner_content_digest = format!("blake3-256:{}", blake3::hash(&bytes).to_hex());
    let owner = WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/lib.rs".to_owned(),
        content_digest: owner_content_digest.clone(),
        bytes: bytes.clone(),
        selectors: vec![
            agent_semantic_client_db::runtime_server_workspace::WorkspaceSelectorSnapshot {
                selector: selector.to_owned(),
                byte_start: 0,
                byte_end: bytes.len(),
                query_keys: Vec::new(),
                derived_projections: Vec::new(),
            },
        ],
    };
    let receipt = registry
        .rebind_selector_overlay(
            workspace_identity,
            &root,
            agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRebind {
                owner,
                overlay:
                    agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay {
                projection_kind:
                    agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                structural_selector: selector.to_owned(),
                owner_path: "src/lib.rs".to_owned(),
                owner_content_digest: owner_content_digest.clone(),
                byte_start: 0,
                byte_end: bytes.len(),
                projection_bytes: bytes.clone(),
                    },
            },
        )
        .await
        .expect("atomically rebind live owner and selector projection");
    assert!(receipt.inserted);
    let rebound_owner = registry
        .read_runtime_owner(workspace_identity, &root, "src/lib.rs")
        .expect("read rebound resident owner");
    assert!(matches!(
        rebound_owner,
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner {
            owner,
            ..
        } if owner.content_digest == owner_content_digest
    ));
    assert!(matches!(
        registry
            .read_runtime_selector(
                workspace_identity,
                &root,
                agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::Source,
                selector,
            )
            .expect("read rebound selector projection"),
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead::Projection {
            bytes: projection,
            ..
        } if projection == bytes
    ));

    let shutdown = registry.shutdown().await.expect("drain writer lane");
    assert!(shutdown.queued_publications_drained);
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
                authority: None,
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
async fn multi_owner_delta_publishes_one_atomic_generation_epoch() {
    let root = fixture_root();
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let workspace_identity = "workspace-owner-delta";
    registry
        .publish(
            "publish-canonical-before-delta",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, &root, 1, b"fn previous() {}\n"),
        )
        .await
        .expect("publish canonical generation");
    let old_lease = registry
        .lease(workspace_identity, &root)
        .expect("lease generation before delta");
    let first = b"fn first() {}\n";
    let second = b"fn second() {}\n";
    let receipt = registry
        .publish_owner_delta(
            "publish-owner-delta-atomically",
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest: old_lease.generation().generation_digest.clone(),
                owners: vec![
                    WorkspaceOwnerSnapshot {
                        authority: None,
                        owner_path: "src/first.rs".to_owned(),
                        content_digest: format!("blake3-256:{}", blake3::hash(first).to_hex()),
                        bytes: first.to_vec(),
                        selectors: Vec::new(),
                    },
                    WorkspaceOwnerSnapshot {
                        authority: None,
                        owner_path: "src/second.rs".to_owned(),
                        content_digest: format!("blake3-256:{}", blake3::hash(second).to_hex()),
                        bytes: second.to_vec(),
                        selectors: Vec::new(),
                    },
                ],
                tombstones: vec!["src/lib.rs".to_owned()],
                relations: Vec::new(),
            },
        )
        .await
        .expect("publish owner delta in one writer epoch");

    assert_eq!(receipt.active_epoch, 1);
    assert_eq!(receipt.target_epoch, 2);
    assert_eq!(old_lease.epoch(), 1);
    assert!(old_lease.owner("src/lib.rs").is_some());
    let current = registry
        .lease(workspace_identity, &root)
        .expect("lease generation after delta");
    assert_eq!(current.epoch(), 2);
    assert!(current.owner("src/lib.rs").is_none());
    assert_eq!(
        current.owner("src/first.rs").as_deref(),
        Some(first.as_slice())
    );
    assert_eq!(
        current.owner("src/second.rs").as_deref(),
        Some(second.as_slice())
    );
    assert_eq!(current.generation().source_snapshot.leaf_count, 2);
    assert_eq!(current.generation().workspace_generation.owner_count, 2);
    assert_eq!(
        current.generation().workspace_generation.root_digest,
        current.generation().source_snapshot.root_digest
    );

    registry
        .shutdown()
        .await
        .expect("drain owner delta writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stale_owner_delta_is_rejected_before_pointer_or_epoch_change() {
    let root = fixture_root();
    let registry =
        RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create workspace registry");
    let workspace_identity = "workspace-owner-delta-cas";
    registry
        .publish(
            "publish-canonical-before-cas",
            WorkspaceRecoverySource::TursoGeneration,
            generation(workspace_identity, &root, 1, b"fn baseline() {}\n"),
        )
        .await
        .expect("publish canonical generation");
    let baseline = registry
        .lease(workspace_identity, &root)
        .expect("lease baseline generation");
    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &root,
            workspace_identity,
            &root,
        )
        .expect("baseline pointer path");
    let pointer_before = tokio::fs::read(&pointer_path)
        .await
        .expect("read baseline pointer");
    let baseline_epoch = baseline.epoch();
    let baseline_digest = baseline.generation().generation_digest.clone();
    let owner = WorkspaceOwnerSnapshot {
        authority: None,
        owner_path: "src/lib.rs".to_owned(),
        content_digest: format!("blake3-256:{}", blake3::hash(b"fn next() {}\n").to_hex()),
        bytes: b"fn next() {}\n".to_vec(),
        selectors: Vec::new(),
    };
    let stale = registry
        .publish_owner_delta(
            "publish-stale-owner-delta",
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest:
                    "blake3-256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_owned(),
                owners: vec![owner.clone()],
                tombstones: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect_err("stale owner delta must fail closed");
    assert!(stale.contains("base generation digest mismatch"));
    let unchanged = registry
        .lease(workspace_identity, &root)
        .expect("lease unchanged generation");
    let pointer_after_stale = tokio::fs::read(&pointer_path)
        .await
        .expect("read pointer after stale delta");
    assert_eq!(pointer_after_stale, pointer_before);
    assert_eq!(unchanged.epoch(), baseline_epoch);
    assert_eq!(unchanged.generation().generation_digest, baseline_digest);
    assert!(unchanged.owner("src/lib.rs").is_some());

    let valid = registry
        .publish_owner_delta(
            "publish-valid-owner-delta-after-cas",
            workspace_identity,
            &root,
            WorkspaceGenerationDelta {
                schema_id: WORKSPACE_GENERATION_DELTA_SCHEMA_ID.to_owned(),
                schema_version: "2".to_owned(),
                base_generation_digest: baseline_digest.clone(),
                owners: vec![owner],
                tombstones: Vec::new(),
                relations: Vec::new(),
            },
        )
        .await
        .expect("current-base owner delta publishes");
    assert_eq!(valid.target_epoch, baseline_epoch + 1);
    assert!(valid.old_generation_readable);
    let current = registry
        .lease(workspace_identity, &root)
        .expect("lease published generation");
    assert_ne!(current.generation().generation_digest, baseline_digest);
    let pointer_after_valid = tokio::fs::read(&pointer_path)
        .await
        .expect("read pointer after valid delta");
    assert_ne!(pointer_after_valid, pointer_before);
    let data_plane = agent_semantic_client_db::runtime_server_workspace::WorkspaceSearchGenerationDataPlaneClient::open(
        &pointer_path,
        &root,
    )
    .await
    .expect("open fresh published search generation");
    let fresh_owner = data_plane
        .read_merkle_owner("src/lib.rs")
        .expect("read fresh owner proof");
    assert!(matches!(
        fresh_owner,
        agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeMerkleOwnerRead::Owner { .. }
    ));
    assert_eq!(registry.data_plane_counters().database_opens, 0);
    assert_eq!(registry.data_plane_counters().provider_spawns, 0);
    registry.shutdown().await.expect("drain CAS writer lane");
    let _ = tokio::fs::remove_dir_all(root).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn published_generation_serves_exact_byte_evidence_without_external_io() {
    let root = fixture_root().join("resident-byte-evidence");
    let registry = RuntimeServerWorkspaceRegistry::new(root.clone()).expect("create registry");
    let workspace_identity = "workspace-resident-byte-evidence";
    registry
        .publish(
            "publish-resident-byte-evidence",
            WorkspaceRecoverySource::TursoGeneration,
            generation(
                workspace_identity,
                &root,
                1,
                b"fn exact_literal_marker() {}\n",
            ),
        )
        .await
        .expect("publish canonical generation");
    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &root,
            workspace_identity,
            &root,
        )
        .expect("pointer path");
    let data_plane =
        agent_semantic_client_db::runtime_resident_read::RuntimeResidentReadClient::open(
            &pointer_path,
            &root,
        )
        .await
        .expect("open resident generation");
    let present = data_plane
        .read_byte_evidence("exact_literal_marker", None, 8)
        .expect("read exact byte evidence");
    assert_eq!(present.hits.len(), 1);
    assert_eq!(present.hits[0].owner_path, "src/lib.rs");
    let scoped_present = data_plane
        .read_byte_evidence_for_owner_scope("exact_literal_marker", "src/lib.rs", None, 8)
        .expect("read owner-scoped exact byte evidence");
    assert_eq!(scoped_present.hits.len(), 1);
    assert_eq!(scoped_present.hits[0].owner_path, "src/lib.rs");
    let wrong_owner = data_plane
        .read_byte_evidence_for_owner_scope("exact_literal_marker", "src/other.rs", None, 8)
        .expect("prove owner-scoped exact byte absence");
    assert!(wrong_owner.hits.is_empty());
    let scoped_source_index = data_plane
        .read_source_index_for_owner_scope("exact_literal_marker", "src/lib.rs", None, 8)
        .expect("read owner-scoped resident source index");
    assert_eq!(scoped_source_index.hits.len(), 1);
    assert_eq!(scoped_source_index.hits[0].owner_path, "src/lib.rs");
    let absent = data_plane
        .read_byte_evidence("definitely_absent_literal", None, 8)
        .expect("prove exact byte absence");
    assert!(absent.hits.is_empty());
    assert!(data_plane.read_byte_evidence("x", None, 8).is_err());
    let mut samples = Vec::with_capacity(2_048);
    for _ in 0..2_048 {
        let started = std::time::Instant::now();
        let result = data_plane
            .read_byte_evidence("exact_literal_marker", None, 8)
            .expect("warm resident byte evidence");
        samples.push(started.elapsed().as_nanos());
        assert_eq!(result.hits.len(), 1);
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    let p99 = samples[samples.len() * 99 / 100];
    assert!(
        p95 < 1_000_000,
        "resident byte evidence p95 exceeds 1ms: {p95}"
    );
    println!("residentByteEvidence samples=2048 p95Ns={p95} p99Ns={p99}");
    assert_eq!(registry.data_plane_counters().database_opens, 0);
    assert_eq!(registry.data_plane_counters().provider_spawns, 0);
    assert_eq!(registry.data_plane_counters().control_socket_roundtrips, 0);
    registry.shutdown().await.expect("shutdown registry");
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
        source_blobs: Default::default(),
        relations: Vec::new(),
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
            provider_id: Some("asp-rust".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: Vec::new(),
        }],
        selectors: vec![crate::db_engine_source_index::rust_selector_fixture(
            "src/lib.rs",
            "rust://src/lib.rs#item/function/restored",
            "restored",
            b"pub fn restored() {}\n",
        )],
    };
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/lib.rs",
        blake3::hash(source).to_hex().to_string(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        blake3::hash(b"concurrent-cold-restore-provider")
            .to_hex()
            .to_string(),
    );
    let materialization = WorkspaceCanonicalMaterialization::new(
        workspace_identity,
        source_snapshot,
        &import,
        [1, 0],
        vec![WorkspaceOwnerSnapshot {
            authority: None,
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
        let materialization = materialization
            .clone()
            .into_validated(workspace_identity)
            .expect("validate per-request canonical cold-restore materialization");
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
    assert_eq!(
        counters.filesystem_reads, 0,
        "canonical cold restore must publish from the validated MemoryBackend without a data-plane filesystem read"
    );
    assert_eq!(counters.filesystem_writes, 1);
    assert_eq!(counters.database_opens, 0);
    assert_eq!(counters.provider_spawns, 0);
    assert_eq!(counters.control_socket_roundtrips, 0);

    let pointer_path =
        agent_semantic_client_db::runtime_server_workspace::workspace_generation_pointer_path(
            &root,
            workspace_identity,
            &root,
        )
        .expect("resolve active generation pointer");
    tokio::fs::remove_file(&pointer_path)
        .await
        .expect("remove the published pointer while resident memory remains warm");
    assert_eq!(
        registry
            .published_generation_state(workspace_identity, &root)
            .await
            .expect("observe missing immutable generation"),
        agent_semantic_client_db::runtime_server_workspace::PublishedWorkspaceGenerationState::Missing,
    );
    let resident_republication = registry
        .admit_canonical_generation_resident(
            "resident-ready-before-durability",
            workspace_identity,
            materialization
                .clone()
                .into_validated(workspace_identity)
                .expect("validate resident republish materialization"),
        )
        .await
        .expect("resident publication must not wait for its durable pointer");
    let resident_lease = registry
        .lease(workspace_identity, &root)
        .expect("resident generation remains queryable");
    assert_eq!(resident_lease.epoch(), resident_republication.target_epoch);
    registry
        .wait_canonical_generation_durable(
            workspace_identity,
            &root,
            &resident_republication.generation_digest,
            resident_republication.target_epoch,
        )
        .await
        .expect("explicit restore boundary waits for durability");
    let current_snapshot =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationPointerReader::open(
            &pointer_path,
        )
        .await
        .expect("open current generation pointer")
        .read()
        .expect("read current generation snapshot");
    let mut incompatible_snapshot =
        serde_json::to_value(current_snapshot).expect("encode current generation snapshot");
    let incompatible_object = incompatible_snapshot
        .as_object_mut()
        .expect("generation snapshot object");
    incompatible_object.insert(
        "schemaId".to_owned(),
        serde_json::json!("agent.semantic-protocols.runtime-server-workspace-generation.v1"),
    );
    incompatible_object.insert("schemaVersion".to_owned(), serde_json::json!("1"));
    let incompatible_payload = serde_json::to_vec(&incompatible_snapshot)
        .expect("encode complete incompatible pointer fixture");
    let mut incompatible_pointer = vec![0_u8; 4_096];
    incompatible_pointer[..8].copy_from_slice(&2_u64.to_ne_bytes());
    incompatible_pointer[8..16].copy_from_slice(&(incompatible_payload.len() as u64).to_ne_bytes());
    incompatible_pointer[16..16 + incompatible_payload.len()]
        .copy_from_slice(&incompatible_payload);
    tokio::fs::write(&pointer_path, incompatible_pointer)
        .await
        .expect("publish an incompatible generation pointer");

    // The resident data plane deliberately does not poll the pointer on every
    // warm query. Model a real process-cold repair boundary: the old resident
    // drains, and the process-local load-once cell disappears with the process.
    // The incompatible pointer's unvalidated epoch is not generation authority.
    registry
        .shutdown()
        .await
        .expect("drain original writer lane");
    let registry = std::sync::Arc::new(
        RuntimeServerWorkspaceRegistry::new(root.clone())
            .expect("create process-cold repair registry"),
    );
    let repaired = registry
        .ensure_canonical_generation(
            "repair-incompatible-pointer",
            workspace_identity,
            materialization
                .clone()
                .into_validated(workspace_identity)
                .expect("validate repair materialization"),
        )
        .await
        .expect("republish current-schema pointer through the writer lane");
    assert_eq!(repaired.target_epoch, 1);
    let mut durability_updates = registry
        .subscribe_generation_durability(workspace_identity, &root)
        .expect("subscribe repaired generation durability");
    let durability = loop {
        if let Some(receipt) = durability_updates.borrow().clone()
            && receipt.target_epoch == repaired.target_epoch
            && receipt.state
                == agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady
        {
            break receipt;
        }
        durability_updates
            .changed()
            .await
            .expect("repaired generation durability lane remains available");
    };
    durability
        .validate()
        .expect("validate repaired durability receipt");
    assert_eq!(durability.target_epoch, repaired.target_epoch);
    assert_eq!(
        durability.state,
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationDurabilityState::DurableReady,
        "explicit restart-restore boundary requires canonical pointer durability"
    );
    let repaired_pointer =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceGenerationPointerReader::open(
            &pointer_path,
        )
        .await
        .expect("open repaired generation pointer")
        .read()
        .expect("decode repaired generation pointer");
    repaired_pointer
        .validate()
        .expect("validate repaired current-schema pointer");
    assert_eq!(
        repaired_pointer.source_kind,
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem
    );

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
                authority: None,
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
