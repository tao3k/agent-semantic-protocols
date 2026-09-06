// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use super::{
    RuntimeServerControlRequest, RuntimeServerEndpoint, discard_closed_or_dirty_control_stream,
    exchange_runtime_server_request_with_budget,
};
use crate::runtime_server_control::{
    RuntimeServerOperation, runtime_server_transport_contract_digest,
};
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;

#[tokio::test]
async fn pooled_lane_discards_a_closed_previous_generation_before_write() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind stale endpoint");
    let address = listener.local_addr().expect("stale endpoint address");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept stale lane");
        drop(stream);
    });
    let stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connect stale lane");
    server.await.expect("close prior generation lane");
    let mut stream = Some(stream);
    for _ in 0..20 {
        discard_closed_or_dirty_control_stream(&mut stream);
        if stream.is_none() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(stream.is_none(), "closed generation lane must be discarded");
}

#[tokio::test]
async fn stalled_control_exchange_is_bounded_and_discards_the_lane() {
    let temporary = tempfile::tempdir().expect("temporary control exchange root");
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind stalled control endpoint");
    let control_address = listener.local_addr().expect("control endpoint address");
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept control lane");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    });
    let endpoint = RuntimeServerEndpoint {
        binary_content_digest:
            "blake3-256:0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        runtime_generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        schema_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest: runtime_server_transport_contract_digest(),
        owner_epoch: 1,
        owner_process_id: 0,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(b"runtime-digest"),
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(b"runtime-digest"),
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "0".repeat(64)),
        binding_token: "binding".to_owned(),
        control_endpoint:
            crate::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
                control_address,
            )
            .expect("control endpoint"),
        data_endpoint:
            crate::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
                ([127, 0, 0, 1], 41012).into(),
            )
            .expect("data endpoint"),
        provider_endpoint:
            crate::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
                ([127, 0, 0, 1], 41013).into(),
            )
            .expect("provider endpoint"),
        workspace_store_path: temporary.path().join("workspace").display().to_string(),
        status_memory_path: temporary.path().join("status").display().to_string(),
    };
    let request = RuntimeServerControlRequest {
        schema_id: "agent.semantic-protocols.runtime-server-control-request".to_owned(),
        schema_version: "1".to_owned(),
        operation: RuntimeServerOperation::Restart,
        expected_runtime_binary_identity:
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                b"next-runtime-digest",
            ),
        request_id: "bounded-control-exchange".to_owned(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        project_root: None,
    };
    let mut stream = None;
    let error = exchange_runtime_server_request_with_budget(
        &endpoint,
        &mut stream,
        &request,
        std::time::Duration::from_millis(10),
    )
    .await
    .expect_err("stalled exchange must time out");
    assert!(error.contains("runtime-server-control-exchange-budget-exceeded"));
    assert!(stream.is_none(), "timed out lane must be discarded");
    server.abort();
}
