use super::{oneshot, RuntimeServerWorkspaceRegistry, WorkspaceRecoveryReceipt};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn progress_receipt() -> WorkspaceRecoveryReceipt {
    WorkspaceRecoveryReceipt {
        schema_id: crate::runtime_server_workspace::WORKSPACE_RECOVERY_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        request_id: "request-1".to_owned(),
        workspace_identity: "workspace-1".to_owned(),
        source: crate::runtime_server_workspace::WorkspaceRecoverySource::TursoGeneration,
        state: crate::runtime_server_workspace::WorkspaceGenerationState::PublishingNext,
        active_epoch: 1,
        target_epoch: 2,
        generation_digest:
            "blake3-256:1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        source_root_digest:
            "blake3-256:2222222222222222222222222222222222222222222222222222222222222222".to_owned(),
        old_generation_readable: true,
        counters: crate::runtime_server_workspace::RuntimeDataPlaneCounters::default(),
    }
}

#[tokio::test]
async fn immediate_writer_acceptance_returns_reserved_progress() {
    let (send, receive) = oneshot::channel();
    send.send(Ok(progress_receipt())).expect("receiver is live");

    let receipt = RuntimeServerWorkspaceRegistry::await_recovery_acceptance(
        receive,
        tokio::time::Instant::now() + std::time::Duration::from_millis(100),
    )
    .await
    .expect("accepted publication");

    assert_eq!(receipt.active_epoch, 1);
    assert_eq!(receipt.target_epoch, 2);
    assert_eq!(
        receipt.state,
        crate::runtime_server_workspace::WorkspaceGenerationState::PublishingNext
    );
}

#[tokio::test]
async fn acceptance_timeout_does_not_cancel_enqueued_writer_work() {
    let (send, receive) = oneshot::channel();
    let (release, wait_for_release) = oneshot::channel();
    let completed = Arc::new(AtomicBool::new(false));
    let writer_completed = Arc::clone(&completed);
    let writer = tokio::spawn(async move {
        let _ = wait_for_release.await;
        writer_completed.store(true, Ordering::Release);
        let _ = send.send(Ok(progress_receipt()));
    });

    let error = RuntimeServerWorkspaceRegistry::await_recovery_acceptance(
        receive,
        tokio::time::Instant::now(),
    )
    .await
    .expect_err("zero acceptance deadline must fail closed");

    assert!(error.contains("acceptance deadline"));
    let _ = release.send(());
    writer.await.expect("writer task completed");
    assert!(completed.load(Ordering::Acquire));
}
