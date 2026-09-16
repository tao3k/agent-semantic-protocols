// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Runtime readiness socket identity integration tests.

use agent_semantic_runtime_server::readiness::RuntimeServerReadinessListener;
use agent_semantic_runtime_server::resident_publication::resident_readiness_root;
use std::os::unix::ffi::OsStrExt as _;

#[tokio::test]
async fn overlong_state_home_fails_closed_without_a_tmp_readiness_authority() {
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

    for state_home in [&long_state_home, &other_state_home] {
        let error = resident_readiness_root(state_home)
            .await
            .expect_err("overlong State Home must not escape to /tmp");
        assert!(
            error.contains("reasonKind=runtime-readiness-path-unrepresentable"),
            "unexpected readiness error: {error}"
        );
    }
}

#[tokio::test]
async fn readiness_endpoint_is_owned_by_the_canonical_state_home() {
    let state_home = tempfile::Builder::new()
        .prefix("asp-r-")
        .tempdir_in("/tmp")
        .expect("short Runtime State Home");
    let canonical_state_home = tokio::fs::canonicalize(state_home.path())
        .await
        .expect("canonical State Home");
    let root = resident_readiness_root(state_home.path())
        .await
        .expect("derive State Home readiness root");
    let endpoint = root.endpoint("request", "candidate-a");

    assert!(root.as_path().starts_with(&canonical_state_home));
    assert_eq!(
        root.as_path(),
        canonical_state_home.join("runtime/serving/readiness")
    );
    assert!(endpoint.as_path().as_os_str().as_bytes().len() <= 100);

    let listener = RuntimeServerReadinessListener::bind_root(
        &root,
        "request".to_owned(),
        "candidate-a".to_owned(),
    )
    .await
    .expect("bind State Home-owned readiness endpoint");
    assert_eq!(listener.endpoint(), endpoint);
}
