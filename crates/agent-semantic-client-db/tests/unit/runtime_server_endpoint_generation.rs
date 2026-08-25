use std::path::Path;

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use agent_semantic_client_db::runtime_server_control::{
    RuntimeServerEndpoint, runtime_server_transport_contract_digest,
};

struct TestListeners {
    _control: tokio::net::UnixListener,
    _data: tokio::net::UnixListener,
    provider: Option<tokio::net::UnixListener>,
    _client: tokio::net::TcpListener,
}

async fn endpoint_with_listeners(root: &Path) -> (RuntimeServerEndpoint, TestListeners) {
    let control_path = root.join("control.sock");
    let data_path = root.join("data.sock");
    let provider_path = root.join("provider.sock");
    let control = tokio::net::UnixListener::bind(&control_path).expect("bind control");
    let data = tokio::net::UnixListener::bind(&data_path).expect("bind data");
    let provider = tokio::net::UnixListener::bind(&provider_path).expect("bind provider");
    let client = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind client HTTP");
    let client_endpoint = format!(
        "http://{}",
        client.local_addr().expect("client HTTP address")
    );
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
        owner_epoch: 41,
        owner_process_id: std::process::id(),
        runtime_artifact_path: root.join("asp").display().to_string(),
        runtime_binary_identity: runtime_identity(),
        monitor_capability: true,
        observed_runtime_binary_identity: runtime_identity(),
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "1".repeat(64)),
        binding_token: "test-binding".to_owned(),
        socket_path: control_path.display().to_string(),
        data_plane_socket_path: data_path.display().to_string(),
        provider_plane_socket_path: provider_path.display().to_string(),
        client_http_endpoint: client_endpoint,
        workspace_store_path: root.join("workspaces").display().to_string(),
        status_memory_path: root.join("status.memory").display().to_string(),
    };
    (
        endpoint,
        TestListeners {
            _control: control,
            _data: data,
            provider: Some(provider),
            _client: client,
        },
    )
}

fn runtime_identity() -> RuntimeBinaryIdentity {
    RuntimeBinaryIdentity::Content {
        value: "2".repeat(64),
        algorithm: "blake3-256".to_owned(),
    }
}

#[tokio::test]
async fn healthy_generation_requires_every_published_listener() {
    let root = tempfile::tempdir().expect("tempdir");
    let (endpoint, _listeners) = endpoint_with_listeners(root.path()).await;
    endpoint
        .validate_service_reachability()
        .await
        .expect("one reachable endpoint generation");
}

#[tokio::test]
async fn stale_status_memory_cannot_hide_a_missing_provider_listener() {
    let root = tempfile::tempdir().expect("tempdir");
    let (endpoint, mut listeners) = endpoint_with_listeners(root.path()).await;
    drop(listeners.provider.take());
    let error = endpoint
        .validate_service_reachability()
        .await
        .expect_err("missing provider listener must fail closed");
    assert!(
        error.contains("provider listener is unreachable"),
        "unexpected error: {error}"
    );
}
