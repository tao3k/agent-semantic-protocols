use super::discard_prepared_runtime_artifact;
use super::prepare_runtime_artifact_candidate;
use super::stage_runtime_artifact;

#[tokio::test]
async fn dev_candidate_is_digest_materialized_reused_and_removed_on_abort() {
    let temporary = tempfile::tempdir().expect("temporary artifact catalog");
    let state_home = temporary.path().join("state");
    let candidate_dir = state_home.join("runtime/artifacts/bundles/dev");
    std::fs::create_dir_all(&candidate_dir).expect("candidate directory");
    let source = temporary.path().join("target-debug-asp");
    std::fs::write(&source, b"immutable-dev-runtime-candidate").expect("dev Runtime source");

    let candidate = prepare_runtime_artifact_candidate(&state_home, &candidate_dir, &source)
        .await
        .expect("materialize candidate");
    assert!(!candidate.was_present);
    assert!(candidate.path.is_file());
    assert!(
        !std::fs::symlink_metadata(&candidate.path)
            .expect("artifact metadata")
            .file_type()
            .is_symlink()
    );

    let reused = prepare_runtime_artifact_candidate(&state_home, &candidate_dir, &source)
        .await
        .expect("reuse candidate");
    assert!(reused.was_present);
    discard_prepared_runtime_artifact(&reused)
        .await
        .expect("preserve reused artifact");
    assert!(candidate.path.exists());

    discard_prepared_runtime_artifact(&candidate)
        .await
        .expect("discard aborted candidate");
    assert!(!candidate.path.exists());
}

#[cfg(unix)]
#[test]
fn publication_snapshot_has_an_independent_inode() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("source");
    let staged = root.path().join("staged");
    std::fs::write(&source, b"artifact").expect("source");
    stage_runtime_artifact(&source, &staged).expect("stage");
    assert_ne!(
        std::fs::metadata(source).expect("source metadata").ino(),
        std::fs::metadata(staged).expect("staged metadata").ino()
    );
}

#[test]
fn publication_snapshot_preserves_bytes() {
    let root = tempfile::tempdir().expect("tempdir");
    let source = root.path().join("source");
    let staged = root.path().join("staged");
    std::fs::write(&source, b"artifact").expect("source");
    stage_runtime_artifact(&source, &staged).expect("snapshot");
    assert_eq!(
        std::fs::read(source).expect("source bytes"),
        std::fs::read(staged).expect("staged bytes")
    );
}

#[test]
fn staging_failure_preserves_active_and_healthy_slots() {
    let root = tempfile::tempdir().expect("tempdir");
    let active = root.path().join("active");
    let healthy = root.path().join("healthy");
    std::fs::write(&active, b"active").expect("active");
    std::fs::write(&healthy, b"healthy").expect("healthy");
    let result = stage_runtime_artifact(&root.path().join("missing"), &root.path().join("staged"));
    assert!(result.is_err());
    assert_eq!(std::fs::read(active).expect("active bytes"), b"active");
    assert_eq!(std::fs::read(healthy).expect("healthy bytes"), b"healthy");
}
