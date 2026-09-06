// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use agent_semantic_client_db::runtime_server_control::acquire_runtime_server_election;
use agent_semantic_client_db::runtime_server_control::runtime_server_endpoint_path;
use agent_semantic_client_db::runtime_server_control::runtime_server_runtime_base;

#[test]
fn canonical_state_home_owns_runtime_transport_publication() {
    let fixture = tempfile::tempdir().expect("create State Home transport fixture");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create canonical State Home");
    let canonical = std::fs::canonicalize(&state_home).expect("canonical State Home");
    let expected_base = canonical.join("runtime/serving");

    assert_eq!(
        runtime_server_runtime_base(&state_home).expect("resolve Runtime serving base"),
        expected_base
    );
    assert_eq!(
        runtime_server_endpoint_path(&state_home).expect("resolve Runtime endpoint publication"),
        canonical.join("runtime/serving/endpoint.v1.json")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn election_is_singleton_per_state_home_and_isolated_across_state_homes() {
    let fixture = tempfile::tempdir().expect("create State Home isolation fixture");
    let state_home_a = fixture.path().join("state-a");
    let state_home_b = fixture.path().join("state-b");
    std::fs::create_dir_all(&state_home_a).expect("create first State Home");
    std::fs::create_dir_all(&state_home_b).expect("create second State Home");

    let runtime_base_a =
        runtime_server_runtime_base(&state_home_a).expect("resolve first State Home runtime base");
    let runtime_base_b =
        runtime_server_runtime_base(&state_home_b).expect("resolve second State Home runtime base");
    assert_ne!(runtime_base_a, runtime_base_b);
    assert_eq!(
        runtime_base_a,
        std::fs::canonicalize(&state_home_a)
            .expect("canonical first State Home")
            .join("runtime/serving")
    );
    assert_eq!(
        runtime_base_b,
        std::fs::canonicalize(&state_home_b)
            .expect("canonical second State Home")
            .join("runtime/serving")
    );

    let election_a = acquire_runtime_server_election(&state_home_a)
        .await
        .expect("acquire first State Home election");
    let election_b = acquire_runtime_server_election(&state_home_b)
        .await
        .expect("acquire isolated second State Home election");
    let duplicate_a = acquire_runtime_server_election(&state_home_a).await;
    assert!(
        duplicate_a.is_err(),
        "one State Home must admit only one Runtime Server owner"
    );

    drop(election_a);
    drop(election_b);
    tokio::fs::remove_dir_all(runtime_base_a)
        .await
        .expect("remove first isolated runtime base");
    tokio::fs::remove_dir_all(runtime_base_b)
        .await
        .expect("remove second isolated runtime base");
}

#[test]
fn canonical_aliases_share_one_state_home_identity() {
    let fixture = tempfile::tempdir().expect("create State Home alias fixture");
    let state_home = fixture.path().join("state");
    std::fs::create_dir_all(&state_home).expect("create canonical State Home");
    let alias = fixture.path().join("state-alias");
    std::os::unix::fs::symlink(&state_home, &alias).expect("create State Home alias");

    assert_eq!(
        runtime_server_runtime_base(&state_home).expect("resolve canonical State Home"),
        runtime_server_runtime_base(&alias).expect("resolve aliased State Home"),
    );
}
