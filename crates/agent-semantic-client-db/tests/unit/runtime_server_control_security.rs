use std::sync::Arc;

use agent_semantic_client_db::runtime_server_control::{
    prepare_runtime_server_endpoint_in, publish_runtime_server_endpoint,
    validate_runtime_server_peer_fd,
};
use agent_semantic_client_db::runtime_server_runtime::RuntimeServerConnectionSupervisor;
use agent_semantic_client_db::{
    RuntimeServerOperation, WorkspaceDbRegistry, call_runtime_server,
    runtime_server::{RuntimeServer, RuntimeServerExit},
};

async fn fixture_endpoint(
    runtime_dir: &tempfile::TempDir,
    epoch: u64,
) -> (
    agent_semantic_client_db::RuntimeServerEndpoint,
    Arc<agent_semantic_artifacts::runtime_artifact_catalog::RuntimeArtifactCatalog>,
) {
    let state_home = agent_semantic_runtime::resolve_state_home().expect("resolve State Home");
    let catalog =
        agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_artifact_catalog(
            &state_home,
        )
        .await
        .expect("load runtime artifact catalog");
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
fn authenticates_same_uid_unix_peer() {
    let (left, right) =
        std::os::unix::net::UnixStream::pair().expect("same-UID Unix socket pair available");
    validate_runtime_server_peer_fd(std::os::fd::AsRawFd::as_raw_fd(&left))
        .expect("left peer credential matches current effective UID");
    validate_runtime_server_peer_fd(std::os::fd::AsRawFd::as_raw_fd(&right))
        .expect("right peer credential matches current effective UID");
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
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 29).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());
    call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
            b"transport-replay-probe",
        ),
        "single-use-control-nonce".to_owned(),
    )
    .await
    .expect("first use of control nonce succeeds");
    let replay = call_runtime_server(
        &endpoint,
        RuntimeServerOperation::Status,
        agent_semantic_artifacts::runtime_artifact_catalog::RuntimeBinaryIdentity::from_bytes(
            b"transport-replay-probe",
        ),
        "single-use-control-nonce".to_owned(),
    )
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
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
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
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
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
    let runtime_dir = tempfile::tempdir().expect("create isolated runtime server directory");
    let (endpoint, artifact_catalog) = fixture_endpoint(&runtime_dir, 41).await;
    let server = RuntimeServer::bind_with_catalog(
        endpoint.clone(),
        Arc::new(WorkspaceDbRegistry::default()),
        artifact_catalog,
    )
    .await
    .expect("bind runtime server");
    let server = tokio::spawn(server.serve());
    let mut stalled = tokio::net::UnixStream::connect(&endpoint.socket_path)
        .await
        .expect("connect stalled unauthenticated client");
    tokio::time::sleep(
        agent_semantic_client_db::runtime_server_runtime::RUNTIME_SERVER_CONNECTION_IO_BUDGET * 3,
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
