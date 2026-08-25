use agent_semantic_client_db::{
    runtime_server_control::RuntimeServerEndpoint,
    runtime_server_workspace::{ExactProjectionKind, WorkspaceRuntimeSelectorOverlay},
    workspace_db_ipc::WorkspaceDbIpcSession,
};

#[tokio::test]
async fn read_only_runtime_session_rejects_selector_mutation_before_io() {
    let endpoint = RuntimeServerEndpoint {
        binary_content_digest:
            "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        runtime_generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        schema_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
        schema_version: "1".to_owned(),
        owner_epoch: 1,
        owner_process_id: 0,
        binding_token: "fixture-binding".to_owned(),
        socket_path: "/tmp/unused-control.sock".to_owned(),
        data_plane_socket_path: "/tmp/unused-data.sock".to_owned(),
        provider_plane_socket_path: "/tmp/unused-providers.sock".to_owned(),
        client_http_endpoint: "http://127.0.0.1:1".to_owned(),
        runtime_artifact_path: "/tmp/asp".to_owned(),
        runtime_binary_identity:
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
                value: "fixture-runtime-digest".to_owned(),
                algorithm: "blake3-256".to_owned(),
            },
        monitor_capability: true,
        observed_runtime_binary_identity:
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
                value: "fixture-runtime-digest".to_owned(),
                algorithm: "blake3-256".to_owned(),
            },
        artifact_mode: "fixture".to_owned(),
        artifact_catalog_digest: "fixture-catalog-digest".to_owned(),
        workspace_store_path: "/tmp/unused-workspace-store".to_owned(),
        status_memory_path: "/tmp/unused-status-memory".to_owned(),
        transport_contract_digest: "fixture-transport-digest".to_owned(),
    };
    let session = WorkspaceDbIpcSession::for_runtime_server_read_only(
        &endpoint,
        "fixture-workspace",
        "/tmp/fixture-project".into(),
    );
    let error = session
        .publish_runtime_selector_overlay(WorkspaceRuntimeSelectorOverlay {
            projection_kind: ExactProjectionKind::Source,
            structural_selector: "rust://src/lib.rs#item/function/example".to_owned(),
            owner_path: "src/lib.rs".to_owned(),
            owner_content_digest: "fixture-owner-digest".to_owned(),
            byte_start: 0,
            byte_end: 0,
            projection_bytes: Vec::new(),
        })
        .await
        .expect_err("read-only Runtime session must reject mutation before transport I/O");

    assert_eq!(
        error,
        "runtime-client-capability-denied: read-only session cannot submit Runtime mutations"
    );
}
