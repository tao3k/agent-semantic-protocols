use super::{
    ENDPOINT_SCHEMA_ID, REQUEST_SCHEMA_ID, RuntimeServerControlRequest, RuntimeServerEndpoint,
    RuntimeServerOperation, SCHEMA_VERSION,
};
use agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity;

fn endpoint() -> RuntimeServerEndpoint {
    RuntimeServerEndpoint {
        binary_content_digest: RuntimeBinaryIdentity::from_bytes(b"running-runtime")
            .content_digest()
            .to_string(),
        runtime_generation_digest: "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        schema_digest: "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        schema_id: ENDPOINT_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        transport_contract_digest:
            agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest(),
        owner_epoch: 7,
        owner_process_id: 77,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(b"running-runtime"),
        monitor_capability: true,
        observed_runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(b"running-runtime"),
        artifact_mode: "release".to_owned(),
        artifact_catalog_digest:
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        binding_token: "binding-token".to_owned(),
        socket_path: "/runtime/control.sock".to_owned(),
        data_plane_socket_path: "/runtime/data.sock".to_owned(),
        provider_plane_socket_path: "/runtime/providers.sock".to_owned(),
        workspace_store_path: "/runtime/workspaces".to_owned(),
        status_memory_path: "/runtime/status.memory".to_owned(),
    }
}

fn request(operation: RuntimeServerOperation) -> RuntimeServerControlRequest {
    RuntimeServerControlRequest {
        schema_id: REQUEST_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        operation,
        project_root: None,
        expected_runtime_binary_identity: RuntimeBinaryIdentity::from_bytes(b"running-runtime"),
        request_id: "supervisor-control".to_owned(),
        transport_contract_digest: "blake3-256:running-transport".to_owned(),
        owner_epoch: 7,
        binding_token: "binding-token".to_owned(),
    }
}

#[test]
fn stale_transport_is_rejected_by_status_and_admitted_by_explicit_restart() {
    let mut endpoint = endpoint();
    endpoint.transport_contract_digest =
        agent_semantic_client_db::runtime_server_control::runtime_server_transport_contract_digest(
        );
    let mut status = request(RuntimeServerOperation::Status);
    status.transport_contract_digest = "blake3-256:next-transport".to_owned();
    assert!(
        status
            .requires_restart(&endpoint)
            .expect_err("status must reject data-plane contract drift")
            .contains("transport contract mismatch")
    );

    status.operation = RuntimeServerOperation::Restart;
    assert!(
        status
            .requires_restart(&endpoint)
            .expect("authenticated restart must cross data-plane contract drift")
    );
}

#[test]
fn transport_digest_includes_the_northbound_client_frame_contract() {
    let source = include_str!("../../src/runtime_server_control/model.rs");
    assert!(source.contains("ASP_CLIENT_FRAME_CONTRACT"));
    assert!(source.contains("b\"asp-client-frame\".as_slice()"));
    assert!(source.contains("schemas/asp-client-frame.schema.json"));
}

#[test]
fn restart_never_weakens_owner_epoch_or_binding_token() {
    let endpoint = endpoint();
    let mut restart = request(RuntimeServerOperation::Restart);
    restart.transport_contract_digest = "blake3-256:next-transport".to_owned();
    restart.owner_epoch += 1;
    assert_eq!(
        restart.requires_restart(&endpoint),
        Err("Runtime Server control request binding mismatch".to_owned())
    );
    restart.owner_epoch = endpoint.owner_epoch;
    restart.binding_token = "wrong-binding".to_owned();
    assert_eq!(
        restart.requires_restart(&endpoint),
        Err("Runtime Server control request binding mismatch".to_owned())
    );
}

#[test]
fn endpoint_requires_a_typed_artifact_mode_and_catalog_digest() {
    endpoint()
        .validate_supervisor_control()
        .expect("valid endpoint");

    let mut invalid_mode = endpoint();
    invalid_mode.artifact_mode = "developer".to_owned();
    assert!(invalid_mode.validate_supervisor_control().is_err());

    let mut invalid_digest = endpoint();
    invalid_digest.artifact_catalog_digest = "catalog-latest".to_owned();
    assert!(invalid_digest.validate_supervisor_control().is_err());
}

#[test]
fn endpoint_owner_process_identity_is_additive_and_legacy_safe() {
    let mut encoded = serde_json::to_value(endpoint()).expect("serialize endpoint");
    assert_eq!(encoded["ownerProcessId"], 77);

    encoded
        .as_object_mut()
        .expect("endpoint object")
        .remove("ownerProcessId");
    let legacy: RuntimeServerEndpoint =
        serde_json::from_value(encoded).expect("decode legacy endpoint without ownerProcessId");
    assert_eq!(legacy.owner_process_id, 0);
}

#[test]
fn stop_fallback_requires_epoch_process_and_executable_binding() {
    let endpoint = endpoint();
    let owner = agent_semantic_client_db::RuntimeServerSpawnReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
        schema_version: "1".to_owned(),
        process_id: endpoint.owner_process_id,
        nonce: format!("owner-{}", endpoint.owner_process_id),
        state_home: "/runtime".to_owned(),
        activation_generation: 1,
        launcher_artifact_path: endpoint.runtime_artifact_path.clone(),
        launcher_artifact_digest:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                b"owner fixture",
            ),
        spawn_argv: vec!["server".to_owned(), "daemon".to_owned()],
        previous_serving_digest: None,
        previous_owner_epoch: None,
    };

    agent_semantic_client_db::runtime_server_supervisor::validate_runtime_server_owner_binding(
        &endpoint, &endpoint, &owner,
    )
    .expect("matching owner binding");

    let mut replacement = endpoint.clone();
    replacement.owner_epoch += 1;
    assert!(
        agent_semantic_client_db::runtime_server_supervisor::validate_runtime_server_owner_binding(
            &endpoint,
            &replacement,
            &owner,
        )
        .is_err(),
        "a replacement epoch must never be terminated by a stale stop"
    );

    let mut wrong_owner = owner.clone();
    wrong_owner.process_id += 1;
    assert!(
        agent_semantic_client_db::runtime_server_supervisor::validate_runtime_server_owner_binding(
            &endpoint,
            &endpoint,
            &wrong_owner,
        )
        .is_err(),
        "endpoint and owner receipt process identities must match"
    );

    let mut legacy = endpoint.clone();
    legacy.owner_process_id = 0;
    assert!(
        agent_semantic_client_db::runtime_server_supervisor::validate_runtime_server_owner_binding(
            &legacy, &legacy, &owner,
        )
        .is_err(),
        "legacy unbound endpoints must fail closed"
    );
}

#[tokio::test]
async fn identity_handoff_retires_only_the_bound_runtime_server_owner() {
    let state_home = tempfile::tempdir().expect("temporary State Home");
    let runtime_artifact_path = state_home.path().join("runtime/bin/asp");
    let mut endpoint = agent_semantic_client_db::prepare_runtime_server_endpoint(
        state_home.path(),
        &runtime_artifact_path,
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"test-runtime-artifact",
        ),
        "dev",
        "blake3-256:0000000000000000000000000000000000000000000000000000000000000000",
        7,
        "identity-handoff-binding",
    )
    .await
    .expect("prepare endpoint");
    endpoint.owner_process_id = std::process::id();
    let owner = agent_semantic_client_db::RuntimeServerSpawnReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
        schema_version: "1".to_owned(),
        process_id: endpoint.owner_process_id,
        nonce: format!("owner-{}", endpoint.owner_process_id),
        state_home: state_home.path().to_string_lossy().into_owned(),
        activation_generation: 1,
        launcher_artifact_path: endpoint.runtime_artifact_path.clone(),
        launcher_artifact_digest:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                b"owner fixture",
            ),
        spawn_argv: vec!["server".to_owned(), "daemon".to_owned()],
        previous_serving_digest: None,
        previous_owner_epoch: None,
    };
    let endpoint_path =
        agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path_async(
            state_home.path(),
        )
        .await
        .expect("endpoint path");
    agent_semantic_client_db::runtime_server_control::publish_runtime_server_endpoint(
        &endpoint_path,
        &endpoint,
    )
    .await
    .expect("publish endpoint");
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &owner,
    )
    .await
    .expect("publish owner receipt");

    agent_semantic_client_db::runtime_server_supervisor::retire_runtime_server_owner_for_handoff(
        state_home.path(),
        &endpoint,
    )
    .await
    .expect("retire the bound owner");
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(state_home.path())
            .await
            .expect("read retired owner receipt")
            .is_none()
    );

    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(
        state_home.path(),
        &owner,
    )
    .await
    .expect("republish owner receipt");
    let mut stale_endpoint = endpoint.clone();
    stale_endpoint.owner_epoch += 1;
    assert!(
        agent_semantic_client_db::runtime_server_supervisor::retire_runtime_server_owner_for_handoff(
            state_home.path(),
            &stale_endpoint,
        )
        .await
        .is_err(),
        "a stale epoch must fail closed"
    );
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(state_home.path())
            .await
            .expect("read preserved owner receipt")
            .is_some(),
        "binding mismatch must preserve the current owner receipt"
    );
}
