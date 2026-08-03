use std::sync::Arc;

use agent_semantic_client_db::{
    ProviderIncrementalOwnerWrite, ProviderOwnerFingerprint, ProviderOwnerMetadata,
    WorkspaceDbRegistry, WorkspaceDbRegistryCounters,
};

use crate::test_support::{StateHomeGuard, environment_lock, workspace};
use tempfile::TempDir;

#[tokio::test(flavor = "current_thread")]
async fn wrong_workspace_identity_fails_before_database_open() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create wrong-identity tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, resolved, mut scope) = workspace(temp.path(), "wrong-identity");
    let client_db_path = resolved.paths.client_db_path;
    scope.workspace_identity = "workspace-wrong".to_owned();
    let registry = WorkspaceDbRegistry::default();

    let error = registry
        .acquire(&project_root, &scope)
        .await
        .expect_err("wrong workspace identity must fail closed");

    assert!(error.contains("workspace identity mismatch"));
    let counters = registry.counters();
    assert_eq!(counters.workspace_resolution_count, 1);
    assert_eq!(counters.project_root_canonicalization_count, 0);
    assert_eq!(counters.database_open_count, 0);
    assert_eq!(counters.schema_bootstrap_count, 0);
    assert!(!client_db_path.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn finish_loaded_writes_does_not_bootstrap_an_unused_workspace() {
    let registry = WorkspaceDbRegistry::default();

    registry
        .finish_loaded_writes(
            agent_semantic_client_db::WorkspaceDbWriteFinishMode::OwnerDurabilityBoundary,
        )
        .await
        .expect("finish an empty resident registry");

    assert_eq!(registry.counters(), WorkspaceDbRegistryCounters::default());
}

#[tokio::test(flavor = "current_thread")]
async fn member_project_root_reuses_the_canonical_workspace_entry() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create member-root tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (workspace_root, _resolved, mut scope) = workspace(temp.path(), "member-root");
    let member_root = workspace_root.join("crates/member");
    std::fs::create_dir_all(&member_root).expect("create nested workspace member");
    scope.project_root = member_root.display().to_string();
    let registry = WorkspaceDbRegistry::default();

    let root_session = registry
        .acquire(&workspace_root, &scope)
        .await
        .expect("workspace root must accept a member-scoped provider request");
    let member_session = registry
        .acquire(&member_root, &scope)
        .await
        .expect("workspace member must resolve through the canonical workspace entry");

    assert_eq!(
        root_session.workspace_identity(),
        member_session.workspace_identity()
    );
    assert_eq!(
        root_session.client_db_path(),
        member_session.client_db_path()
    );
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.registry_hit_count, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn concurrent_leases_resolve_open_and_bootstrap_once() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create concurrent tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, _resolved, scope) = workspace(temp.path(), "concurrent");
    let registry = Arc::new(WorkspaceDbRegistry::default());
    let mut leases = tokio::task::JoinSet::new();
    let session_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .saturating_mul(16)
        .clamp(32, 512);
    let cold_started = tokio::time::Instant::now();
    for _ in 0..session_count {
        let registry = Arc::clone(&registry);
        let project_root = project_root.clone();
        let scope = scope.clone();
        leases.spawn(async move { registry.acquire(project_root, &scope).await });
    }
    let mut sessions = Vec::new();
    while let Some(session) = leases.join_next().await {
        sessions.push(session.expect("concurrent lease task must join"));
    }

    assert_eq!(sessions.len(), session_count);
    assert!(sessions.into_iter().all(|session| {
        session
            .expect("concurrent lease must succeed")
            .workspace_identity()
            == scope.workspace_identity
    }));
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.registry_hit_count, session_count as u64 - 1);
    assert_eq!(counters.workspace_resolution_count, 1);
    assert_eq!(counters.project_root_canonicalization_count, 1);
    assert_eq!(counters.workspace_lock_retry_count, 0);

    let cold_elapsed = cold_started.elapsed();
    let mut warm_leases = tokio::task::JoinSet::new();
    for _ in 0..session_count {
        let registry = Arc::clone(&registry);
        let project_root = project_root.clone();
        let scope = scope.clone();
        warm_leases.spawn(async move {
            let started = tokio::time::Instant::now();
            let session = registry.acquire(project_root, &scope).await;
            (session, started.elapsed())
        });
    }
    let mut warm_latencies = Vec::with_capacity(session_count);
    while let Some(completed) = warm_leases.join_next().await {
        let (session, latency) = completed.expect("warm admission task must join");
        assert_eq!(
            session.expect("warm admitted session").workspace_identity(),
            scope.workspace_identity
        );
        warm_latencies.push(latency);
    }
    warm_latencies.sort_unstable();
    let p99_index = session_count.saturating_mul(99).div_ceil(100) - 1;
    let warm_p99 = warm_latencies[p99_index];
    assert!(
        warm_p99 < std::time::Duration::from_millis(1),
        "{session_count} warm workspace admissions exceeded sub-millisecond p99: {warm_p99:?}"
    );
    let warm_counters = registry.counters();
    assert_eq!(warm_counters.database_open_count, 1);
    assert_eq!(
        warm_counters.connection_create_count, counters.connection_create_count,
        "warm admission must not grow the resident Turso read pool"
    );
    assert_eq!(warm_counters.schema_bootstrap_count, 1);
    assert_eq!(warm_counters.workspace_resolution_count, 1);
    assert_eq!(warm_counters.project_root_canonicalization_count, 1);
    eprintln!(
        "workspace-admission-performance sessionCount={session_count} coldTotalMicros={} \
         warmP99Micros={} databaseOpens={} schemaBootstraps={} workspaceResolutions={} \
         projectRootCanonicalizations={}",
        cold_elapsed.as_micros(),
        warm_p99.as_micros(),
        warm_counters.database_open_count,
        warm_counters.schema_bootstrap_count,
        warm_counters.workspace_resolution_count,
        warm_counters.project_root_canonicalization_count,
    );
}

#[tokio::test(flavor = "current_thread")]
async fn different_workspaces_initialize_independent_entries() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create independent tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root_a, _resolved_a, scope_a) = workspace(temp.path(), "workspace-a");
    let (project_root_b, _resolved_b, scope_b) = workspace(temp.path(), "workspace-b");
    let registry = WorkspaceDbRegistry::default();

    let (left, right) = tokio::join!(
        registry.acquire(&project_root_a, &scope_a),
        registry.acquire(&project_root_b, &scope_b),
    );
    let left = left.expect("first workspace lease must succeed");
    let right = right.expect("second workspace lease must succeed");

    assert_ne!(left.workspace_identity(), right.workspace_identity());
    assert_ne!(left.client_db_path(), right.client_db_path());
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 2);
    let available_parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    assert!(
        counters.connection_create_count >= 4
            && counters.connection_create_count
                <= u64::try_from(2 * (available_parallelism + 1)).unwrap_or(u64::MAX)
            && counters.connection_create_count % 2 == 0,
        "two workspaces must each prebuild one adaptive read pool plus one writer connection: {counters:?}"
    );
    assert_eq!(counters.schema_bootstrap_count, 2);
    assert_eq!(counters.workspace_lock_retry_count, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn resident_turso_session_restores_an_empty_memory_backend_without_reopening_the_database() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create cold-restore tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, _resolved, scope) = workspace(temp.path(), "cold-restore");
    let durable_registry = WorkspaceDbRegistry::default();
    let session = durable_registry
        .acquire(&project_root, &scope)
        .await
        .expect("admit durable Turso session");
    let source = b"pub fn resident_restore() {}\n";
    let file_hashes = vec![agent_semantic_client_core::ClientCacheFileHash {
        path: "src/lib.rs".to_owned(),
        sha256: "11".repeat(32),
        byte_len: source.len() as u64,
        mtime_ms: 1,
    }];
    let source_snapshot = agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes([(
        "src/lib.rs",
        blake3::hash(source).to_hex().to_string(),
    )])
    .evidence(
        agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
        blake3::hash(b"resident-provider").to_hex().to_string(),
    );
    let import = agent_semantic_client_db::ClientDbSourceIndexImport {
        relations: Vec::new(),
        generation_id: agent_semantic_client_core::CacheGenerationId::from("resident-cold-restore"),
        project_root: project_root.clone(),
        schema_id: agent_semantic_client_core::SemanticSchemaId::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID,
        ),
        schema_version: agent_semantic_client_core::SemanticSchemaVersion::from(
            agent_semantic_client_db::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
        ),
        file_hashes,
        owners: vec![agent_semantic_client_db::ClientDbSourceIndexOwner {
            owner_path: "src/lib.rs".into(),
            language_id: Some("rust".into()),
            provider_id: Some("rs-harness".into()),
            source_kind: "file".into(),
            line_count: Some(1),
            query_keys: vec![],
        }],
        selectors: vec![],
    };
    let source_blobs =
        crate::projection_fixture::source_blobs_fixture([("src/lib.rs", source.as_slice())]);
    let materialization =
        agent_semantic_client_db::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
            scope.workspace_identity.clone(),
            &source_snapshot,
            &import,
            &source_blobs,
            Vec::new(),
        )
        .expect("build complete canonical materialization");
    session
        .commit_source_index_generation(
            agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
                import,
                file_count: 7,
                source_snapshot,
            },
            materialization,
        )
        .await
        .expect("commit durable canonical generation");
    let durable_baseline = durable_registry.counters();

    let memory_registry =
        agent_semantic_client_db::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(
            temp.path().join("runtime-workspaces"),
        )
        .expect("create empty MemoryBackend registry");
    let receipt =
        agent_semantic_client_db::runtime_server_workspace::restore_active_turso_generation(
            &memory_registry,
            &session,
            "restore-from-resident-turso",
            &scope.workspace_identity,
            std::path::Path::new(&scope.project_root),
        )
        .await
        .expect("restore canonical generation from resident Turso handle");
    assert_eq!(receipt.target_epoch, 1);
    assert_eq!(
        durable_registry.counters().database_open_count,
        durable_baseline.database_open_count,
        "cold MemoryBackend restore must reuse the resident Turso database"
    );
    assert_eq!(
        durable_registry.counters().schema_bootstrap_count,
        durable_baseline.schema_bootstrap_count,
        "cold MemoryBackend restore must not bootstrap Turso again"
    );
    let lease = memory_registry
        .lease(
            &scope.workspace_identity,
            std::path::Path::new(&scope.project_root),
        )
        .expect("lease restored MemoryBackend generation");
    assert_eq!(lease.generation().root_depth, [1, 0]);
    assert_eq!(
        lease.owner("src/lib.rs").expect("restored owner").as_ref(),
        source
    );
    let data_plane = memory_registry.data_plane_counters();
    assert_eq!(data_plane.database_opens, 0);
    assert_eq!(data_plane.provider_spawns, 0);
    assert_eq!(data_plane.control_socket_roundtrips, 0);
    assert_eq!(data_plane.filesystem_reads, 1);
    assert_eq!(data_plane.filesystem_writes, 1);

    memory_registry
        .shutdown()
        .await
        .expect("drain cold-restore writer lane");
}

#[tokio::test(flavor = "multi_thread")]
async fn two_hundred_concurrent_sessions_remain_isolated_across_two_workspaces() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create multi-workspace concurrency tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root_a, _resolved_a, scope_a) = workspace(temp.path(), "pressure-workspace-a");
    let (project_root_b, _resolved_b, scope_b) = workspace(temp.path(), "pressure-workspace-b");
    let registry = Arc::new(WorkspaceDbRegistry::default());
    let mut leases = tokio::task::JoinSet::new();

    for index in 0..200 {
        let registry = Arc::clone(&registry);
        let (project_root, scope) = if index % 2 == 0 {
            (project_root_a.clone(), scope_a.clone())
        } else {
            (project_root_b.clone(), scope_b.clone())
        };
        leases.spawn(async move {
            let session = registry.acquire(project_root, &scope).await?;
            Ok::<_, String>((
                scope.workspace_identity,
                session.workspace_identity().to_owned(),
                session.client_db_path().to_path_buf(),
            ))
        });
    }

    let mut workspace_a_paths = std::collections::BTreeSet::new();
    let mut workspace_b_paths = std::collections::BTreeSet::new();
    while let Some(lease) = leases.join_next().await {
        let (expected_identity, actual_identity, db_path) = lease
            .expect("multi-workspace lease task must join")
            .expect("multi-workspace lease must succeed");
        assert_eq!(actual_identity, expected_identity);
        if actual_identity == scope_a.workspace_identity {
            workspace_a_paths.insert(db_path);
        } else if actual_identity == scope_b.workspace_identity {
            workspace_b_paths.insert(db_path);
        } else {
            panic!("session escaped both requested workspace identities: {actual_identity}");
        }
    }

    assert_eq!(workspace_a_paths.len(), 1);
    assert_eq!(workspace_b_paths.len(), 1);
    assert_ne!(workspace_a_paths, workspace_b_paths);
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 2);
    let available_parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    assert!(
        counters.connection_create_count >= 4
            && counters.connection_create_count
                <= u64::try_from(2 * (available_parallelism + 1)).unwrap_or(u64::MAX)
            && counters.connection_create_count % 2 == 0,
        "two workspaces must each prebuild one adaptive read pool plus one writer connection: {counters:?}"
    );
    assert_eq!(counters.schema_bootstrap_count, 2);
    assert_eq!(counters.registry_hit_count, 198);
    assert_eq!(counters.workspace_lock_retry_count, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn one_hundred_concurrent_writes_share_one_serial_writer() {
    let _environment = environment_lock();
    let temp = TempDir::new().expect("create concurrent-writers tempfile");
    let state_home = temp.path().join("state");
    let _state_home = StateHomeGuard::install(&state_home);
    let (project_root, _resolved, mut scope) = workspace(temp.path(), "concurrent-writers");
    scope.provider_workspace_identity_digest = format!("{:064x}", 17);
    let registry = Arc::new(WorkspaceDbRegistry::default());
    let session = registry
        .acquire(&project_root, &scope)
        .await
        .expect("acquire writer test workspace");
    let mut writes = tokio::task::JoinSet::new();
    for index in 0..100 {
        let session = session.clone();
        let scope = scope.clone();
        writes.spawn(async move {
            let source_bytes = vec![u8::try_from(index % 256).expect("fixture byte")];
            let content_digest =
                agent_semantic_content_identity::ArtifactHash::blake3(source_bytes.as_slice())
                    .value;
            session
                .write_provider_incremental_owner(&ProviderIncrementalOwnerWrite {
                    scope,
                    owner_path: format!("src/owner-{index:03}.rs"),
                    fingerprint: ProviderOwnerFingerprint {
                        metadata: ProviderOwnerMetadata {
                            file_identity: format!("writer-file-{index}"),
                            size_bytes: 1,
                            modified_unix_nanos: index,
                            change_time_unix_nanos: index,
                        },
                        content_digest,
                    },
                    source_bytes,
                    projection_completeness: "complete-owner".to_owned(),
                    projections: Vec::new(),
                })
                .await
        });
    }
    let mut completed = 0;
    while let Some(write) = writes.join_next().await {
        write
            .expect("concurrent writer task must join")
            .expect("concurrent writer transaction must commit");
        completed += 1;
    }

    assert_eq!(completed, 100);
    let counters = registry.counters();
    assert_eq!(counters.database_open_count, 1);
    assert_eq!(counters.connection_create_count, 2);
    assert_eq!(counters.schema_bootstrap_count, 1);
    assert_eq!(counters.writer_transaction_count, 100);
    assert_eq!(counters.max_active_writer_count, 1);
}
