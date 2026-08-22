use agent_semantic_client_db::RuntimeServerEndpoint;
use agent_semantic_client_db::runtime_server_lifecycle_coordinator::{
    OwnerClassification, RuntimeServerLifecycleCoordinator,
};
use agent_semantic_client_db::{RuntimeServerExitReceipt, RuntimeServerSpawnReceipt};
use agent_semantic_runtime::runtime_artifact_catalog::RuntimeBinaryIdentity;
use tempfile::tempdir;

#[tokio::test]
async fn missing_owner_is_classified_without_process_probe() {
    let coordinator =
        RuntimeServerLifecycleCoordinator::new("/tmp/asp-test-state", "/tmp/asp-test-bin");
    assert_eq!(
        coordinator.classify(None).await.unwrap(),
        OwnerClassification::Missing
    );
}

#[tokio::test]
async fn stale_pid_is_classified_without_signal() {
    let coordinator =
        RuntimeServerLifecycleCoordinator::new("/tmp/asp-test-state", "/tmp/asp-test-bin");
    assert_eq!(
        coordinator.classify(Some(u32::MAX)).await.unwrap(),
        OwnerClassification::Stale
    );
}

#[tokio::test]
async fn wrong_executable_is_rejected_before_termination() {
    let coordinator = RuntimeServerLifecycleCoordinator::new(
        "/tmp/asp-test-state",
        "/definitely/not/the/current/executable",
    );
    let result = coordinator
        .terminate_verified(std::process::id(), false)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn owner_receipt_roundtrip_is_atomic_and_schema_stable() {
    let dir = tempdir().unwrap();
    let receipt = RuntimeServerSpawnReceipt {
        schema_id: "owner".into(),
        schema_version: "1".into(),
        process_id: 7,
        nonce: "n".into(),
        state_home: dir.path().display().to_string(),
        runtime_artifact_path: "/bin/asp".into(),
    };
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(dir.path(), &receipt)
        .await
        .unwrap();
    assert_eq!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(dir.path())
            .await
            .unwrap()
            .unwrap()
            .nonce,
        "n"
    );
    agent_semantic_client_db::runtime_server_lifecycle::remove_owner_receipt(dir.path())
        .await
        .unwrap();
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(dir.path())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn exit_receipt_roundtrip_preserves_terminal_state() {
    let dir = tempdir().unwrap();
    agent_semantic_client_db::runtime_server_lifecycle::publish_with_errors(
        dir.path(),
        9,
        true,
        vec!["ok".into()],
    )
    .await
    .unwrap();
    let receipt: RuntimeServerExitReceipt =
        agent_semantic_client_db::runtime_server_lifecycle::read_latest_owner_exit(dir.path())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(receipt.owner_epoch, 9);
    assert!(receipt.clean_drain);
}

fn fixture_endpoint(state_home: &std::path::Path, owner_epoch: u64) -> RuntimeServerEndpoint {
    let runtime_root = agent_semantic_client_db::runtime_server_runtime_base(state_home).unwrap();
    let binding_token = format!("binding-{owner_epoch}");
    let identity_value = "runtime-digest";
    let digest =
        blake3::hash(format!("{owner_epoch}\0{binding_token}\0{identity_value}").as_bytes())
            .to_hex();
    RuntimeServerEndpoint {
        schema_id: "agent.semantic-protocols.runtime-server-endpoint.v1".to_owned(),
        schema_version: "1".to_owned(),
        transport_contract_digest:
            agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest(),
        owner_epoch,
        owner_process_id: 0,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_binary_identity: RuntimeBinaryIdentity::DeveloperSourceGeneration {
            value: identity_value.to_owned(),
            algorithm: "blake3-metadata-v1".to_owned(),
        },
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::DeveloperSourceGeneration {
            value: identity_value.to_owned(),
            algorithm: "blake3-metadata-v1".to_owned(),
        },
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "a".repeat(64)),
        binding_token,
        socket_path: runtime_root.join(format!("r-{}.sock", &digest[..16])).display().to_string(),
        data_plane_socket_path: runtime_root.join(format!("r-{}.data.sock", &digest[..16])).display().to_string(),
        provider_plane_socket_path: runtime_root
            .join(format!("r-{}.providers.sock", &digest[..16]))
            .display()
            .to_string(),
        workspace_store_path: runtime_root.join("workspaces").display().to_string(),
        status_memory_path: runtime_root.join("status.v1.memory").display().to_string(),
    }
}

async fn publish_endpoint_fixture(state_home: &std::path::Path, endpoint: &RuntimeServerEndpoint) {
    let endpoint_path = agent_semantic_client_db::runtime_server_endpoint_path(state_home).unwrap();
    agent_semantic_client_db::runtime_server_control::publish_runtime_server_endpoint(
        &endpoint_path,
        endpoint,
    )
    .await
    .unwrap();
    for path in [
        &endpoint.socket_path,
        &endpoint.data_plane_socket_path,
        &endpoint.status_memory_path,
    ] {
        tokio::fs::write(path, b"fixture").await.unwrap();
    }
}

#[tokio::test]
async fn endpoint_cleanup_retires_only_the_observed_owner_epoch() {
    let state_home = tempdir().unwrap();
    let endpoint = fixture_endpoint(state_home.path(), 7);
    publish_endpoint_fixture(state_home.path(), &endpoint).await;
    agent_semantic_client_db::runtime_server_control::cleanup_runtime_server_endpoint(
        state_home.path(),
        &endpoint,
    )
    .await
    .unwrap();
    assert!(
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            state_home.path(),
        )
        .await
        .unwrap()
        .is_none()
    );
}

#[tokio::test]
async fn endpoint_cleanup_is_idempotent_after_terminal_removal() {
    let state_home = tempdir().unwrap();
    let endpoint = fixture_endpoint(state_home.path(), 8);
    agent_semantic_client_db::runtime_server_control::cleanup_runtime_server_endpoint(
        state_home.path(),
        &endpoint,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn replacement_epoch_is_never_removed_by_a_stale_stop_owner() {
    let state_home = tempdir().unwrap();
    let stale = fixture_endpoint(state_home.path(), 3);
    let replacement = fixture_endpoint(state_home.path(), 4);
    publish_endpoint_fixture(state_home.path(), &replacement).await;
    assert!(
        agent_semantic_client_db::runtime_server_control::cleanup_runtime_server_endpoint(
            state_home.path(),
            &stale,
        )
        .await
        .is_err()
    );
    let actual =
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            state_home.path(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(actual.owner_epoch, replacement.owner_epoch);
    agent_semantic_client_db::runtime_server_control::cleanup_runtime_server_endpoint(
        state_home.path(),
        &replacement,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn live_owner_receipt_is_the_supervisor_admission_authority() {
    let state_home = tempdir().unwrap();
    let executable = std::env::current_exe().unwrap();
    let process_id = agent_semantic_runtime::runtime_process_lifecycle::current_process_id();
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &RuntimeServerSpawnReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
            schema_version: "1".to_owned(),
            process_id,
            nonce: "live-owner".to_owned(),
            state_home: state_home.path().display().to_string(),
            runtime_artifact_path: executable.display().to_string(),
        },
    )
    .await
    .unwrap();
    let request = agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
        state_home: state_home.path().to_path_buf(),
        expected_executable: executable,
        launch: agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec {
            program: "/must/not/spawn".into(),
            args: Vec::new(),
            current_dir: None,
            environment: Vec::new(),
            stderr: state_home.path().join("stderr"),
        },
    };
    assert_eq!(
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .classify_owner(&request)
            .await
            .unwrap(),
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::AlreadyResident
    );
}
