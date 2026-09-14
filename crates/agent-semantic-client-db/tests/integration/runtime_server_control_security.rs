// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::sync::Arc;

use agent_semantic_client_db::RuntimeServerOperation;
use agent_semantic_client_db::WorkspaceDbRegistry;
use agent_semantic_client_db::call_runtime_server;
use agent_semantic_client_db::runtime_server::RuntimeServer;
use agent_semantic_client_db::runtime_server::RuntimeServerExit;
use agent_semantic_client_db::runtime_server_connection::RuntimeServerConnectionSupervisor;
use agent_semantic_client_db::runtime_server_control::prepare_runtime_server_endpoint_in;
use agent_semantic_client_db::runtime_server_control::publish_runtime_server_endpoint;

async fn raw_status_exchange(
    endpoint: &agent_semantic_client_db::RuntimeServerEndpoint,
    request_id: &str,
) -> Result<agent_semantic_client_db::RuntimeServerControlReceipt, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // This test exercises transport replay itself. The public Status client
    // additionally requires a canonical resident-transaction authority, which
    // is deliberately outside this isolated transport fixture.
    let request = agent_semantic_client_db::runtime_server_control::RuntimeServerControlRequest {
        schema_id: "agent.semantic-protocols.runtime-server-control-request".to_owned(),
        schema_version: "1".to_owned(),
        operation: RuntimeServerOperation::Status,
        expected_runtime_binary_identity:
            agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
                b"transport-replay-probe",
            ),
        request_id: request_id.to_owned(),
        transport_contract_digest: endpoint.transport_contract_digest.clone(),
        owner_epoch: endpoint.owner_epoch,
        binding_token: endpoint.binding_token.clone(),
        project_root: None,
    };
    let bytes = serde_json::to_vec(&vec![request])
        .map_err(|error| format!("encode replay probe: {error}"))?;
    let mut stream = tokio::net::TcpStream::connect(endpoint.control_endpoint.socket_addr())
        .await
        .map_err(|error| format!("connect replay probe: {error}"))?;
    stream
        .write_u32(u32::try_from(bytes.len()).map_err(|_| "replay probe is too large")?)
        .await
        .map_err(|error| format!("write replay probe length: {error}"))?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|error| format!("write replay probe: {error}"))?;
    let length = stream
        .read_u32()
        .await
        .map_err(|error| format!("read replay receipt length: {error}"))?;
    let mut bytes = vec![0; length as usize];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|error| format!("read replay receipt: {error}"))?;
    let mut receipts: Vec<agent_semantic_client_db::RuntimeServerControlReceipt> =
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("decode replay receipt: {error}"))?;
    if receipts.len() != 1 {
        return Err(format!("replay probe returned {} receipts", receipts.len()));
    }
    Ok(receipts.remove(0))
}

fn private_runtime_dir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    // Production endpoint preparation validates both the serving directory
    // and its Runtime root. A default tempfile lives directly below the
    // world-writable OS temporary root, which is intentionally not a valid
    // Runtime authority. Keep security fixtures beneath a private directory
    // in this checkout's ignored target tree instead.
    let fixture_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/runtime-server-security-tests");
    std::fs::create_dir_all(&fixture_root).expect("create private Runtime fixture root");
    std::fs::set_permissions(&fixture_root, std::fs::Permissions::from_mode(0o700))
        .expect("protect private Runtime fixture root");
    tempfile::Builder::new()
        .prefix("runtime-")
        .tempdir_in(fixture_root)
        .expect("create isolated runtime server directory")
}

async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> (
    agent_semantic_client_db::RuntimeServerEndpoint,
    Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog>,
) {
    // Transport security is independent of the host's currently published
    // provider closure. A self-contained catalog prevents local installation
    // drift from changing these endpoint and replay tests.
    let catalog = agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog::new(
        agent_semantic_config::runtime_dev::RuntimeArtifactMode::Release,
    );
    let endpoint = prepare_runtime_server_endpoint_in(
        runtime_dir.path(),
        std::path::Path::new("/runtime/asp"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"runtime-digest",
        ),
        catalog.mode_label(),
        &catalog.digest(),
        epoch,
        &format!("binding-{epoch}"),
    )
    .await
    .expect("prepare isolated runtime server endpoint");
    (endpoint, Arc::new(catalog))
}

#[test]
fn connection_supervisor_enforces_hard_capacity_and_records_rejection() {
    let supervisor = RuntimeServerConnectionSupervisor::new("security-test", 2);
    let first = supervisor.try_admit().expect("first connection admitted");
    let second = supervisor.try_admit().expect("second connection admitted");
    assert!(
        supervisor.try_admit().is_none(),
        "third connection rejected"
    );
    let saturated = supervisor.snapshot();
    assert_eq!(saturated.active, 2);
    assert_eq!(saturated.limit, 2);
    assert_eq!(saturated.high_watermark, 2);
    assert_eq!(saturated.rejected, 1);
    drop(first);
    let replacement = supervisor
        .try_admit()
        .expect("released capacity can be reused");
    drop(replacement);
    drop(second);
    assert_eq!(supervisor.snapshot().active, 0);
}

#[tokio::test]
async fn rejects_symlinked_runtime_base_before_permission_changes() {
    let unique = format!(
        "asp-runtime-security-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    );
    let fixture_root = std::env::temp_dir().join(unique);
    let redirect_target = fixture_root.join("redirect-target");
    let runtime_base = fixture_root.join("runtime-base");
    std::fs::create_dir_all(&redirect_target).expect("create redirect target");
    std::os::unix::fs::symlink(&redirect_target, &runtime_base)
        .expect("create malicious runtime-base symlink");
    let error = prepare_runtime_server_endpoint_in(
        &runtime_base,
        std::path::Path::new("/tmp/asp-test-artifact"),
        &agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"artifact-digest",
        ),
        "test",
        "catalog-digest",
        1,
        "binding-token",
    )
    .await
    .expect_err("symlinked runtime root must fail closed");
    assert!(error.contains("non-symlink current-UID"), "{error}");
    std::fs::remove_file(&runtime_base).expect("remove test symlink");
    std::fs::remove_dir(&redirect_target).expect("remove redirect target");
    std::fs::remove_dir(&fixture_root).expect("remove fixture root");
}

#[tokio::test(flavor = "multi_thread")]
async fn control_request_nonce_is_single_use_for_the_owner_epoch() {
    let runtime_dir = private_runtime_dir();
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 29).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());
    raw_status_exchange(&endpoint, "single-use-control-nonce")
        .await
        .expect("first use of control nonce succeeds");
    let replay = raw_status_exchange(&endpoint, "single-use-control-nonce")
        .await
        .expect_err("replayed control nonce must fail closed");
    assert!(!replay.is_empty(), "replay rejection must be typed");
    call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        endpoint.runtime_binary_identity.clone(),
        "replay-test-shutdown".to_owned(),
    )
    .await
    .expect("restart drains replay test server");
    assert_eq!(
        server.await.expect("join runtime server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
}

#[tokio::test]
async fn endpoint_publication_is_private_and_non_symlink() {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    let runtime_dir = private_runtime_dir();
    let (endpoint, _) = fixture_endpoint(&runtime_dir, 31).await;
    let endpoint_path = runtime_dir.path().join("endpoint.v1.json");
    publish_runtime_server_endpoint(&endpoint_path, &endpoint)
        .await
        .expect("publish private endpoint");
    let metadata = std::fs::symlink_metadata(&endpoint_path).expect("inspect endpoint");
    assert!(metadata.file_type().is_file());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
}

#[tokio::test]
async fn endpoint_publication_does_not_follow_prepositioned_temp_symlink() {
    let runtime_dir = private_runtime_dir();
    let (endpoint, _) = fixture_endpoint(&runtime_dir, 37).await;
    let endpoint_path = runtime_dir.path().join("endpoint.v1.json");
    let temporary_identity =
        blake3::hash(format!("{}\0{}", endpoint.owner_epoch, endpoint.binding_token).as_bytes())
            .to_hex();
    let temporary = endpoint_path.with_extension(format!(
        "tmp-{}-{}",
        endpoint.owner_epoch,
        &temporary_identity[..16]
    ));
    let redirect_target = runtime_dir.path().join("redirect-target");
    std::fs::write(&redirect_target, b"sentinel").expect("write redirect target");
    std::os::unix::fs::symlink(&redirect_target, &temporary)
        .expect("preposition endpoint temporary symlink");
    let error = publish_runtime_server_endpoint(&endpoint_path, &endpoint)
        .await
        .expect_err("exclusive endpoint publication must reject a prepositioned symlink");
    assert!(error.contains("exclusive"), "{error}");
    assert_eq!(
        std::fs::read(&redirect_target).expect("read redirect target"),
        b"sentinel"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unauthenticated_control_connection_is_closed_at_first_frame_budget() {
    let runtime_dir = private_runtime_dir();
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 41).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());
    let mut stalled = tokio::net::TcpStream::connect(endpoint.control_endpoint.socket_addr())
        .await
        .expect("connect stalled unauthenticated client");
    tokio::time::sleep(
        agent_semantic_client_db::runtime_server_connection::RUNTIME_SERVER_CONNECTION_IO_BUDGET
            * 3,
    )
    .await;
    let mut byte = [0_u8; 1];
    let read = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        tokio::io::AsyncReadExt::read(&mut stalled, &mut byte),
    )
    .await;
    assert!(
        matches!(read, Ok(Ok(0)) | Ok(Err(_))),
        "stalled first-frame connection remained admitted: {read:?}"
    );
    call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Restart,
        endpoint.runtime_binary_identity.clone(),
        "slowloris-test-shutdown".to_owned(),
    )
    .await
    .expect("restart drains slowloris test server");
    assert_eq!(
        server.await.expect("join runtime server").expect("serve"),
        RuntimeServerExit::RestartRequested
    );
}
