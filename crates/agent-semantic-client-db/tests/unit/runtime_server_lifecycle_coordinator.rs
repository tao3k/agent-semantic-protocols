use agent_semantic_client_db::runtime_server_lifecycle_coordinator::{OwnerClassification, RuntimeServerLifecycleCoordinator};
use agent_semantic_client_db::{RuntimeServerExitReceipt, RuntimeServerSpawnReceipt};
use tempfile::tempdir;

#[tokio::test]
async fn missing_owner_is_classified_without_process_probe() {
    let coordinator = RuntimeServerLifecycleCoordinator::new("/tmp/asp-test-state", "/tmp/asp-test-bin");
    assert_eq!(coordinator.classify(None).await.unwrap(), OwnerClassification::Missing);
}

#[tokio::test]
async fn stale_pid_is_classified_without_signal() {
    let coordinator = RuntimeServerLifecycleCoordinator::new("/tmp/asp-test-state", "/tmp/asp-test-bin");
    assert_eq!(coordinator.classify(Some(u32::MAX)).await.unwrap(), OwnerClassification::Stale);
}

#[tokio::test]
async fn wrong_executable_is_rejected_before_termination() {
    let coordinator = RuntimeServerLifecycleCoordinator::new("/tmp/asp-test-state", "/definitely/not/the/current/executable");
    let result = coordinator.terminate_verified(std::process::id(), false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn owner_receipt_roundtrip_is_atomic_and_schema_stable() {
    let dir = tempdir().unwrap();
    let receipt = RuntimeServerSpawnReceipt { schema_id: "owner".into(), schema_version: "1".into(), process_id: 7, nonce: "n".into(), state_home: dir.path().display().to_string(), runtime_artifact_path: "/bin/asp".into() };
    agent_semantic_client_db::runtime_server_lifecycle::write_owner_receipt(dir.path(), &receipt).await.unwrap();
    assert_eq!(agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(dir.path()).await.unwrap().unwrap().nonce, "n");
    agent_semantic_client_db::runtime_server_lifecycle::remove_owner_receipt(dir.path()).await.unwrap();
    assert!(agent_semantic_client_db::runtime_server_lifecycle::read_owner_receipt(dir.path()).await.unwrap().is_none());
}

#[tokio::test]
async fn exit_receipt_roundtrip_preserves_terminal_state() {
    let dir = tempdir().unwrap();
    agent_semantic_client_db::runtime_server_lifecycle::publish_with_errors(dir.path(), 9, true, vec!["ok".into()]).await.unwrap();
    let receipt: RuntimeServerExitReceipt = agent_semantic_client_db::runtime_server_lifecycle::read_latest_owner_exit(dir.path()).await.unwrap().unwrap();
    assert_eq!(receipt.owner_epoch, 9);
    assert!(receipt.clean_drain);
}
