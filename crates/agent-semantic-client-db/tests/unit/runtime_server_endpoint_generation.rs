use std::path::Path;

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use agent_semantic_client_db::runtime_server_control::RuntimeServerEndpoint;
use agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest;

struct TestListeners {
    _control: tokio::net::TcpListener,
    _data: tokio::net::TcpListener,
    provider: Option<tokio::net::TcpListener>,
}

async fn endpoint_with_listeners(root: &Path) -> (RuntimeServerEndpoint, TestListeners) {
    let control = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind control");
    let data = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind data");
    let provider = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind provider");
    let control_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(control.local_addr().expect("control address")).expect("control endpoint");
    let data_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(data.local_addr().expect("data address")).expect("data endpoint");
    let provider_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(provider.local_addr().expect("provider address")).expect("provider endpoint");
    let runtime_binary_identity = runtime_identity();
    let binary_content_digest = match &runtime_binary_identity {
        RuntimeBinaryIdentity::Content { digest } => digest.to_string(),
    };
    let endpoint = RuntimeServerEndpoint {
        binary_content_digest,
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
        runtime_binary_identity: runtime_binary_identity.clone(),
        monitor_capability: true,
        observed_runtime_binary_identity: runtime_binary_identity,
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "1".repeat(64)),
        binding_token: "test-binding".to_owned(),
        control_endpoint,
        data_endpoint,
        provider_endpoint,
        workspace_store_path: root.join("workspaces").display().to_string(),
        status_memory_path: root.join("status.memory").display().to_string(),
    };
    (
        endpoint,
        TestListeners {
            _control: control,
            _data: data,
            provider: Some(provider),
        },
    )
}

fn runtime_identity() -> RuntimeBinaryIdentity {
    RuntimeBinaryIdentity::from_bytes(b"runtime-endpoint-generation")
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
