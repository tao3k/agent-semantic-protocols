// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

use std::path::Path;

use agent_semantic_artifacts::RuntimeArtifactStateLayout;

#[test]
fn mutable_runtime_artifact_state_has_one_physical_authority() {
    let state_home = Path::new("/tmp/asp-state-layout-contract");
    let layout = RuntimeArtifactStateLayout::new(state_home);

    assert_eq!(layout.root(), state_home.join("runtime/artifacts"));
    assert_eq!(layout.generation_store(), layout.root().join("generations"));
    assert_eq!(layout.active_slot(), layout.root().join("active"));
    assert_eq!(layout.healthy_slot(), layout.root().join("healthy"));
    assert_eq!(layout.activation(), layout.root().join("activation"));
    assert_eq!(layout.leases(), layout.root().join("leases"));

    for path in [
        layout.generation_store(),
        layout.active_slot(),
        layout.healthy_slot(),
        layout.activation(),
        layout.leases(),
    ] {
        assert!(path.starts_with(layout.root()));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_publication_uses_only_the_canonical_artifact_authority() {
    let temporary = tempfile::tempdir().expect("temporary state home");
    let state_home = temporary.path().join("state");
    let source = temporary.path().join("asp-candidate");
    let target = state_home.join("runtime/bin/asp");
    std::fs::write(&source, b"runtime-candidate-v1").expect("candidate bytes");

    agent_semantic_artifacts::runtime_artifact_publication::publish_runtime_artifact(
        &state_home,
        &source,
        &target,
        "test",
    )
    .await
    .expect("publish Runtime artifact");

    let layout = RuntimeArtifactStateLayout::new(&state_home);
    assert!(layout.generation_store().is_dir());
    assert!(!layout.root().join("blake3-256").exists());
    assert!(!layout.root().join("bundles").exists());
    assert!(layout.active_slot().exists());
    let active_bundle = std::fs::read_link(layout.active_slot()).expect("active bundle selector");
    assert!(
        active_bundle.join("asp").exists(),
        "active bundle member must survive retention"
    );
    assert!(layout.pending_activation().is_file());
    assert!(layout.leases().is_dir());
    assert!(
        !layout.publication_lease().exists(),
        "successful publication must consume its quiescence lease"
    );
}
