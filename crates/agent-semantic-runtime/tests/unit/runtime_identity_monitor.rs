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
    let digest = blake3::hash(value.as_bytes()).to_hex().to_string();
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
            "sourceGeneration": format!("blake3-256:{}", "b".repeat(64)),
            "sourceGenerationAlgorithm": "filesystem-generation-v1",
            "artifactDigest": digest,
            "identityKind": "content",
            "identityValue": digest,
            "identityAlgorithm": "blake3-256"
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
    assert!(event.previous_identity.contains(":blake3-256"));
    assert!(event.observed_identity.contains(":blake3-256"));
    monitor.shutdown().await;
}

#[tokio::test]
async fn unchanged_identity_does_not_emit_or_hot_write() {
    let root = tempfile::tempdir().expect("tempdir");
    write_identity(root.path(), &format!("blake3-256:{}", "a".repeat(64))).await;
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        "asp".to_owned(),
        8,
        Duration::from_millis(5),
        Duration::from_secs(1),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
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

#[tokio::test]
async fn startup_overwrites_stale_receipt_with_current_starting_owner() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("runtime/server/monitor-state.json");
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(
        &path,
        serde_json::json!({"schemaVersion":"1","phase":"watching","ownerEpoch":3,"heartbeat":true})
            .to_string(),
    )
    .await
    .unwrap();
    let monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        "asp".to_owned(),
        44,
        Duration::from_secs(1),
        Duration::from_secs(5),
    );
    tokio::time::sleep(Duration::from_millis(10)).await;
    let value: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
    assert_eq!(value["phase"], "starting");
    assert_eq!(value["ownerEpoch"], 44);
    assert_eq!(value["heartbeat"], false);
    monitor.shutdown().await;
}

#[tokio::test]
async fn first_identity_tick_transitions_current_owner_to_watching() {
    let root = tempfile::tempdir().expect("tempdir");
    write_identity(root.path(), "generation-a").await;
    let monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        "asp".to_owned(),
        55,
        Duration::from_millis(5),
        Duration::from_secs(5),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    let path = root.path().join("runtime/server/monitor-state.json");
    let value: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(path).await.unwrap()).unwrap();
    assert_eq!(value["phase"], "watching");
    assert_eq!(value["ownerEpoch"], 55);
    assert_eq!(value["heartbeat"], true);
    monitor.shutdown().await;
}
