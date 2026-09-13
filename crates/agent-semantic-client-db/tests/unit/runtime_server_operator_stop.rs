// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn operator_stop_admits_only_a_distinct_content_bound_publication() {
    let temporary = tempfile::tempdir().expect("operator-stop state root");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp");
    let target = state_home.join("runtime/bin/asp");
    std::fs::write(&source, "#!/bin/sh\nexit 0\n").expect("write executable fixture");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("mark fixture executable");
    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        "dev",
    )
    .await
    .expect("publish stopped content identity");
    let stopped = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        &state_home,
    )
    .await
    .expect("read stopped publication")
    .expect("pending stopped publication");

    agent_semantic_client_db::runtime_server_lifecycle::mark_operator_stopped(&state_home)
        .await
        .expect("publish operator-stop authority");
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await
            .expect("read operator-stop authority")
    );
    assert!(
        !agent_semantic_client_db::runtime_server_lifecycle::admit_activation_after_operator_stop(
            &state_home,
            &stopped.artifact_digest,
            &stopped.publication_nonce,
        )
        .await
        .expect("reject the exact stopped publication")
    );
    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        "dev",
    )
    .await
    .expect("republish with a distinct nonce");
    let distinct = agent_semantic_artifacts::runtime_artifact_activation::read_runtime_artifact_activation_event(
        &state_home,
    )
    .await
    .expect("read distinct publication")
    .expect("pending distinct publication");
    assert_ne!(distinct.publication_nonce, stopped.publication_nonce);
    assert!(
        agent_semantic_client_db::runtime_server_lifecycle::admit_activation_after_operator_stop(
            &state_home,
            &distinct.artifact_digest,
            &distinct.publication_nonce,
        )
        .await
        .expect("admit a distinct content-bound publication")
    );
    assert!(
        !agent_semantic_client_db::runtime_server_lifecycle::operator_stopped(&state_home)
            .await
            .expect("new activation clears operator-stop authority")
    );
}
