use agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint_in;

fn supervisor_request(
    state_home: &std::path::Path,
    executable: std::path::PathBuf,
) -> agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
    let artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&executable).unwrap(),
        );
    agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
        state_home: state_home.to_path_buf(),
        expected_executable: executable.clone(),
        activation_generation: 1,
        artifact_digest,
        previous_artifact_digest: None,
        previous_owner_epoch: None,
        launch: agent_semantic_runtime::runtime_process_lifecycle::RuntimeProcessLaunchSpec {
            program: executable,
            args: vec!["--help".to_owned()],
            current_dir: None,
            environment: Vec::new(),
            stderr: state_home.join("runtime-server.stderr"),
        },
    }
}

#[tokio::test]
async fn dead_owner_legacy_endpoint_is_republished_by_current_writer() {
    let state_home = tempfile::tempdir().expect("create isolated State Home");
    let endpoint = prepare_runtime_server_endpoint_in(
        state_home.path(),
        &state_home.path().join("asp"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"legacy-runtime",
        ),
        "dev",
        &format!("blake3-256:{}", "1".repeat(64)),
        1,
        "legacy-binding",
    )
    .await
    .expect("prepare endpoint");
    let mut legacy = serde_json::to_value(&endpoint).expect("encode endpoint");
    let malicious_path = state_home
        .path()
        .parent()
        .unwrap()
        .join("must-not-delete.sock");
    tokio::fs::write(&malicious_path, b"sentinel")
        .await
        .unwrap();
    let legacy_object = legacy.as_object_mut().expect("endpoint object");
    legacy_object.remove("providerPlaneSocketPath");
    legacy_object.insert(
        "socketPath".into(),
        serde_json::Value::String(malicious_path.display().to_string()),
    );
    let endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(state_home.path()).unwrap();
    tokio::fs::create_dir_all(endpoint_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&endpoint_path, serde_json::to_vec(&legacy).unwrap())
        .await
        .unwrap();
    let _outcome = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .ensure_runtime_server(
            supervisor_request(state_home.path(), std::env::current_exe().unwrap()),
            false,
        )
        .await;
    assert!(
        !tokio::fs::try_exists(&endpoint_path).await.unwrap(),
        "stale legacy receipt must be removed before current writer admission"
    );
    assert!(tokio::fs::try_exists(&malicious_path).await.unwrap());
    tokio::fs::remove_file(malicious_path).await.unwrap();
}

#[tokio::test]
async fn live_owner_legacy_endpoint_fails_closed() {
    let state_home = tempfile::tempdir().expect("create isolated State Home");
    let endpoint = prepare_runtime_server_endpoint_in(
        state_home.path(),
        &state_home.path().join("asp"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"legacy-runtime",
        ),
        "dev",
        &format!("blake3-256:{}", "1".repeat(64)),
        1,
        "legacy-binding",
    )
    .await
    .expect("prepare endpoint");
    let mut legacy = serde_json::to_value(&endpoint).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("providerPlaneSocketPath");
    let endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(state_home.path()).unwrap();
    tokio::fs::create_dir_all(endpoint_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&endpoint_path, serde_json::to_vec(&legacy).unwrap())
        .await
        .unwrap();
    let process_id = agent_semantic_runtime::runtime_process_lifecycle::current_process_id();
    let executable = std::env::current_exe().unwrap();
    let executable_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&executable).unwrap(),
        );
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &agent_semantic_client_db::RuntimeServerSpawnReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".into(),
            schema_version: "1".into(),
            process_id,
            nonce: "live-legacy".into(),
            state_home: state_home.path().display().to_string(),
            activation_generation: 1,
            launcher_artifact_path: executable.display().to_string(),
            launcher_artifact_digest: executable_digest,
            spawn_argv: vec![executable.display().to_string()],
            previous_serving_digest: None,
            previous_owner_epoch: None,
        },
    )
    .await
    .unwrap();
    let error = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .ensure_runtime_server(
            supervisor_request(state_home.path(), std::env::current_exe().unwrap()),
            false,
        )
        .await
        .expect_err("live owner must reject malformed endpoint");
    assert_eq!(
        error,
        "undecodable Runtime Server endpoint and owner receipt identities differ; refusing unbound termination: failed to decode Runtime Server endpoint: missing field `providerPlaneSocketPath`"
    );
}

#[tokio::test]
async fn endpoint_without_owner_receipt_is_stale_and_republished() {
    let state_home = tempfile::tempdir().expect("create isolated State Home");
    let executable = std::env::current_exe().unwrap();
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        &std::fs::read(&executable).unwrap(),
    );
    let endpoint = prepare_runtime_server_endpoint_in(
        state_home.path(),
        &executable,
        &digest,
        "dev",
        &format!("blake3-256:{}", "1".repeat(64)),
        1,
        "stale-without-owner",
    )
    .await
    .expect("prepare endpoint");
    let endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(state_home.path()).unwrap();
    tokio::fs::create_dir_all(endpoint_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&endpoint_path, serde_json::to_vec(&endpoint).unwrap())
        .await
        .unwrap();

    let result = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .ensure_runtime_server(supervisor_request(state_home.path(), executable), false)
        .await
        .expect("stale endpoint must be recoverable");
    assert_eq!(
        result,
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::SpawnAccepted
    );
}

#[tokio::test]
async fn cached_healthy_endpoint_with_dead_owner_is_republished() {
    let state_home = tempfile::tempdir().expect("create isolated State Home");
    let executable = std::env::current_exe().expect("resolve fixture executable");
    let artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&executable).expect("read fixture executable"),
        );
    let mut endpoint = prepare_runtime_server_endpoint_in(
        state_home.path(),
        &executable,
        &artifact_digest,
        "dev",
        &format!("blake3-256:{}", "1".repeat(64)),
        1,
        "cached-healthy-dead-owner",
    )
    .await
    .expect("prepare cached healthy endpoint");
    let dead_process_id = u32::MAX;
    endpoint.owner_process_id = dead_process_id;
    let endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(state_home.path()).unwrap();
    tokio::fs::create_dir_all(endpoint_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&endpoint_path, serde_json::to_vec(&endpoint).unwrap())
        .await
        .unwrap();
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &agent_semantic_client_db::RuntimeServerSpawnReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".into(),
            schema_version: "1".into(),
            process_id: dead_process_id,
            nonce: "cached-healthy-dead-owner".into(),
            state_home: state_home.path().display().to_string(),
            activation_generation: 1,
            launcher_artifact_path: executable.display().to_string(),
            launcher_artifact_digest: artifact_digest,
            spawn_argv: vec![executable.display().to_string()],
            previous_serving_digest: None,
            previous_owner_epoch: None,
        },
    )
    .await
    .expect("publish dead owner receipt");

    let outcome = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .ensure_runtime_server(supervisor_request(state_home.path(), executable), false)
        .await
        .expect("dead cached owner must be recoverable");
    assert_eq!(
        outcome,
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::SpawnAccepted
    );
}
