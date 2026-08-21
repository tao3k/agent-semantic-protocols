use super::{
    RuntimeServerControlRequest, RuntimeServerEndpoint, UnixStream,
    discard_closed_or_dirty_control_stream, exchange_runtime_server_request_with_budget,
};
use crate::runtime_server_control::{
    RuntimeServerOperation, runtime_server_transport_contract_digest,
};
use agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity;

#[tokio::test]
async fn pooled_lane_discards_a_closed_previous_generation_before_write() {
    let temporary = tempfile::tempdir().expect("temporary stale lane root");
    let socket_path = temporary.path().join("stale.sock");
    let listener = tokio::net::UnixListener::bind(&socket_path).expect("bind stale endpoint");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept stale lane");
        drop(stream);
    });
    let stream = UnixStream::connect(&socket_path)
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
    let socket_path = temporary.path().join("control.sock");
    let listener =
        tokio::net::UnixListener::bind(&socket_path).expect("bind stalled control endpoint");
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("accept control lane");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    });
    let endpoint = RuntimeServerEndpoint {
        schema_id: "agent.semantic-protocols.runtime-server-endpoint.v1".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest: runtime_server_transport_contract_digest(),
        owner_epoch: 1,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_binary_identity: RuntimeBinaryIdentity::Content { value: "runtime-digest".to_owned(), algorithm: "blake3-256".to_owned() },
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::Content { value: "runtime-digest".to_owned(), algorithm: "blake3-256".to_owned() },
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "0".repeat(64)),
        binding_token: "binding".to_owned(),
        socket_path: socket_path.display().to_string(),
        data_plane_socket_path: temporary.path().join("data.sock").display().to_string(),
        workspace_store_path: temporary.path().join("workspace").display().to_string(),
        status_memory_path: temporary.path().join("status").display().to_string(),
    };
    let request = RuntimeServerControlRequest {
        schema_id: "agent.semantic-protocols.runtime-server-control-request.v1".to_owned(),
        schema_version: "1".to_owned(),
        operation: RuntimeServerOperation::Restart,
        expected_runtime_binary_identity: agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity::Content { value: "next-runtime-digest".to_owned(), algorithm: "blake3-256".to_owned() },
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
