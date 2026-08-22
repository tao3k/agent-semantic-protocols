use std::path::Path;
use std::time::Duration;

use crate::runtime_identity_monitor::{
    MonitorAction, RuntimeIdentityMonitor, spawn_runtime_identity_monitor_with_intervals,
};

#[test]
fn same_identity_is_a_noop_after_ready() {
    let mut monitor = RuntimeIdentityMonitor::default();
    assert_eq!(monitor.observe("a"), MonitorAction::BeginDrain);
    assert_eq!(monitor.drain_completed(), MonitorAction::SpawnLatest);
    assert_eq!(monitor.observe("a"), MonitorAction::Noop);
}

#[test]
fn latest_identity_wins_during_drain() {
    let mut monitor = RuntimeIdentityMonitor::default();
    assert_eq!(monitor.observe("a"), MonitorAction::BeginDrain);
    assert_eq!(monitor.observe("b"), MonitorAction::Noop);
    assert_eq!(monitor.observe("c"), MonitorAction::Noop);
    assert_eq!(monitor.drain_completed(), MonitorAction::SpawnLatest);
}

async fn write_identity(state_home: &Path, value: &str) {
    let path = state_home.join("runtime/artifact-identities/asp.json");
    tokio::fs::create_dir_all(path.parent().expect("identity parent"))
        .await
        .expect("create identity parent");
    tokio::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-artifact-identity",
            "schemaVersion": "1",
            "artifactKind": "asp",
            "artifactMode": "dev",
            "stablePath": state_home.join("runtime/bin/asp"),
            "sourcePath": state_home.join("target/debug/asp"),
            "artifactDigest": value,
            "identityKind": "developer-source-generation",
            "identityValue": value,
            "identityAlgorithm": "blake3-metadata-v1"
        }))
        .expect("encode identity"),
    )
    .await
    .expect("write identity");
}

#[tokio::test]
async fn identity_change_emits_once_without_waiting_for_owner_exit() {
    let root = tempfile::tempdir().expect("tempdir");
    write_identity(root.path(), "generation-a").await;
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        "asp".to_owned(),
        7,
        Duration::from_millis(5),
        Duration::from_secs(1),
    );
    tokio::time::sleep(Duration::from_millis(15)).await;
    write_identity(root.path(), "generation-b").await;
    let event = tokio::time::timeout(Duration::from_millis(100), monitor.next_event())
        .await
        .expect("identity event deadline")
        .expect("identity event");
    assert!(event.previous_identity.contains("generation-a"));
    assert!(event.observed_identity.contains("generation-b"));
    monitor.shutdown().await;
}

#[tokio::test]
async fn unchanged_identity_does_not_emit_or_hot_write() {
    let root = tempfile::tempdir().expect("tempdir");
    write_identity(root.path(), "generation-a").await;
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        "asp".to_owned(),
        8,
        Duration::from_millis(5),
        Duration::from_secs(1),
    );
    tokio::time::sleep(Duration::from_millis(20)).await;
    let monitor_receipt = root.path().join("runtime/server/monitor-state.json");
    let before = tokio::fs::read(&monitor_receipt)
        .await
        .expect("initial monitor receipt");
    assert!(
        tokio::time::timeout(Duration::from_millis(25), monitor.next_event())
            .await
            .is_err()
    );
    let after = tokio::fs::read(monitor_receipt)
        .await
        .expect("unchanged monitor receipt");
    assert_eq!(before, after);
    monitor.shutdown().await;
}

#[tokio::test]
async fn cancellation_terminates_monitor_task() {
    let root = tempfile::tempdir().expect("tempdir");
    let monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        "asp".to_owned(),
        9,
        Duration::from_secs(1),
        Duration::from_secs(5),
    );
    tokio::time::timeout(Duration::from_millis(100), monitor.shutdown())
        .await
        .expect("monitor cancellation deadline");
}
