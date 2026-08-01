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
fn runtime_generation_admission_ensure_and_locator_repair_have_distinct_typed_wire_shapes() {
    let admit = serde_json::to_value(WorkspaceDbIpcOperation::AdmitRuntimeGeneration {
        project_root: "/workspace".to_owned(),
    })
    .expect("encode runtime generation admission");
    let ensure = serde_json::to_value(WorkspaceDbIpcOperation::EnsureRuntimeGeneration {
        project_root: "/workspace".to_owned(),
    })
    .expect("encode ensured runtime generation");
    let repair = serde_json::to_value(WorkspaceDbIpcOperation::RepairRuntimeGenerationLocator {
        project_root: "/workspace".to_owned(),
    })
    .expect("encode runtime generation locator repair");
    assert_eq!(
        admit,
        serde_json::json!({
            "kind": "admit-runtime-generation",
            "projectRoot": "/workspace"
        })
    );
    assert_eq!(
        ensure,
        serde_json::json!({
            "kind": "ensure-runtime-generation",
            "projectRoot": "/workspace"
        })
    );
    assert_eq!(
        repair,
        serde_json::json!({
            "kind": "repair-runtime-generation-locator",
            "projectRoot": "/workspace"
        })
    );
}

#[test]
fn runtime_owner_tombstone_has_one_typed_v1_wire_shape() {
    let value = serde_json::to_value(WorkspaceDbIpcOperation::TombstoneRuntimeOwner {
        project_root: "/workspace".to_owned(),
        owner_path: "src/previous.rs".to_owned(),
    })
    .expect("encode runtime owner tombstone");
    assert_eq!(
        value,
        serde_json::json!({
            "kind": "tombstone-runtime-owner",
            "projectRoot": "/workspace",
            "ownerPath": "src/previous.rs"
        })
    );
}

#[test]
fn runtime_owner_relocation_has_one_atomic_v1_wire_shape() {
    let value = serde_json::to_value(WorkspaceDbIpcOperation::RelocateRuntimeOwner {
        project_root: "/workspace".to_owned(),
        previous_owner_path: "src/previous.rs".to_owned(),
        owner: agent_semantic_client_db::runtime_server_workspace::WorkspaceOwnerSnapshot {
            owner_path: "src/current.rs".to_owned(),
            content_digest:
                "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            bytes: b"fn current() {}\n".to_vec(),
            selectors: Vec::new(),
        },
    })
    .expect("encode runtime owner relocation");
    assert_eq!(value["kind"], "relocate-runtime-owner");
    assert_eq!(value["projectRoot"], "/workspace");
    assert_eq!(value["previousOwnerPath"], "src/previous.rs");
    assert_eq!(value["owner"]["ownerPath"], "src/current.rs");
}

#[test]
fn runtime_owner_freshness_has_one_typed_v1_wire_shape() {
    let value = serde_json::to_value(WorkspaceDbIpcOperation::EnsureRuntimeOwner {
        project_root: "/workspace".to_owned(),
        language_id: "rust".to_owned(),
        owner_path: "src/lib.rs".to_owned(),
    })
    .expect("encode runtime owner freshness");
    assert_eq!(
        value,
        serde_json::json!({
            "kind": "ensure-runtime-owner",
            "projectRoot": "/workspace",
            "languageId": "rust",
            "ownerPath": "src/lib.rs"
        })
    );
}

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
    let endpoint = prepare_workspace_db_owner_endpoint(
        &runtime,
        "workspace-deep-state",
        7,
        7007,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-7",
        "token-7",
    )
    .expect("prepare short owner endpoint");
    assert!(!endpoint.socket_path.contains("deep-state-segment"));
    assert!(endpoint.socket_path.len() <= 103);
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let request = WorkspaceDbIpcRequest {
        schema_id: "agent.semantic-protocols.workspace-db-owner-request.v1".to_owned(),
        schema_version: "1".to_owned(),
        workspace_identity: endpoint.workspace_identity.clone(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
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
    assert_eq!(
        response.transport_contract_digest,
        endpoint.transport_contract_digest
    );
}

#[test]
fn owner_epochs_use_distinct_socket_paths_within_one_workspace() {
    let fixture = TempDir::new().expect("create owner epoch socket fixture");
    let runtime = fixture.path().join("runtime");
    let first = prepare_workspace_db_owner_endpoint(
        &runtime,
        "workspace-owner-epoch",
        17,
        7017,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-17",
        "token-17",
    )
    .expect("prepare first owner epoch");
    let second = prepare_workspace_db_owner_endpoint(
        &runtime,
        "workspace-owner-epoch",
        18,
        7018,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-18",
        "token-18",
    )
    .expect("prepare second owner epoch");

    assert_ne!(
        first.socket_path, second.socket_path,
        "a stale owner cleanup must never be able to unlink the current owner's listener pathname"
    );
    assert!(first.socket_path.len() <= 103);
    assert!(second.socket_path.len() <= 103);
}

#[tokio::test(flavor = "current_thread")]
async fn warm_workspace_owner_transport_is_sub_millisecond() {
    let _performance = crate::test_support::performance_lock();
    const SAMPLES: u64 = 32;
    let fixture = TempDir::new().expect("create workspace IPC performance fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("r"),
        "workspace-transport-performance",
        11,
        7011,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-11",
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
        7009,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-9",
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
            let error = read
                .expect("join rejected control-plane source-index read")
                .expect_err("control-plane source-index reads must fail closed");
            assert!(
                error.contains(
                    "source-index reads are only accepted by the Runtime Server data plane"
                ),
                "unexpected source-index control-plane rejection: {error}"
            );
        }
        let source_index_elapsed = source_index_started.elapsed();
        let average_nanos = source_index_elapsed.as_nanos() / (SOURCE_INDEX_SAMPLES as u128);
        assert!(
            average_nanos < 1_000_000,
            "warm rejected source-index control-plane IPC average must remain sub-millisecond: samples={} elapsedMicros={} averageNanos={}",
            SOURCE_INDEX_SAMPLES,
            source_index_elapsed.as_micros(),
            average_nanos,
        );
        eprintln!(
            "[workspace-db-source-index-control-plane-rejection-performance] samples={} elapsedMicros={} averageNanos={} budgetNanos=1000000",
            SOURCE_INDEX_SAMPLES,
            source_index_elapsed.as_micros(),
            average_nanos,
        );
        let owner_path = "src/lib.rs";
        let source_bytes = b"pub fn example() {}".to_vec();
        let owner_content_digest =
            agent_semantic_content_identity::ArtifactHash::blake3(source_bytes.as_slice()).value;
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
            size_bytes: source_bytes.len() as u64,
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
            source_bytes: source_bytes.clone(),
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
        let snapshot = session
            .read_provider_owner_snapshot(&scope, owner_path)
            .await
            .expect("read complete incremental owner through IPC session")
            .expect("incremental owner snapshot must exist");
        assert_eq!(snapshot.source_bytes, source_bytes);
        assert_eq!(snapshot.fingerprint.content_digest, owner_content_digest);
        assert_eq!(snapshot.projections.len(), 1);
        assert_eq!(snapshot.projections[0].item_name, "example");
        let (warm_probe, warm_snapshot) = session
            .read_provider_owner_warm(
                &scope,
                &ProviderOwnerBatchProbeRequest {
                    owner_path: owner_path.to_owned(),
                    metadata: metadata.clone(),
                },
            )
            .await
            .expect("read provider owner warm state through one IPC operation");
        assert_eq!(warm_probe.decision, ProviderOwnerDecision::Unchanged);
        assert_eq!(
            warm_snapshot
                .expect("warm provider owner snapshot")
                .source_bytes,
            source_bytes
        );

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

#[test]
fn workspace_owner_endpoint_rejects_transport_contract_drift() {
    let fixture = TempDir::new().expect("create endpoint identity fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("r"),
        "workspace-artifact-identity",
        8,
        7008,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-8",
        "token-8",
    )
    .expect("prepare workspace owner endpoint");

    endpoint
        .validate_for_workspace("workspace-artifact-identity")
        .expect("matching transport contract must validate");
    let mut stale_receipt =
        serde_json::to_value(&endpoint).expect("serialize typed owner endpoint");
    stale_receipt
        .as_object_mut()
        .expect("endpoint JSON object")
        .remove("transportContractDigest");
    let missing_digest_error = serde_json::from_value::<WorkspaceDbOwnerEndpoint>(stale_receipt)
        .expect_err("a pre-contract endpoint receipt must be stale");
    assert!(
        missing_digest_error
            .to_string()
            .contains("transportContractDigest"),
        "missing transport identity must be explicit: {missing_digest_error}"
    );

    let mut drifted_endpoint = endpoint;
    drifted_endpoint.transport_contract_digest = "blake3-256:stale-contract".to_owned();
    let error = drifted_endpoint
        .validate_for_workspace("workspace-artifact-identity")
        .expect_err("a stale transport contract must not reuse the resident endpoint");
    assert!(
        error.contains("transport contract mismatch"),
        "transport contract drift must remain an explicit endpoint identity failure: {error}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn workspace_owner_transport_handshake_rejects_request_contract_drift() {
    let fixture = TempDir::new().expect("create transport contract fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("runtime"),
        "workspace-contract-handshake",
        9,
        7009,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-9",
        "token-9",
    )
    .expect("prepare workspace owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let mut drifted_request = request(
        &endpoint,
        "request-contract-drift",
        WorkspaceDbIpcOperation::Health,
    );
    drifted_request.transport_contract_digest = "blake3-256:stale-contract".to_owned();

    let (served, response) = tokio::join!(
        serve_one_workspace_db_ipc_request(&listener, &endpoint),
        call_workspace_db_owner(&endpoint, &drifted_request),
    );

    served.expect("serve drifted transport request");
    let response = response.expect("receive typed rejection");
    assert_eq!(
        response.transport_contract_digest,
        endpoint.transport_contract_digest
    );
    assert!(
        matches!(
            response.result,
            WorkspaceDbIpcResult::Failed {
                ref code,
                ref message
            } if code == "workspace-owner-binding-mismatch"
                && message.contains("transport contract")
        ),
        "transport drift must fail closed through the resident handshake: {:?}",
        response.result
    );
}

#[tokio::test(flavor = "current_thread")]
async fn disconnected_client_does_not_terminate_the_resident_transport() {
    let fixture = TempDir::new().expect("create disconnected client fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("runtime"),
        "workspace-disconnected-client",
        12,
        7012,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-12",
        "token-12",
    )
    .expect("prepare workspace owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let registry = std::sync::Arc::new(WorkspaceDbRegistry::default());
    let last_activity = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let client = async {
        let disconnected = tokio::net::UnixStream::connect(&endpoint.socket_path)
            .await
            .expect("connect client that disconnects before a frame");
        drop(disconnected);
        tokio::task::yield_now().await;

        WorkspaceDbIpcSession::new(endpoint.clone())
            .health()
            .await
            .expect("resident transport must survive a disconnected client");
        shutdown_tx
            .send(true)
            .map_err(|_| "resident shutdown receiver disappeared".to_owned())
    };

    let (served, client_result) = tokio::join!(
        serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            registry,
            last_activity,
            shutdown_rx,
        ),
        client,
    );
    client_result.expect("complete disconnected client scenario");
    served.expect("resident transport survives the disconnected client");
}

#[tokio::test(flavor = "multi_thread")]
async fn typed_shutdown_retires_one_owner_across_concurrent_sessions() {
    let fixture = TempDir::new().expect("create typed shutdown fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("runtime"),
        "workspace-typed-shutdown",
        16,
        7016,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-16",
        "token-16",
    )
    .expect("prepare workspace owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let registry = std::sync::Arc::new(WorkspaceDbRegistry::default());
    let last_activity = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let clients = async {
        let mut probes = tokio::task::JoinSet::new();
        for _ in 0..32 {
            let session = WorkspaceDbIpcSession::new(endpoint.clone());
            probes.spawn(async move { session.health().await });
        }
        while let Some(probe) = probes.join_next().await {
            probe
                .expect("health probe task completes")
                .expect("health probe uses the current owner");
        }

        let started = std::time::Instant::now();
        tokio::time::timeout(
            std::time::Duration::from_millis(50),
            WorkspaceDbIpcSession::new(endpoint.clone()).shutdown(),
        )
        .await
        .expect("typed shutdown stays within the transport performance gate")
        .expect("typed shutdown is accepted by the bound owner");
        assert!(
            started.elapsed() < std::time::Duration::from_millis(50),
            "typed shutdown exceeded the transport performance gate: {:?}",
            started.elapsed()
        );
    };

    let (served, ()) = tokio::join!(
        serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            registry,
            last_activity,
            shutdown_rx,
        ),
        clients,
    );
    served.expect("typed shutdown retires the resident owner");
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_sessions_share_one_workspace_owner() {
    let fixture = TempDir::new().expect("create concurrent session fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("runtime"),
        "workspace-concurrent-sessions",
        13,
        7013,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-13",
        "token-13",
    )
    .expect("prepare workspace owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let registry = std::sync::Arc::new(WorkspaceDbRegistry::default());
    let last_activity = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let clients = async {
        let first = WorkspaceDbIpcSession::new(endpoint.clone());
        let second = WorkspaceDbIpcSession::new(endpoint.clone());
        let (first_health, second_health) = tokio::join!(first.health(), second.health());
        first_health.expect("first session uses the shared workspace owner");
        second_health.expect("second session uses the shared workspace owner");
        shutdown_tx
            .send(true)
            .map_err(|_| "resident shutdown receiver disappeared".to_owned())
    };

    let (served, client_result) = tokio::join!(
        serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            registry,
            last_activity,
            shutdown_rx,
        ),
        clients,
    );
    client_result.expect("complete concurrent session scenario");
    served.expect("one workspace owner serves concurrent sessions");
}

#[tokio::test(flavor = "multi_thread")]
async fn stalled_session_does_not_block_other_workspace_sessions() {
    let fixture = TempDir::new().expect("create stalled session fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("runtime"),
        "workspace-stalled-session",
        14,
        7014,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-14",
        "token-14",
    )
    .expect("prepare workspace owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let registry = std::sync::Arc::new(WorkspaceDbRegistry::default());
    let last_activity = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let clients = async {
        let _stalled = tokio::net::UnixStream::connect(&endpoint.socket_path)
            .await
            .expect("connect stalled workspace session");
        tokio::task::yield_now().await;

        let mut probes = tokio::task::JoinSet::new();
        for probe_id in 0..32 {
            let session = WorkspaceDbIpcSession::new(endpoint.clone());
            probes.spawn(async move {
                session
                    .health()
                    .await
                    .map_err(|error| format!("health probe {probe_id} failed: {error}"))
            });
        }
        while let Some(probe) = probes.join_next().await {
            probe.map_err(|error| format!("health probe task failed: {error}"))??;
        }
        shutdown_tx
            .send(true)
            .map_err(|_| "resident shutdown receiver disappeared".to_owned())
    };

    let (served, client_result) = tokio::join!(
        serve_workspace_db_session_until_shutdown(
            &listener,
            &endpoint,
            registry,
            last_activity,
            shutdown_rx,
        ),
        clients,
    );
    client_result.expect("complete stalled session scenario");
    served.expect("stalled session must not block the shared workspace owner");
}

#[tokio::test(flavor = "current_thread")]
async fn health_probe_is_bounded_when_owner_never_responds() {
    let fixture = TempDir::new().expect("create bounded health fixture");
    let endpoint = prepare_workspace_db_owner_endpoint(
        &fixture.path().join("runtime"),
        "workspace-bounded-health",
        15,
        7015,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-15",
        "token-15",
    )
    .expect("prepare workspace owner endpoint");
    let listener = bind_workspace_db_owner(&endpoint).expect("bind owner endpoint");
    let unresponsive_owner = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept health probe");
        std::future::pending::<()>().await;
    });

    let started = std::time::Instant::now();
    let error = WorkspaceDbIpcSession::new(endpoint)
        .health()
        .await
        .expect_err("unresponsive owner must time out");
    unresponsive_owner.abort();

    assert_eq!(error, "workspace resident DB service health exceeded 50ms");
    assert!(
        started.elapsed() < std::time::Duration::from_millis(150),
        "health timeout exceeded its bounded recovery window: {:?}",
        started.elapsed()
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
        7009,
        std::path::Path::new("/runtime/asp"),
        "runtime-digest-9",
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
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
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
