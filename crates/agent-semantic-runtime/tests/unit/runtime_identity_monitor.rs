use std::path::Path;
use std::time::Duration;

use crate::runtime_identity_monitor::MonitorAction;
use crate::runtime_identity_monitor::ResidentActivationIdentity;
use crate::runtime_identity_monitor::RuntimeIdentityMonitor;
use crate::runtime_identity_monitor::runtime_identity_poll_interval;
use crate::runtime_identity_monitor::spawn_runtime_identity_monitor_with_intervals;

#[test]
fn developer_monitor_is_millisecond_cadence_without_accelerating_release_polling() {
    assert_eq!(
        runtime_identity_poll_interval("dev"),
        Duration::from_millis(50)
    );
    assert_eq!(
        runtime_identity_poll_interval("release"),
        Duration::from_secs(5)
    );
}

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

async fn write_applied_activation(state_home: &Path, value: &str, publication_nonce: &str) {
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        value.as_bytes(),
    );
    let artifact_path = state_home
        .join("runtime/artifacts/blake3-256")
        .join(digest.content_digest().as_str())
        .join("asp");
    let path = state_home.join("runtime/activation/applied.json");
    tokio::fs::create_dir_all(path.parent().expect("identity parent"))
        .await
        .expect("create identity parent");
    tokio::fs::write(
        path,
        serde_json::to_vec(&serde_json::json!({
            "artifactDigest": digest,
            "artifactPath": artifact_path,
            "candidateSlotPath": state_home.join("runtime/resident/candidates/asp"),
            "previousArtifactDigest": null,
            "artifactMode": "dev",
            "publishedAtUnixMillis": 1,
            "publicationNonce": publication_nonce,
            "candidateIdentity": {
                "artifactDigest": digest,
                "artifactPath": artifact_path,
                "stablePath": state_home.join("runtime/bin/asp"),
                "artifactMode": "dev",
                "publicationNonce": publication_nonce,
            }
        }))
        .expect("encode identity"),
    )
    .await
    .expect("write identity");
}

async fn write_legacy_identity_receipt(state_home: &Path, value: &str) {
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        value.as_bytes(),
    );
    let path = state_home.join("runtime/artifact-identities/asp.json");
    tokio::fs::create_dir_all(path.parent().expect("legacy identity parent"))
        .await
        .expect("create legacy identity parent");
    tokio::fs::write(
        path,
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.runtime-artifact-identity",
            "schemaVersion": "1",
            "identityValue": digest,
        })
        .to_string(),
    )
    .await
    .expect("write legacy identity receipt");
}

#[tokio::test]
async fn identity_change_emits_once_without_waiting_for_owner_exit() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "generation-a", "publication-a").await;
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        7,
        None,
        Duration::from_millis(5),
        Duration::from_secs(1),
    );
    tokio::time::sleep(Duration::from_millis(15)).await;
    write_applied_activation(root.path(), "generation-b", "publication-b").await;
    let event = tokio::time::timeout(Duration::from_millis(100), monitor.next_event())
        .await
        .expect("identity event deadline")
        .expect("identity event");
    assert!(event.previous_identity.starts_with("digest:blake3-256:"));
    assert!(
        event
            .previous_identity
            .contains("publicationNonce:publication-a")
    );
    assert!(event.previous_identity.ends_with(" ownerEpoch:7"));
    assert!(event.observed_identity.starts_with("digest:blake3-256:"));
    assert!(
        event
            .observed_identity
            .contains("publicationNonce:publication-b")
    );
    assert!(event.observed_identity.ends_with(" ownerEpoch:7"));
    monitor.shutdown().await;
}

#[tokio::test]
async fn unchanged_identity_does_not_emit_or_hot_write() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "generation-a", "publication-a").await;
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        8,
        None,
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
        9,
        None,
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
        44,
        None,
        Duration::from_secs(1),
        Duration::from_secs(5),
    );
    tokio::time::sleep(Duration::from_millis(10)).await;
    let value: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.unwrap()).unwrap();
    assert_eq!(value["phase"], "applied-authority-unavailable");
    assert_eq!(value["observationError"], "applied-activation-absent");
    assert_eq!(value["ownerEpoch"], 44);
    assert_eq!(value["heartbeat"], false);
    monitor.shutdown().await;
}

#[tokio::test]
async fn first_identity_tick_transitions_current_owner_to_watching() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "generation-a", "publication-a").await;
    let monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        55,
        None,
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

#[tokio::test]
async fn change_before_first_tick_is_compared_with_the_running_owner_identity() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "generation-a", "publication-a").await;
    let running_digest =
        agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
            b"generation-a",
        );
    let running = ResidentActivationIdentity {
        publication_nonce: "publication-a".to_owned(),
        artifact_digest: running_digest,
        owner_epoch: 56,
    };
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        56,
        Some(running.clone()),
        Duration::from_millis(25),
        Duration::from_secs(5),
    );
    write_applied_activation(root.path(), "generation-b", "publication-b").await;
    let event = tokio::time::timeout(Duration::from_millis(100), monitor.next_event())
        .await
        .expect("identity event deadline")
        .expect("identity event");
    assert_eq!(
        event.previous_identity,
        format!(
            "digest:{} publicationNonce:{} ownerEpoch:{}",
            running.artifact_digest, running.publication_nonce, running.owner_epoch
        )
    );
    assert_ne!(event.observed_identity, event.previous_identity);
    monitor.shutdown().await;
}

#[tokio::test]
async fn previous_healthy_publication_cannot_retire_a_starting_active_owner() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "previous", "publication-previous").await;
    let running = ResidentActivationIdentity {
        publication_nonce: "publication-candidate".to_owned(),
        artifact_digest:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                b"candidate",
            ),
        owner_epoch: 57,
    };
    let applied = ResidentActivationIdentity {
        publication_nonce: "publication-previous".to_owned(),
        artifact_digest:
            agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                b"previous",
            ),
        owner_epoch: 57,
    };
    assert_ne!(running, applied);
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        57,
        Some(running),
        Duration::from_millis(5),
        Duration::from_secs(5),
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(40), monitor.next_event())
            .await
            .is_err(),
        "previous Healthy publication is startup convergence lag, not replacement authority"
    );
    monitor.shutdown().await;
}

#[tokio::test]
async fn legacy_developer_identity_drift_cannot_retire_the_applied_resident() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "resident", "publication-resident").await;
    write_legacy_identity_receipt(root.path(), "developer-source-drift").await;
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        b"resident",
    );
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        70,
        Some(ResidentActivationIdentity {
            publication_nonce: "publication-resident".to_owned(),
            artifact_digest: digest,
            owner_epoch: 70,
        }),
        Duration::from_millis(5),
        Duration::from_secs(5),
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(40), monitor.next_event())
            .await
            .is_err(),
        "developer/source receipt must not be a resident replacement authority"
    );
    monitor.shutdown().await;
}

#[tokio::test]
async fn same_digest_distinct_applied_publication_emits_one_replacement() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "resident", "publication-a").await;
    let digest = agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
        b"resident",
    );
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        71,
        Some(ResidentActivationIdentity {
            publication_nonce: "publication-a".to_owned(),
            artifact_digest: digest,
            owner_epoch: 71,
        }),
        Duration::from_millis(5),
        Duration::from_secs(5),
    );
    write_applied_activation(root.path(), "resident", "publication-b").await;
    let event = tokio::time::timeout(Duration::from_millis(100), monitor.next_event())
        .await
        .expect("publication replacement deadline")
        .expect("publication replacement event");
    assert!(
        event
            .previous_identity
            .contains("publicationNonce:publication-a")
    );
    assert!(
        event
            .observed_identity
            .contains("publicationNonce:publication-b")
    );
    assert!(
        monitor.next_event().await.is_none(),
        "one applied publication change must emit exactly once"
    );
    monitor.shutdown().await;
}

#[tokio::test]
async fn absent_or_malformed_applied_authority_never_drains_the_running_owner() {
    for malformed in [false, true] {
        let root = tempfile::tempdir().expect("tempdir");
        if malformed {
            let path = root.path().join("runtime/activation/applied.json");
            tokio::fs::create_dir_all(path.parent().expect("applied parent"))
                .await
                .expect("create applied parent");
            tokio::fs::write(path, b"{")
                .await
                .expect("write malformed applied authority");
        }
        let mut monitor = spawn_runtime_identity_monitor_with_intervals(
            root.path().to_path_buf(),
            72,
            Some(ResidentActivationIdentity {
                publication_nonce: "publication-resident".to_owned(),
                artifact_digest:
                    agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                        b"resident",
                    ),
                owner_epoch: 72,
            }),
            Duration::from_millis(5),
            Duration::from_secs(5),
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(40), monitor.next_event())
                .await
                .is_err(),
            "unavailable applied authority must not fabricate a replacement"
        );
        let receipt: serde_json::Value = serde_json::from_slice(
            &tokio::fs::read(root.path().join("runtime/server/monitor-state.json"))
                .await
                .expect("read monitor failure receipt"),
        )
        .expect("decode monitor failure receipt");
        assert_eq!(receipt["phase"], "applied-authority-unavailable");
        monitor.shutdown().await;
    }
}

#[tokio::test]
async fn pending_activation_is_not_an_observed_serving_identity() {
    let root = tempfile::tempdir().expect("tempdir");
    write_applied_activation(root.path(), "candidate", "publication-candidate").await;
    tokio::fs::rename(
        root.path().join("runtime/activation/applied.json"),
        root.path().join("runtime/activation/pending.json"),
    )
    .await
    .expect("publish pending-only activation");
    let mut monitor = spawn_runtime_identity_monitor_with_intervals(
        root.path().to_path_buf(),
        73,
        Some(ResidentActivationIdentity {
            publication_nonce: "publication-resident".to_owned(),
            artifact_digest:
                agent_semantic_artifacts::blake3_content_digest::Blake3ContentDigest::from_bytes(
                    b"resident",
                ),
            owner_epoch: 73,
        }),
        Duration::from_millis(5),
        Duration::from_secs(5),
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(40), monitor.next_event())
            .await
            .is_err(),
        "pending activation must be claimed by the activation actor, not the resident monitor"
    );
    monitor.shutdown().await;
}
