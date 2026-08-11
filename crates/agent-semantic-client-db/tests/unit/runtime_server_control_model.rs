use super::{
    ENDPOINT_SCHEMA_ID, REQUEST_SCHEMA_ID, RuntimeServerControlRequest, RuntimeServerEndpoint,
    RuntimeServerOperation, SCHEMA_VERSION,
};

fn endpoint() -> RuntimeServerEndpoint {
    RuntimeServerEndpoint {
        schema_id: ENDPOINT_SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION.to_owned(),
        transport_contract_digest: "blake3-256:running-transport".to_owned(),
        owner_epoch: 7,
        runtime_artifact_path: "/runtime/asp".to_owned(),
        runtime_artifact_digest: "blake3-256:running-runtime".to_owned(),
        artifact_mode: "release".to_owned(),
        artifact_catalog_digest:
            "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        binding_token: "binding-token".to_owned(),
        socket_path: "/runtime/control.sock".to_owned(),
        data_plane_socket_path: "/runtime/data.sock".to_owned(),
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
        expected_runtime_artifact_digest: "blake3-256:running-runtime".to_owned(),
        request_id: "supervisor-reconcile".to_owned(),
        transport_contract_digest: "blake3-256:running-transport".to_owned(),
        owner_epoch: 7,
        binding_token: "binding-token".to_owned(),
    }
}

#[test]
fn stale_transport_is_rejected_by_status_and_admitted_by_reconcile() {
    let endpoint = endpoint();
    let mut status = request(RuntimeServerOperation::Status);
    status.transport_contract_digest = "blake3-256:next-transport".to_owned();
    assert!(
        status
            .requires_restart(&endpoint)
            .expect_err("status must reject data-plane contract drift")
            .contains("transport contract mismatch")
    );

    status.operation = RuntimeServerOperation::Reconcile;
    assert!(
        status
            .requires_restart(&endpoint)
            .expect("authenticated reconcile must cross data-plane contract drift")
    );
}

#[test]
fn matching_reconcile_is_noop_and_either_digest_drift_requests_restart() {
    let endpoint = endpoint();
    let mut reconcile = request(RuntimeServerOperation::Reconcile);
    assert!(
        !reconcile
            .requires_restart(&endpoint)
            .expect("matching reconcile")
    );

    reconcile.expected_runtime_artifact_digest = "blake3-256:next-runtime".to_owned();
    assert!(
        reconcile
            .requires_restart(&endpoint)
            .expect("runtime drift")
    );
    reconcile.expected_runtime_artifact_digest = endpoint.runtime_artifact_digest.clone();
    reconcile.transport_contract_digest = "blake3-256:next-transport".to_owned();
    assert!(
        reconcile
            .requires_restart(&endpoint)
            .expect("transport drift")
    );
}

#[test]
fn reconcile_never_weakens_owner_epoch_or_binding_token() {
    let endpoint = endpoint();
    let mut reconcile = request(RuntimeServerOperation::Reconcile);
    reconcile.transport_contract_digest = "blake3-256:next-transport".to_owned();
    reconcile.owner_epoch += 1;
    assert_eq!(
        reconcile.requires_restart(&endpoint),
        Err("Runtime Server control request binding mismatch".to_owned())
    );
    reconcile.owner_epoch = endpoint.owner_epoch;
    reconcile.binding_token = "wrong-binding".to_owned();
    assert_eq!(
        reconcile.requires_restart(&endpoint),
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
