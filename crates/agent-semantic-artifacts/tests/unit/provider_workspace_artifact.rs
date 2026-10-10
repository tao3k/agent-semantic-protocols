// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{materialize_provider_workspace_artifact, provider_workspace_artifact_snapshot};

#[test]
fn materialized_tree_preserves_complete_merkle_identity() {
    let fixture = tempfile::tempdir().expect("fixture");
    let source = fixture.path().join("source");
    let target = fixture.path().join("target");
    std::fs::create_dir_all(source.join("bin")).expect("source tree");
    std::fs::write(source.join("bin/provider"), b"provider-v1").expect("provider");
    std::fs::write(source.join("metadata.json"), b"{\"v\":1}").expect("metadata");

    let expected = provider_workspace_artifact_snapshot(&source).expect("source identity");
    materialize_provider_workspace_artifact(&source, &target).expect("materialize");
    let actual = provider_workspace_artifact_snapshot(&target).expect("target identity");

    assert_eq!(actual, expected);
    assert_eq!(actual.1, 2);
}

#[test]
fn content_mutation_changes_provider_artifact_identity() {
    let fixture = tempfile::tempdir().expect("fixture");
    let artifact = fixture.path().join("provider");
    std::fs::write(&artifact, b"provider-v1").expect("provider");
    let before = provider_workspace_artifact_snapshot(&artifact).expect("before");
    std::fs::write(&artifact, b"provider-v2").expect("mutate");
    let after = provider_workspace_artifact_snapshot(&artifact).expect("after");
    assert_ne!(after.0, before.0);
}
