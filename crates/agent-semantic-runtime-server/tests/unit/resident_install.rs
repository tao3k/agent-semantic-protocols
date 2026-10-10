// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::os::unix::fs::PermissionsExt;

use agent_semantic_artifacts::runtime_artifact_retention::RuntimeArtifactMutationGuard;

use super::install_resident_runtime;

#[tokio::test]
async fn install_publishes_without_a_runtime_child_and_releases_the_lock() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("target-debug-asp");
    let target = temporary.path().join("bin/asp");
    tokio::fs::write(&source, b"#!/bin/sh\nexit 2\n")
        .await
        .expect("write candidate");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755))
        .expect("candidate permissions");

    let receipt = install_resident_runtime(&state_home, &source, &target, "dev", None)
        .await
        .expect("publication must not depend on candidate readiness");

    assert_eq!(receipt.status, "published-active-awaiting-health");
    let artifact_root = state_home.join("runtime/artifacts");
    let guard = RuntimeArtifactMutationGuard::try_acquire(&artifact_root)
        .expect("publication must release the artifact lock before return");
    drop(guard);
}
