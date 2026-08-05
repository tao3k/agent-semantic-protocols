use super::{RuntimeServerStopReceipt, finalize_stopped_runtime_server};
use agent_semantic_client_db::{RuntimeServerEndpoint, runtime_server_endpoint_path};

fn fixture_endpoint(state_home: &std::path::Path, owner_epoch: u64) -> RuntimeServerEndpoint {
    let runtime_root = state_home.join("runtime").join("server");
    RuntimeServerEndpoint {
        schema_id: "agent.semantic-protocols.runtime-server-endpoint.v1".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest:
            agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest(),
        owner_epoch,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_artifact_digest: "runtime-digest".to_owned(),
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "a".repeat(64)),
        binding_token: format!("binding-{owner_epoch}"),
        socket_path: runtime_root.join("control.sock").to_string_lossy().into_owned(),
        data_plane_socket_path: runtime_root.join("data.sock").to_string_lossy().into_owned(),
        workspace_store_path: runtime_root.join("workspaces").to_string_lossy().into_owned(),
        status_memory_path: runtime_root.join("status.memory").to_string_lossy().into_owned(),
    }
}

async fn publish_fixture(state_home: &std::path::Path, endpoint: &RuntimeServerEndpoint) {
    tokio::fs::create_dir_all(state_home.join("runtime").join("server"))
        .await
        .expect("create runtime root");
    tokio::fs::write(
        runtime_server_endpoint_path(state_home),
        serde_json::to_vec(endpoint).expect("encode endpoint"),
    )
    .await
    .expect("publish endpoint");
    for path in [
        &endpoint.socket_path,
        &endpoint.data_plane_socket_path,
        &endpoint.status_memory_path,
    ] {
        tokio::fs::write(path, b"fixture")
            .await
            .expect("publish owned artifact");
    }
}

fn assert_terminal(receipt: &RuntimeServerStopReceipt) {
    assert_eq!(receipt.state, "stopped");
    assert!(receipt.endpoint_removed);
    assert!(receipt.control_socket_removed);
    assert!(receipt.data_plane_socket_removed);
    assert!(receipt.status_memory_removed);
}

#[tokio::test]
async fn stop_finalization_retires_only_the_observed_owner_epoch() {
    let state_home = tempfile::tempdir().expect("state home");
    let endpoint = fixture_endpoint(state_home.path(), 7);
    publish_fixture(state_home.path(), &endpoint).await;
    let receipt =
        finalize_stopped_runtime_server(state_home.path(), Some(&endpoint), "stop-7".to_owned())
            .await
            .expect("finalize observed epoch");
    assert_terminal(&receipt);
}

#[tokio::test]
async fn repeated_stop_finalization_is_idempotent() {
    let state_home = tempfile::tempdir().expect("state home");
    let first = finalize_stopped_runtime_server(state_home.path(), None, "stop-1".to_owned())
        .await
        .expect("first finalization");
    let second = finalize_stopped_runtime_server(state_home.path(), None, "stop-2".to_owned())
        .await
        .expect("second finalization");
    assert_terminal(&first);
    assert_terminal(&second);
}

#[tokio::test]
async fn replacement_epoch_is_never_removed_by_a_stale_stop_owner() {
    let state_home = tempfile::tempdir().expect("state home");
    let stale = fixture_endpoint(state_home.path(), 3);
    let replacement = fixture_endpoint(state_home.path(), 4);
    publish_fixture(state_home.path(), &replacement).await;
    let error =
        finalize_stopped_runtime_server(state_home.path(), Some(&stale), "stale-stop".to_owned())
            .await
            .expect_err("stale stop owner must not retire replacement epoch");
    assert!(error.contains("terminal artifacts remain"));
    assert!(
        tokio::fs::try_exists(runtime_server_endpoint_path(state_home.path()))
            .await
            .expect("inspect replacement endpoint")
    );
}
