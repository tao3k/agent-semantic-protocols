use std::path::PathBuf;

use super::{
    GENERATION_DISCOVERY_RECEIPTS, GENERATION_DISCOVERY_SCHEMA_ID, GENERATION_DISCOVERY_TASKS,
    GenerationDiscoveryKey, GenerationDiscoveryTaskGuard, WorkspaceGenerationDiscoveryReceipt,
    WorkspaceGenerationDiscoveryState, unix_time_ms,
};

#[test]
fn discovery_receipt_is_discovering_before_future_completes() {
    let key = GenerationDiscoveryKey {
        workspace_identity: "discovering-receipt-test".to_owned(),
        project_root: PathBuf::from("/discovering-receipt-test"),
    };
    let started = unix_time_ms();
    GENERATION_DISCOVERY_RECEIPTS.insert(
        key.clone(),
        WorkspaceGenerationDiscoveryReceipt {
            schema_id: GENERATION_DISCOVERY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: key.workspace_identity.clone(),
            project_root: key.project_root.display().to_string(),
            state: WorkspaceGenerationDiscoveryState::Discovering,
            attempt: 1,
            started_at_unix_ms: started,
            deadline_unix_ms: started + 800,
            finished_at_unix_ms: None,
            reason_kind: None,
            error: None,
        },
    );
    let receipt = GENERATION_DISCOVERY_RECEIPTS.get(&key).expect("receipt");
    assert_eq!(
        receipt.state,
        WorkspaceGenerationDiscoveryState::Discovering
    );
    assert!(receipt.finished_at_unix_ms.is_none());
    drop(receipt);
    GENERATION_DISCOVERY_RECEIPTS.remove(&key);
}

#[test]
fn short_deadline_timeout_publishes_failed_terminal_receipt() {
    let key = GenerationDiscoveryKey {
        workspace_identity: "timeout-discovery-test".to_owned(),
        project_root: PathBuf::from("/timeout-discovery-test"),
    };
    let now = unix_time_ms();
    GENERATION_DISCOVERY_RECEIPTS.insert(
        key.clone(),
        WorkspaceGenerationDiscoveryReceipt {
            schema_id: GENERATION_DISCOVERY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: key.workspace_identity.clone(),
            project_root: key.project_root.display().to_string(),
            state: WorkspaceGenerationDiscoveryState::Failed,
            attempt: 1,
            started_at_unix_ms: now.saturating_sub(10),
            deadline_unix_ms: now,
            finished_at_unix_ms: Some(now),
            reason_kind: Some("discovery-timeout".to_owned()),
            error: Some("workspace candidate discovery exceeded 5ms".to_owned()),
        },
    );
    let receipt = GENERATION_DISCOVERY_RECEIPTS.get(&key).expect("receipt");
    assert_eq!(receipt.state, WorkspaceGenerationDiscoveryState::Failed);
    assert_eq!(receipt.reason_kind.as_deref(), Some("discovery-timeout"));
    assert!(receipt.finished_at_unix_ms.is_some());
    drop(receipt);
    GENERATION_DISCOVERY_RECEIPTS.remove(&key);
}

#[test]
fn discovery_error_publishes_failed_terminal_receipt() {
    let key = GenerationDiscoveryKey {
        workspace_identity: "error-discovery-test".to_owned(),
        project_root: PathBuf::from("/error-discovery-test"),
    };
    let now = unix_time_ms();
    GENERATION_DISCOVERY_RECEIPTS.insert(
        key.clone(),
        WorkspaceGenerationDiscoveryReceipt {
            schema_id: GENERATION_DISCOVERY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: key.workspace_identity.clone(),
            project_root: key.project_root.display().to_string(),
            state: WorkspaceGenerationDiscoveryState::Failed,
            attempt: 1,
            started_at_unix_ms: now,
            deadline_unix_ms: now + 800,
            finished_at_unix_ms: Some(now),
            reason_kind: Some("discovery-failed".to_owned()),
            error: Some("injected discovery error".to_owned()),
        },
    );
    let receipt = GENERATION_DISCOVERY_RECEIPTS.get(&key).expect("receipt");
    assert_eq!(receipt.reason_kind.as_deref(), Some("discovery-failed"));
    assert_eq!(receipt.error.as_deref(), Some("injected discovery error"));
    drop(receipt);
    GENERATION_DISCOVERY_RECEIPTS.remove(&key);
}

#[test]
fn terminal_failed_receipt_is_not_discovery_in_progress() {
    let key = GenerationDiscoveryKey {
        workspace_identity: "terminal-discovery-test".to_owned(),
        project_root: PathBuf::from("/terminal-discovery-test"),
    };
    let now = unix_time_ms();
    GENERATION_DISCOVERY_RECEIPTS.insert(
        key.clone(),
        WorkspaceGenerationDiscoveryReceipt {
            schema_id: GENERATION_DISCOVERY_SCHEMA_ID.to_owned(),
            schema_version: "1".to_owned(),
            workspace_identity: key.workspace_identity.clone(),
            project_root: key.project_root.display().to_string(),
            state: WorkspaceGenerationDiscoveryState::Failed,
            attempt: 1,
            started_at_unix_ms: now,
            deadline_unix_ms: now + 800,
            finished_at_unix_ms: Some(now),
            reason_kind: Some("discovery-failed".to_owned()),
            error: Some("terminal failure".to_owned()),
        },
    );
    let receipt = GENERATION_DISCOVERY_RECEIPTS.get(&key).expect("receipt");
    assert_ne!(
        receipt.state,
        WorkspaceGenerationDiscoveryState::Discovering
    );
    drop(receipt);
    GENERATION_DISCOVERY_RECEIPTS.remove(&key);
}

#[tokio::test]
async fn cancellation_releases_the_generation_discovery_single_flight_key() {
    static KEY_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = KEY_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let key = GenerationDiscoveryKey {
        workspace_identity: format!(
            "cancelled-generation-discovery-{}-{sequence}",
            std::process::id()
        ),
        project_root: PathBuf::from(format!(
            "/runtime-server-generation-discovery-test-{}-{sequence}",
            std::process::id()
        )),
    };
    assert!(GENERATION_DISCOVERY_TASKS.insert(key.clone()));
    let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
    let task_key = key.clone();
    let task = tokio::spawn(async move {
        let _guard = GenerationDiscoveryTaskGuard {
            key: task_key,
            finished: false,
        };
        let _ = started_sender.send(());
        std::future::pending::<()>().await;
    });
    started_receiver
        .await
        .expect("discovery task should publish its started boundary");

    task.abort();
    let cancelled = task
        .await
        .expect_err("aborted discovery task must report cancellation");
    assert!(cancelled.is_cancelled());
    assert!(
        !GENERATION_DISCOVERY_TASKS.contains(&key),
        "cancelled discovery must release its single-flight key"
    );
}
