use tempfile::TempDir;

use crate::test_support::{StateHomeGuard, environment_lock, workspace};
use agent_semantic_client_db::workspace_db_ipc::{
    WorkspaceDbIpcOperation, WorkspaceDbIpcRequest, WorkspaceDbIpcResult, WorkspaceDbOwnerEndpoint,
    WorkspaceDbSourceIndexLookupRequest, bind_workspace_db_owner, call_workspace_db_owner,
    prepare_workspace_db_owner_endpoint, serve_one_workspace_db_ipc_request,
    serve_one_workspace_db_session_request, workspace_db_owner_runtime_base,
};
use agent_semantic_client_db::{
    ProviderOwnerBatchProbeRequest, ProviderOwnerDecision, ProviderOwnerInventoryEntry,
    ProviderOwnerInventoryEntryState, ProviderOwnerInventoryState, ProviderOwnerInventoryWrite,
    ProviderOwnerMetadata, WorkspaceDbRegistry, WorkspaceDbWriteFinishMode,
};

#[test]
fn canonical_owner_runtime_root_ignores_long_process_temp_directory() {
    let runtime = workspace_db_owner_runtime_base();
    assert_eq!(runtime.parent(), Some(std::path::Path::new("/tmp")));
    assert!(
        runtime.as_os_str().len() < 32,
        "canonical owner runtime root must preserve the Unix socket path budget: {}",
        runtime.display()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn deep_state_home_uses_short_private_socket_and_roundtrips_typed_frame() {
    let fixture = TempDir::new().expect("create workspace IPC tempfile");
    let deep_state_home = (0..20).fold(fixture.path().join("state"), |path, index| {
        path.join(format!("deep-state-segment-{index}"))
    });
    std::fs::create_dir_all(&deep_state_home).expect("create deep State Home fixture");
    let runtime = fixture.path().join("r");
    let endpoint =
        prepare_workspace_db_owner_endpoint(&runtime, "workspace-deep-state", 7, "token-7")
            .expect("prepare short owner endpoint");
    assert!(!endpoint.socket_path.contains("deep-state-segment"));
    assert!(endpoint.socket_path.len() <= 103);
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let request = WorkspaceDbIpcRequest {
        schema_id: "agent.semantic-protocols.workspace-db-owner-request.v1".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: endpoint.workspace_identity.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        request_id: "request-1".to_owned(),
        operation: WorkspaceDbIpcOperation::Health,
    };

    let (served, response) = tokio::join!(
        serve_one_workspace_db_ipc_request(&listener, &endpoint),
        call_workspace_db_owner(&endpoint, &request),
    );

    served.expect("serve typed owner request");
    let response = response.expect("call typed owner endpoint");
    assert_eq!(response.result, WorkspaceDbIpcResult::Healthy);
    assert_eq!(response.owner_epoch, 7);
}

#[tokio::test(flavor = "current_thread")]
async fn warm_workspace_owner_transport_is_sub_millisecond() {
    const SAMPLES: u64 = 32;
    let fixture = TempDir::new().expect("create workspace IPC performance fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("r"),
        "workspace-transport-performance",
        11,
        "token-11",
    )
    .expect("prepare workspace DB owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind workspace DB owner");
    let server = async {
        for _ in 0..=SAMPLES {
            serve_one_workspace_db_ipc_request(&listener, &endpoint)
                .await
                .expect("serve workspace DB health request");
        }
    };
    let client = async {
        let warmup = request(&endpoint, "warmup", WorkspaceDbIpcOperation::Health);
        call_workspace_db_owner(&endpoint, &warmup)
            .await
            .expect("warm workspace DB owner transport");
        let started = std::time::Instant::now();
        for sample in 0..SAMPLES {
            let health = request(
                &endpoint,
                &format!("performance-{sample}"),
                WorkspaceDbIpcOperation::Health,
            );
            call_workspace_db_owner(&endpoint, &health)
                .await
                .expect("call warm workspace DB owner transport");
        }
        started.elapsed()
    };
    let (_, elapsed) = tokio::join!(server, client);
    let average = elapsed / u32::try_from(SAMPLES).expect("sample count fits u32");
    println!(
        "[workspace-db-owner-performance] samples={SAMPLES} elapsedMicros={} averageNanos={} budgetNanos=1000000",
        elapsed.as_micros(),
        average.as_nanos()
    );
    assert!(
        average < std::time::Duration::from_millis(1),
        "warm workspace owner transport must average below 1ms, average={average:?} samples={SAMPLES}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn ipc_session_roundtrips_all_workspace_db_operations() {
    let _environment = environment_lock();
    let fixture = tempfile::TempDir::new().expect("create IPC fixture");
    let _state_home = StateHomeGuard::install(&fixture.path().join("state"));
    let (_project_root, _resolved, scope) = workspace(fixture.path(), "ipc-project-all-ops");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("r"),
        &scope.workspace_identity,
        9,
        "token-9",
    )
    .expect("prepare workspace DB owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind workspace DB owner");
    let registry = std::sync::Arc::new(WorkspaceDbRegistry::default());
    registry
        .bootstrap_workspace(&_project_root)
        .await
        .expect("bootstrap the resident workspace before publishing its IPC endpoint");
    let session = WorkspaceDbIpcSession::new(endpoint.clone());
    assert_eq!(
        session.workspace_identity(),
        endpoint.workspace_identity,
        "IPC session identity must be the workspace resident-service identity"
    );
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let last_activity_epoch_seconds = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

    let client = async {
        session
            .health()
            .await
            .expect("workspace identity and resident-service health must validate together");
        let source_index_request = WorkspaceDbSourceIndexLookupRequest {
            project_root: _project_root.clone(),
            indexed_project_root: _project_root.clone(),
            source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence {
                schema_id: "asp.source-snapshot.v1".to_owned(),
                algorithm: "blake3-256".to_owned(),
                root_digest: format!("{:064x}", 41),
                source_kind: agent_semantic_content_identity::SourceSnapshotKind::Filesystem,
                leaf_count: 1,
                base_root_digest: None,
                provider_digest: format!("{:064x}", 43),
                dirty_paths_digest: None,
            },
            query: "example".to_owned(),
            language_id: Some("rust".into()),
            limit: 32,
        };
        const SOURCE_INDEX_SAMPLES: usize = 32;
        let source_index_started = std::time::Instant::now();
        let mut reads = tokio::task::JoinSet::new();
        for _ in 0..SOURCE_INDEX_SAMPLES {
            let session = session.clone();
            let request = source_index_request.clone();
            reads.spawn(async move { session.read_source_index(&request).await });
        }
        while let Some(read) = reads.join_next().await {
            read.expect("join resident source-index read")
                .expect("resident source-index read must not open a competing Turso database");
        }
        let source_index_elapsed = source_index_started.elapsed();
        let average_nanos = source_index_elapsed.as_nanos() / (SOURCE_INDEX_SAMPLES as u128);
        assert!(
            average_nanos < 1_000_000,
            "warm resident source-index IPC average must remain sub-millisecond: samples={} elapsedMicros={} averageNanos={}",
            SOURCE_INDEX_SAMPLES,
            source_index_elapsed.as_micros(),
            average_nanos,
        );
        eprintln!(
            "[workspace-db-source-index-performance] samples={} elapsedMicros={} averageNanos={} budgetNanos=1000000",
            SOURCE_INDEX_SAMPLES,
            source_index_elapsed.as_micros(),
            average_nanos,
        );
        let owner_path = "src/lib.rs";
        let owner_content_digest = format!("{:064x}", 23);
        let inventory = ProviderOwnerInventoryWrite {
            scope: scope.clone(),
            state: ProviderOwnerInventoryState::Exact,
            entries: vec![ProviderOwnerInventoryEntry {
                owner_path: owner_path.to_owned(),
                owner_content_digest: Some(owner_content_digest.clone()),
                state: ProviderOwnerInventoryEntryState::Indexed,
            }],
        };
        let inventory_receipt = session
            .upsert_provider_owner_inventory(&inventory)
            .await
            .expect("write provider owner inventory through IPC session");

        let metadata = ProviderOwnerMetadata {
            file_identity: "file-1".to_owned(),
            size_bytes: 101,
            modified_unix_nanos: 1_001,
            change_time_unix_nanos: 2_001,
        };
        let incremental_write = ProviderIncrementalOwnerWrite {
            scope: scope.clone(),
            owner_path: owner_path.to_owned(),
            fingerprint: ProviderOwnerFingerprint {
                metadata: metadata.clone(),
                content_digest: owner_content_digest.clone(),
            },
            projection_completeness: "complete-owner".to_owned(),
            projections: vec![ProviderSelectorProjection {
                structural_selector: format!("rust://{owner_path}#item/function/example"),
                capture_name: "declaration.name".to_owned(),
                signature: "pub fn example()".to_owned(),
                item_kind: "function".to_owned(),
                item_name: "example".to_owned(),
                source_byte_start: 0,
                source_byte_end: 16,
            }],
        };
        session
            .write_provider_incremental_owner(&incremental_write)
            .await
            .expect("write incremental owner through IPC session");
        let projections = session
            .read_provider_owner_projections(&scope, owner_path)
            .await
            .expect("read incremental projections through IPC session");
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].item_name, "example");

        let query = ProviderTreeSitterQueryIdentity {
            scope: scope.clone(),
            query_digest: format!("{:064x}", 29),
            capture_names: vec!["declaration.name".to_owned()],
        };
        let tree_sitter_result = ProviderTreeSitterOwnerResult {
            owner_path: owner_path.to_owned(),
            owner_content_digest: owner_content_digest.clone(),
            query_digest: query.query_digest.clone(),
            inventory_generation: inventory_receipt.inventory_generation.clone(),
            state: ProviderTreeSitterOwnerResultState::Processed,
            complete_owner_refresh_count: 1,
            projections: vec![ProviderTreeSitterCaptureProjection {
                structural_selector: "rust://src/lib.rs#item/function/example-declaration.name-0"
                    .to_owned(),
                signature: "pub fn example()".to_owned(),
                item_kind: "function".to_owned(),
                item_name: "example".to_owned(),
                capture_name: "declaration.name".to_owned(),
                item_source_byte_start: 0,
                item_source_byte_end: 64,
                source_byte_start: 0,
                source_byte_end: 16,
            }],
        };
        session
            .write_provider_treesitter_owner_result(&query, &tree_sitter_result)
            .await
            .expect("write Tree-sitter owner result through IPC session");
        let read = session
            .read_provider_treesitter_query(&query, 32, None)
            .await
            .expect("read Tree-sitter query through IPC session");
        assert_eq!(read.cached_results.len(), 1);

        let probe = session
            .probe_provider_owners(
                &scope,
                &[ProviderOwnerBatchProbeRequest {
                    owner_path: owner_path.to_owned(),
                    metadata: ProviderOwnerMetadata {
                        file_identity: "file-1".to_owned(),
                        size_bytes: 1,
                        modified_unix_nanos: 1,
                        change_time_unix_nanos: 1,
                    },
                }],
            )
            .await
            .expect("probe provider owner through IPC session");
        assert_eq!(probe.results.len(), 1);

        session
            .finish_writes(&scope, WorkspaceDbWriteFinishMode::OwnerDurabilityBoundary)
            .await
            .expect("finish workspace DB writes through IPC session");
        shutdown_tx
            .send(true)
            .map_err(|_| "workspace DB IPC server shutdown receiver disappeared".to_owned())
    };

    let (served, client_result) = tokio::join!(
        serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            std::sync::Arc::clone(&registry),
            std::sync::Arc::clone(&last_activity_epoch_seconds),
            shutdown_rx,
        ),
        client
    );
    assert!(
        last_activity_epoch_seconds.load(std::sync::atomic::Ordering::Relaxed) > 0,
        "accepted IPC work must refresh resident-service activity"
    );
    client_result.expect("complete IPC session client flow");
    served.expect("serve IPC session until explicit shutdown");
    assert_eq!(
        registry.counters().database_open_count,
        1,
        "all concurrent source-index reads must reuse the resident workspace database"
    );
}

#[tokio::test]
async fn owner_session_roundtrip_writes_reads_and_finishes_durability() {
    let _environment = environment_lock();
    let fixture = TempDir::new().expect("create workspace session IPC tempfile");
    let _state_home = StateHomeGuard::install(&fixture.path().join("state"));
    let (project_root, _resolved, scope) = workspace(fixture.path(), "ipc-project");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("r"),
        &scope.workspace_identity,
        9,
        "token-9",
    )
    .expect("prepare session owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind session owner endpoint");
    let registry = WorkspaceDbRegistry::default();
    let inventory_request = request(
        &endpoint,
        "write-inventory",
        WorkspaceDbIpcOperation::UpsertProviderInventory {
            request: ProviderOwnerInventoryWrite {
                scope: scope.clone(),
                state: ProviderOwnerInventoryState::Exact,
                entries: vec![ProviderOwnerInventoryEntry {
                    owner_path: "src/lib.rs".to_owned(),
                    owner_content_digest: Some(format!("{:064x}", 23)),
                    state: ProviderOwnerInventoryEntryState::Indexed,
                }],
            },
        },
    );
    let (served, written) = tokio::join!(
        serve_one_workspace_db_session_request(&listener, &endpoint, &registry),
        call_workspace_db_owner(&endpoint, &inventory_request),
    );
    served.expect("serve inventory write");
    assert!(matches!(
        written.expect("inventory response").result,
        WorkspaceDbIpcResult::ProviderInventory { receipt }
            if receipt.upserted_entry_count == 1
    ));

    let probe_request = request(
        &endpoint,
        "probe-after-write",
        WorkspaceDbIpcOperation::ProbeProviderOwners {
            scope: scope.clone(),
            owners: vec![ProviderOwnerBatchProbeRequest {
                owner_path: "src/lib.rs".to_owned(),
                metadata: ProviderOwnerMetadata {
                    file_identity: "file-1".to_owned(),
                    size_bytes: 1,
                    modified_unix_nanos: 1,
                    change_time_unix_nanos: 1,
                },
            }],
        },
    );
    let (served, probed) = tokio::join!(
        serve_one_workspace_db_session_request(&listener, &endpoint, &registry),
        call_workspace_db_owner(&endpoint, &probe_request),
    );
    served.expect("serve owner probe");
    assert!(matches!(
        probed.expect("probe response").result,
        WorkspaceDbIpcResult::ProviderOwners { receipt }
            if receipt.results.len() == 1
                && receipt.results[0].probe.decision == ProviderOwnerDecision::New
    ));

    let finish_request = request(
        &endpoint,
        "finish-writes",
        WorkspaceDbIpcOperation::FinishWrites {
            scope,
            mode: WorkspaceDbWriteFinishMode::OwnerDurabilityBoundary,
        },
    );
    let (served, finished) = tokio::join!(
        serve_one_workspace_db_session_request(&listener, &endpoint, &registry),
        call_workspace_db_owner(&endpoint, &finish_request),
    );
    served.expect("serve durability boundary");
    assert!(matches!(
        finished.expect("durability response").result,
        WorkspaceDbIpcResult::WriteFinish { receipt } if receipt.cache_flush_count == 1
    ));
    assert!(project_root.exists());
}

fn request(
    endpoint: &WorkspaceDbOwnerEndpoint,
    request_id: &str,
    operation: WorkspaceDbIpcOperation,
) -> WorkspaceDbIpcRequest {
    WorkspaceDbIpcRequest {
        schema_id: "agent.semantic-protocols.workspace-db-owner-request.v1".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: endpoint.workspace_identity.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        request_id: request_id.to_owned(),
        operation,
    }
}
use agent_semantic_client_db::{
    ProviderIncrementalOwnerWrite, ProviderOwnerFingerprint, ProviderSelectorProjection,
    ProviderTreeSitterCaptureProjection, ProviderTreeSitterOwnerResult,
    ProviderTreeSitterOwnerResultState, ProviderTreeSitterQueryIdentity, WorkspaceDbIpcSession,
    serve_workspace_db_session_until_shutdown,
};
