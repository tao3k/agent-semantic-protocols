use agent_semantic_runtime_server::{
    readiness::RuntimeServerReadinessListener, resident_publication::resident_readiness_root,
};
use std::os::unix::ffi::OsStrExt as _;

#[tokio::test]
async fn max_length_state_root_maps_to_stable_collision_resistant_readiness_endpoint() {
    let temporary = tempfile::tempdir().expect("temporary long Runtime root");
    let long_state_home = temporary
        .path()
        .join("a".repeat(80))
        .join("b".repeat(80))
        .join("c".repeat(80));
    let other_state_home = temporary
        .path()
        .join("a".repeat(80))
        .join("b".repeat(80))
        .join("d".repeat(80));
    tokio::fs::create_dir_all(&long_state_home)
        .await
        .expect("create long legal Runtime root");
    tokio::fs::create_dir_all(&other_state_home)
        .await
        .expect("create second long legal Runtime root");

    let first_root = resident_readiness_root(&long_state_home)
        .await
        .expect("derive bounded readiness root");
    let stable_root = resident_readiness_root(&long_state_home)
        .await
        .expect("derive stable readiness root");
    let isolated_root = resident_readiness_root(&other_state_home)
        .await
        .expect("derive isolated readiness root");
    let first = first_root.endpoint("request", "candidate-a");
    let stable = stable_root.endpoint("request", "candidate-a");
    let isolated_candidate = first_root.endpoint("request", "candidate-b");
    let isolated_state = isolated_root.endpoint("request", "candidate-a");

    assert_eq!(first, stable);
    assert_ne!(first, isolated_candidate);
    assert_ne!(first, isolated_state);
    assert!(first.as_path().as_os_str().as_bytes().len() <= 100);
    assert!(first_root.as_path().starts_with("/tmp"));

    let listener = RuntimeServerReadinessListener::bind_root(
        &first_root,
        "request".to_owned(),
        "candidate-a".to_owned(),
    )
    .await
    .unwrap_or_else(|error| {
        panic!(
            "bind bounded readiness endpoint path={} bytes={} error={error}",
            first.as_path().display(),
            first.as_path().as_os_str().as_bytes().len()
        )
    });
    assert_eq!(listener.endpoint(), first);
}
