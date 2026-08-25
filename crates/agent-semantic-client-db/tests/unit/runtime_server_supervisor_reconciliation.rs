use std::{sync::Arc, time::Duration};

use agent_semantic_client_db::{
    WorkspaceDbRegistry,
    runtime_server::{RuntimeServer, RuntimeServerExit},
    runtime_server_control::{
        RuntimeServerOperation, RuntimeServerState, call_runtime_server,
        prepare_runtime_server_endpoint_in,
    },
};

async fn fixture_catalog()
-> agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog {
    let state_home = agent_semantic_runtime::resolve_state_home().expect("resolve State Home");
    agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_catalog(&state_home)
        .await
        .expect("load runtime artifact catalog")
}

fn supervisor_request(
    state_home: &std::path::Path,
    executable: std::path::PathBuf,
) -> agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
    agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
        state_home: state_home.to_path_buf(),
        expected_executable: executable.clone(),
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
        "blake3-256:legacy-runtime",
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
        "blake3-256:legacy-runtime",
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
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &agent_semantic_client_db::RuntimeServerSpawnReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".into(),
            schema_version: "1".into(),
            process_id,
            nonce: "live-legacy".into(),
            state_home: state_home.path().display().to_string(),
            runtime_artifact_path: std::env::current_exe().unwrap().display().to_string(),
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
    assert!(error.contains("owner is live"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_generation_handoff_publishes_the_next_healthy_endpoint() {
    let _performance = crate::test_support::performance_lock();
    const COMPLETE_HANDOFF_BOUNDARY: Duration = Duration::from_millis(500);
    let runtime_dir = tempfile::tempdir().expect("create isolated Runtime Server directory");
    let catalog = Arc::new(fixture_catalog().await);
    let registry = Arc::new(WorkspaceDbRegistry::default());
    let first_endpoint = prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        &runtime_dir.path().join("asp-generation-one"),
        "blake3-256:generation-one",
        catalog.mode_label(),
        &catalog.digest(),
        51,
        "generation-one-binding",
    )
    .await
    .expect("prepare first Runtime Server endpoint");
    let first_server = RuntimeServer::bind_with_catalog(
        first_endpoint.clone(),
        Arc::clone(&registry),
        Arc::clone(&catalog),
    )
    .await
    .expect("bind first Runtime Server generation");
    let first_server = tokio::spawn(first_server.serve());

    let second_endpoint = prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        &runtime_dir.path().join("asp-generation-two"),
        "blake3-256:generation-two",
        catalog.mode_label(),
        &catalog.digest(),
        52,
        "generation-two-binding",
    )
    .await
    .expect("pre-stage second Runtime Server endpoint");

    let handoff_started = tokio::time::Instant::now();
    let draining = call_runtime_server(
        &first_endpoint,
        RuntimeServerOperation::Restart,
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::Content {
            value: "blake3-256:generation-two".to_owned(),
            algorithm: "blake3-256".to_owned(),
        },
        "generation-handoff".to_owned(),
    )
    .await
    .expect("request generation handoff");
    assert_eq!(draining.state, RuntimeServerState::Draining);
    assert_eq!(
        first_server
            .await
            .expect("join first Runtime Server")
            .expect("serve first generation"),
        RuntimeServerExit::RestartRequested
    );

    let second_server =
        RuntimeServer::bind_with_catalog(second_endpoint.clone(), registry, catalog)
            .await
            .expect("bind second Runtime Server generation");
    let second_server = tokio::spawn(second_server.serve());
    let healthy = call_runtime_server(
        &second_endpoint,
        RuntimeServerOperation::Status,
        second_endpoint.runtime_binary_identity.clone(),
        "generation-two-health".to_owned(),
    )
    .await
    .expect("observe second Runtime Server generation");
    assert_eq!(healthy.state, RuntimeServerState::Healthy);
    assert_eq!(
        healthy.runtime_binary_identity.value(),
        "blake3-256:generation-two"
    );
    assert!(
        handoff_started.elapsed() < COMPLETE_HANDOFF_BOUNDARY,
        "complete in-process Tokio generation handoff exceeded the 500ms lifecycle boundary: {:?}",
        handoff_started.elapsed()
    );

    second_server.abort();
    let _ = second_server.await;
}
