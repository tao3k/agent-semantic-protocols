// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;
use agent_semantic_client_db::RuntimeServerEndpoint;
use agent_semantic_client_db::RuntimeServerExitReceipt;
use agent_semantic_client_db::RuntimeServerSpawnReceipt;
use agent_semantic_client_db::runtime_server_lifecycle_coordinator::OwnerClassification;
use agent_semantic_client_db::runtime_server_lifecycle_coordinator::RuntimeServerLifecycleCoordinator;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

fn activation_request(
    state_home: &std::path::Path,
    executable: std::path::PathBuf,
    publication_nonce: &str,
) -> agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
    let artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&executable).unwrap(),
        );
    agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest::for_activation(
        state_home.to_path_buf(),
        executable.clone(),
        publication_nonce.to_owned(),
        artifact_digest,
        None,
        executable,
        vec!["server".to_owned(), "daemon".to_owned()],
        None,
        Vec::new(),
        state_home.join("stderr"),
    )
}

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
        schema_id: agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID.into(),
        schema_version: "1".into(),
        process_id: 7,
        nonce: "n".into(),
        state_home: dir.path().display().to_string(),
        publication_nonce: "publication-owner-roundtrip".to_owned(),
        launcher_artifact_path: "/bin/asp".into(),
        launcher_artifact_digest:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                b"owner receipt",
            ),
        spawn_argv: vec!["server".into(), "daemon".into()],
        previous_serving_digest: None,
        previous_owner_epoch: None,
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
async fn legacy_v1_owner_receipt_is_typed_stale_and_not_current() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "1",
            "processId": 7,
            "nonce": "legacy",
            "stateHome": dir.path(),
            "runtimeArtifactPath": dir.path().join("runtime/bin/asp"),
        }))
        .unwrap(),
    )
    .unwrap();
    let state =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt_state(dir.path())
            .await
            .unwrap()
            .unwrap();
    assert!(matches!(
        state,
        agent_semantic_client_db::RuntimeServerSpawnReceiptRead::Stale(_)
    ));
    let dependent_error =
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(dir.path())
            .await
            .unwrap_err();
    assert!(dependent_error.contains("runtime-authority-stale"));
}

#[tokio::test]
async fn validated_activation_classifies_legacy_owner_as_not_current() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "1",
            "processId": 7,
            "nonce": "legacy",
            "stateHome": dir.path(),
            "runtimeArtifactPath": dir.path().join("runtime/bin/asp"),
        }))
        .unwrap(),
    )
    .unwrap();
    let request = activation_request(
        dir.path(),
        std::env::current_exe().unwrap(),
        "publication-missing-owner",
    );
    assert_eq!(
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .classify_owner(&request)
            .await
            .unwrap(),
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::OwnerStale
    );
}

#[tokio::test]
async fn malformed_owner_cannot_be_hidden_by_a_validated_activation() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{").unwrap();
    let request = activation_request(
        dir.path(),
        std::env::current_exe().unwrap(),
        "publication-malformed-owner",
    );
    let error = agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
        .classify_owner(&request)
        .await
        .unwrap_err();
    assert!(error.contains("runtime-server-owner-spawn-malformed"));
}

#[tokio::test]
async fn current_owner_and_pending_activation_use_one_nonce_digest_comparator() {
    let dir = tempdir().unwrap();
    let executable = std::env::current_exe().unwrap();
    let request = activation_request(dir.path(), executable.clone(), "publication-current");
    let mut receipt = RuntimeServerSpawnReceipt {
        schema_id: agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID.into(),
        schema_version: "1".into(),
        process_id: agent_semantic_runtime::runtime_process_lifecycle::current_process_id(),
        nonce: "current-owner".into(),
        state_home: dir.path().display().to_string(),
        publication_nonce: "publication-stale".to_owned(),
        launcher_artifact_path: executable.display().to_string(),
        launcher_artifact_digest: request.artifact_digest.clone(),
        spawn_argv: vec!["server".into(), "daemon".into()],
        previous_serving_digest: None,
        previous_owner_epoch: None,
    };
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(dir.path(), &receipt)
        .await
        .unwrap();
    assert_eq!(
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .classify_owner(&request)
            .await
            .unwrap(),
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::OwnerStale
    );

    receipt.publication_nonce = "publication-other".to_owned();
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(dir.path(), &receipt)
        .await
        .unwrap();
    assert_eq!(
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .classify_owner(&request)
            .await
            .unwrap(),
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::OwnerStale
    );

    receipt.publication_nonce = request.publication_nonce.clone();
    receipt.launcher_artifact_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"different artifact",
        );
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(dir.path(), &receipt)
        .await
        .unwrap();
    assert!(
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .classify_owner(&request)
            .await
            .unwrap_err()
            .contains("runtime-server-owner-activation-binding-mismatch")
    );

    receipt.launcher_artifact_digest = request.artifact_digest.clone();
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(dir.path(), &receipt)
        .await
        .unwrap();
    assert_eq!(
        agent_semantic_client_db::runtime_server_supervisor::RuntimeServerSupervisor
            .classify_owner(&request)
            .await
            .unwrap(),
        agent_semantic_client_db::runtime_server_supervisor::SupervisorOutcome::AlreadyResident
    );
}

#[tokio::test]
async fn malformed_partial_and_unknown_owner_receipts_fail_closed() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"{").unwrap();
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt_state(dir.path())
            .await
            .is_err()
    );
    for value in [
        serde_json::json!({"schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID}),
        serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "1",
        }),
        serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "1",
            "processId": 7,
            "nonce": "partial",
            "stateHome": dir.path(),
            "publicationNonce": "publication-partial",
        }),
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-server-owner-spawn.unknown",
            "schemaVersion": "1",
        }),
    ] {
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt_state(
                dir.path(),
            )
            .await
            .is_err()
        );
    }
}

#[tokio::test]
async fn unreleased_schema_version_2_is_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("runtime/server/owner-spawn.v1.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID,
            "schemaVersion": "2",
        }))
        .unwrap(),
    )
    .unwrap();

    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt_state(dir.path())
            .await
            .is_err(),
        "unreleased schema version 2 must remain fail-closed until a Release changes the canonical policy"
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

#[tokio::test]
async fn stale_exit_receipt_is_ignored_for_new_owner_epoch() {
    let dir = tempdir().unwrap();
    agent_semantic_client_db::runtime_server_lifecycle::publish_with_errors(
        dir.path(),
        11,
        true,
        vec!["stale".into()],
    )
    .await
    .unwrap();
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_exit_for(dir.path(), 12,)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_exit_for(dir.path(), 11)
            .await
            .unwrap()
            .unwrap()
            .owner_epoch,
        11
    );
}

fn fixture_endpoint(state_home: &std::path::Path, owner_epoch: u64) -> RuntimeServerEndpoint {
    let runtime_root = agent_semantic_client_db::runtime_server_runtime_base(state_home).unwrap();
    let binding_token = format!("binding-{owner_epoch}");
    let runtime_binary_identity = RuntimeBinaryIdentity::from_bytes(b"runtime-digest");
    let identity_value = runtime_binary_identity.content_digest().to_string();
    let status_identity = blake3::hash(
        format!(
            "{}\0{}\0{}",
            owner_epoch,
            binding_token,
            runtime_binary_identity.content_digest()
        )
        .as_bytes(),
    )
    .to_hex();
    let loopback = |port| {
        agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(
            ([127, 0, 0, 1], port).into(),
        )
        .expect("loopback endpoint")
    };
    RuntimeServerEndpoint {
        binary_content_digest: identity_value,
        runtime_generation_digest: "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        schema_digest: "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        schema_id: "agent.semantic-protocols.runtime-server-endpoint".to_owned(),
        schema_version: "1".to_owned(),
        transport_binding: agent_semantic_client_db::runtime_server_control::RuntimeTransportBinding {
            transport_contract_digest:
                agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest(),
            owner_epoch,
            owner_process_id: 1,
            binding_token,
            control_endpoint: loopback(44001 + (owner_epoch % 100) as u16),
            data_endpoint: loopback(44101 + (owner_epoch % 100) as u16),
            provider_endpoint: loopback(44201 + (owner_epoch % 100) as u16),
        },
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_binary_identity: runtime_binary_identity.clone(),
        monitor_capability: true,
        observed_runtime_binary_identity: runtime_binary_identity,
        artifact_mode: "dev".to_owned(),
        artifact_catalog_digest: format!("blake3-256:{}", "a".repeat(64)),
        workspace_store_path: runtime_root.join("workspaces").display().to_string(),
        status_memory_path: runtime_root
            .join(format!("status-{}.memory", &status_identity[..16]))
            .display()
            .to_string(),
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
    tokio::fs::write(&endpoint.status_memory_path, b"fixture")
        .await
        .unwrap();
}

#[tokio::test]
async fn resident_transaction_requires_the_matching_previous_owner_drain() {
    let temporary = tempdir().expect("resident transaction fixture");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let previous_source = temporary.path().join("previous-asp");
    let target = state_home.join("runtime/bin/asp");
    let source_bytes = b"#!/bin/sh\nexit 0\n";
    let previous_serving_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"previous owner",
        );
    std::fs::write(&source, source_bytes).expect("write fixture executable");
    std::fs::write(&previous_source, b"previous owner").expect("write previous executable");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("mark fixture executable");
    std::fs::set_permissions(&previous_source, std::fs::Permissions::from_mode(0o755))
        .expect("mark previous executable");

    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        &state_home,
        &previous_source,
        &target,
        "dev",
    )
    .await
    .expect("publish previous serving artifact");
    let previous_activation = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        &state_home,
    )
    .await
    .expect("read previous activation")
    .expect("previous activation");
    assert_eq!(previous_activation.artifact_digest, previous_serving_digest);
    agent_semantic_artifacts::runtime_artifact_activation::commit_runtime_artifact_activation(
        &state_home,
        &previous_activation,
        None,
    )
    .await
    .expect("commit previous serving artifact");

    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        "dev",
    )
    .await
    .expect("publish activation candidate");
    let activation = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        &state_home,
    )
    .await
    .expect("read pending activation")
    .expect("pending activation");
    agent_semantic_artifacts::runtime_artifact_activation::commit_runtime_artifact_activation(
        &state_home,
        &activation,
        Some(&previous_serving_digest),
    )
    .await
    .expect("commit activation");

    let mut endpoint = fixture_endpoint(&state_home, 52);
    let runtime_identity = RuntimeBinaryIdentity::from_bytes(source_bytes);
    endpoint.owner_process_id =
        agent_semantic_runtime::runtime_process_lifecycle::current_process_id();
    endpoint.runtime_artifact_path = activation.artifact_path.display().to_string();
    endpoint.binary_content_digest = activation.artifact_digest.to_string();
    endpoint.runtime_binary_identity = runtime_identity.clone();
    endpoint.observed_runtime_binary_identity = runtime_identity;
    let runtime_root =
        agent_semantic_client_db::runtime_server_runtime_base(&state_home).expect("Runtime root");
    let status_identity = blake3::hash(
        format!(
            "{}\0{}\0{}",
            endpoint.owner_epoch,
            endpoint.binding_token,
            endpoint.runtime_binary_identity.content_digest()
        )
        .as_bytes(),
    )
    .to_hex();
    endpoint.status_memory_path = runtime_root
        .join(format!("status-{}.memory", &status_identity[..16]))
        .display()
        .to_string();
    tokio::fs::create_dir_all(&runtime_root)
        .await
        .expect("create Runtime root");
    let _control = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind control listener");
    let _data = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind data listener");
    let _provider = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind provider listener");
    endpoint.control_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(_control.local_addr().expect("control address")).expect("control endpoint");
    endpoint.data_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(_data.local_addr().expect("data address")).expect("data endpoint");
    endpoint.provider_endpoint = agent_semantic_client_db::runtime_server_control::RuntimeServerLoopbackEndpoint::from_socket_addr(_provider.local_addr().expect("provider address")).expect("provider endpoint");
    let endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(&state_home).unwrap();
    agent_semantic_client_db::runtime_server_control::publish_runtime_server_endpoint(
        &endpoint_path,
        &endpoint,
    )
    .await
    .expect("publish endpoint");

    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        &state_home,
        &RuntimeServerSpawnReceipt {
            schema_id: agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_ID.to_owned(),
            schema_version: agent_semantic_client_db::RUNTIME_SERVER_OWNER_SPAWN_SCHEMA_VERSION
                .to_owned(),
            process_id: endpoint.owner_process_id,
            nonce: "resident-transaction".to_owned(),
            state_home: state_home.display().to_string(),
            publication_nonce: activation.publication_nonce.clone(),
            launcher_artifact_path: activation.artifact_path.display().to_string(),
            launcher_artifact_digest: activation.artifact_digest.clone(),
            spawn_argv: vec![
                activation.artifact_path.display().to_string(),
                "server".to_owned(),
            ],
            previous_serving_digest: Some(previous_serving_digest),
            previous_owner_epoch: Some(41),
        },
    )
    .await
    .expect("publish owner receipt");

    let serving_publication = agent_semantic_client_db::runtime_server_lifecycle::
        resolve_runtime_server_serving_publication(&state_home)
        .await
        .expect("content-bound serving endpoint does not require drain observation");
    assert_eq!(
        serving_publication.endpoint.data_endpoint,
        endpoint.data_endpoint
    );
    assert_eq!(
        serving_publication.publication_nonce,
        activation.publication_nonce
    );
    assert_eq!(
        serving_publication.artifact_digest,
        activation.artifact_digest
    );

    let missing = agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
        &state_home,
    )
    .await
    .expect_err("missing previous-owner drain must fail closed");
    assert!(
        missing.contains("missing drain receipt for previous owner epoch 41"),
        "unexpected resident transaction failure: {missing}"
    );

    agent_semantic_client_db::runtime_server_lifecycle::publish_drain(
        &state_home,
        agent_semantic_client_db::RuntimeServerDrainReceipt {
            owner_epoch: 40,
            services: serde_json::json!({}),
            remaining_task_count: 0,
            remaining_child_count: 0,
            clean_drain: true,
        },
    )
    .await
    .expect("publish wrong drain");
    let mismatched =
        agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            &state_home,
        )
        .await
        .expect_err("mismatched previous-owner drain must fail closed");
    assert!(mismatched.contains("expectedEpoch=41 observedEpoch=40"));

    agent_semantic_client_db::runtime_server_lifecycle::publish_drain(
        &state_home,
        agent_semantic_client_db::RuntimeServerDrainReceipt {
            owner_epoch: 41,
            services: serde_json::json!({}),
            remaining_task_count: 0,
            remaining_child_count: 0,
            clean_drain: true,
        },
    )
    .await
    .expect("publish matching drain");
    let transaction =
        agent_semantic_client_db::runtime_server_lifecycle::observe_resident_transaction(
            &state_home,
        )
        .await
        .expect("observe complete resident transaction");
    assert_eq!(transaction.previous_owner_epoch, Some(41));
    assert_eq!(transaction.previous_drain_state, "clean");

    let (transaction, serving_endpoint) = agent_semantic_client_db::runtime_server_lifecycle::
        observe_resident_transaction_with_endpoint(&state_home)
        .await
        .expect("observe one content-bound transaction and endpoint");
    assert_eq!(
        transaction.endpoint_owner_epoch,
        serving_endpoint.owner_epoch
    );
    assert_eq!(
        transaction.endpoint_binary_content_digest.as_str(),
        serving_endpoint.binary_content_digest
    );
    assert_eq!(
        transaction.control_endpoint,
        serving_endpoint.control_endpoint
    );
    assert_eq!(transaction.data_endpoint, serving_endpoint.data_endpoint);
    assert_eq!(
        transaction.provider_endpoint,
        serving_endpoint.provider_endpoint
    );
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
async fn lifecycle_owner_projection_survives_service_identity_schema_drift() {
    let state_home = tempdir().unwrap();
    let mut endpoint = fixture_endpoint(state_home.path(), 11);
    endpoint.owner_process_id = 4242;
    publish_endpoint_fixture(state_home.path(), &endpoint).await;
    let endpoint_path =
        agent_semantic_client_db::runtime_server_endpoint_path(state_home.path()).unwrap();
    let mut value = serde_json::to_value(&endpoint).unwrap();
    value["runtimeBinaryIdentity"] = serde_json::json!({
        "kind": "developerSourceGeneration",
        "identity": {"value": "legacy", "algorithm": "blake3-metadata-v1"}
    });
    value["observedRuntimeBinaryIdentity"] = value["runtimeBinaryIdentity"].clone();
    tokio::fs::write(&endpoint_path, serde_json::to_vec(&value).unwrap())
        .await
        .unwrap();

    assert!(
        agent_semantic_client_db::runtime_server_control::read_runtime_server_supervisor_endpoint(
            state_home.path(),
        )
        .await
        .is_err(),
        "service admission must remain strict"
    );
    let owner = agent_semantic_client_db::runtime_server_control::read_runtime_server_endpoint_owner_binding(
        state_home.path(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(owner.owner_epoch, endpoint.owner_epoch);
    assert_eq!(owner.owner_process_id, endpoint.owner_process_id);
    assert_eq!(owner.binding_token, endpoint.binding_token);
    assert_eq!(owner.runtime_artifact_path, endpoint.runtime_artifact_path);
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
    let executable_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            &std::fs::read(&executable).unwrap(),
        );
    let process_id = agent_semantic_runtime::runtime_process_lifecycle::current_process_id();
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &RuntimeServerSpawnReceipt {
            schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
            schema_version: "1".to_owned(),
            process_id,
            nonce: "live-owner".to_owned(),
            state_home: state_home.path().display().to_string(),
            publication_nonce: "publication-live-owner".to_owned(),
            launcher_artifact_path: executable.display().to_string(),
            launcher_artifact_digest: executable_digest.clone(),
            spawn_argv: vec![executable.display().to_string()],
            previous_serving_digest: None,
            previous_owner_epoch: None,
        },
    )
    .await
    .unwrap();
    let request = agent_semantic_client_db::runtime_server_supervisor::SupervisorRequest {
        state_home: state_home.path().to_path_buf(),
        expected_executable: executable,
        publication_nonce: "publication-live-owner".to_owned(),
        artifact_digest: executable_digest,
        previous_artifact_digest: None,
        previous_owner_epoch: None,
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
