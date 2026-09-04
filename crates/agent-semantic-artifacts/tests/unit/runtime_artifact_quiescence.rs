use super::Blake3ContentDigest;
use super::RuntimeArtifactQuiescenceLease;
use super::prepare_runtime_artifact_quiescence_lease;
use super::producer_process_started_at_unix_millis;
use super::runtime_artifact_quiescence_lease_path;
use crate::runtime_artifact_retention::RuntimeArtifactMutationGuard;

fn digest(byte: char) -> Blake3ContentDigest {
    Blake3ContentDigest::parse(&format!("blake3-256:{}", byte.to_string().repeat(64)))
        .expect("typed digest")
}

fn artifact_guard(state_home: &std::path::Path) -> RuntimeArtifactMutationGuard {
    RuntimeArtifactMutationGuard::try_acquire(&state_home.join("runtime/artifacts"))
        .expect("canonical Artifact mutation guard")
}

#[test]
fn dead_owner_lease_is_replaced_under_the_single_artifact_guard() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let guard = artifact_guard(temporary.path());
    let stale = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:previous",
        &digest('a'),
        &guard,
    )
    .expect("prepare stale lease");
    let stale_nonce = stale.lease.lease_nonce.clone();
    let path = runtime_artifact_quiescence_lease_path(temporary.path());
    let mut dead: RuntimeArtifactQuiescenceLease =
        serde_json::from_slice(&std::fs::read(&path).expect("read stale lease"))
            .expect("decode stale lease");
    dead.producer_process_id = u32::MAX;
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&dead).expect("encode dead-owner lease"),
    )
    .expect("publish dead-owner lease");

    let replacement = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:asp",
        &digest('b'),
        &guard,
    )
    .expect("replace dead-owner lease");

    assert_eq!(replacement.lease.operation, "publish:asp");
    assert_eq!(replacement.lease.artifact_digest, digest('b'));
    assert_ne!(replacement.lease.lease_nonce, stale_nonce);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn reused_pid_does_not_preserve_a_lease_created_before_that_process_started() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let guard = artifact_guard(temporary.path());
    let stale = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:previous",
        &digest('a'),
        &guard,
    )
    .expect("prepare stale lease");
    let path = runtime_artifact_quiescence_lease_path(temporary.path());
    let mut reused: RuntimeArtifactQuiescenceLease =
        serde_json::from_slice(&std::fs::read(&path).expect("read stale lease"))
            .expect("decode stale lease");
    reused.producer_process_id = std::process::id();
    let current_process_started_at = producer_process_started_at_unix_millis(std::process::id())
        .expect("current process start identity");
    reused.created_at_unix_millis = current_process_started_at.saturating_sub(1);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&reused).expect("encode reused-PID lease"),
    )
    .expect("publish reused-PID lease");

    let replacement = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:asp",
        &digest('b'),
        &guard,
    )
    .expect("replace lease owned by an earlier process instance");

    assert_eq!(replacement.lease.operation, "publish:asp");
    assert_eq!(replacement.lease.artifact_digest, digest('b'));
    assert_ne!(replacement.lease.lease_nonce, stale.lease.lease_nonce);
}

#[test]
fn live_competing_owner_identity_mismatch_is_fail_closed() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let guard = artifact_guard(temporary.path());
    let prepared = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:previous",
        &digest('a'),
        &guard,
    )
    .expect("prepare existing lease");

    let error = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:asp",
        &digest('b'),
        &guard,
    )
    .expect_err("live competing producer must remain fail-closed");

    assert!(error.contains("reasonKind=runtime-artifact-quiescence-live-owner-conflict"));
    assert!(error.contains(&format!(
        "producerProcessId={}",
        prepared.lease.producer_process_id
    )));
}

#[test]
fn same_process_same_identity_replay_reuses_the_lease() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let guard = artifact_guard(temporary.path());
    let first = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:asp",
        &digest('a'),
        &guard,
    )
    .expect("prepare first lease");
    let replay = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:asp",
        &digest('a'),
        &guard,
    )
    .expect("admit exact same-process replay");

    assert_eq!(replay.lease, first.lease);
}

#[test]
fn crash_after_consume_can_restore_then_finalize_exactly_once() {
    let temporary = tempfile::tempdir().expect("temporary state");
    let guard = artifact_guard(temporary.path());
    let prepared = prepare_runtime_artifact_quiescence_lease(
        temporary.path(),
        "publish:asp",
        &digest('a'),
        &guard,
    )
    .expect("prepare lease");
    let consumed = prepared
        .consume_under_artifact_guard(&guard)
        .expect("consume before simulated crash");
    prepared
        .restore_after_failed_commit(&consumed, &guard)
        .expect("restore after simulated failed commit");
    let consumed = prepared
        .consume_under_artifact_guard(&guard)
        .expect("consume restored lease");
    prepared
        .finish_consumption(&consumed, &guard)
        .expect("finalize successful retry");

    assert!(!runtime_artifact_quiescence_lease_path(temporary.path()).exists());
    assert!(prepared.consume_under_artifact_guard(&guard).is_err());
}
