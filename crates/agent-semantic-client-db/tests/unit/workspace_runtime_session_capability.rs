// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind;
use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorOverlay;
use agent_semantic_client_db::workspace_db_ipc::WorkspaceDbIpcSession;

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
        transport_binding: agent_semantic_client_db::runtime_server_control::RuntimeTransportBinding {
            transport_contract_digest: "fixture-transport-digest".to_owned(),
            owner_epoch: 1,
            owner_process_id: 1,
            binding_token: "fixture-binding".to_owned(),
            control_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 46001).into()).expect("control endpoint"),
            data_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 46002).into()).expect("data endpoint"),
            provider_endpoint: agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(([127, 0, 0, 1], 46003).into()).expect("provider endpoint"),
        },
        runtime_artifact_path: "/tmp/asp".to_owned(),
        runtime_binary_identity:
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                b"fixture-runtime-digest",
            ),
        monitor_capability: true,
        observed_runtime_binary_identity:
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                b"fixture-runtime-digest",
            ),
        artifact_mode: "fixture".to_owned(),
        artifact_catalog_digest: "fixture-catalog-digest".to_owned(),
        workspace_store_path: "/tmp/unused-workspace-store".to_owned(),
        status_memory_path: "/tmp/unused-status-memory".to_owned(),
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
