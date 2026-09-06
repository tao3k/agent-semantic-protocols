// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Release publication and CI own the one-shot Cargo-DAG workspace policy.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace root")
}

#[test]
fn release_and_developer_publication_share_the_single_workspace_policy_gate() {
    let justfile =
        std::fs::read_to_string(workspace_root().join("Justfile")).expect("read root Justfile");

    let release_recipe = "agent-tools-install-protocol bin_dir=\"\": check-rust-workspace-policy";
    assert!(
        justfile.contains(release_recipe),
        "release publication must depend on the one-shot workspace policy gate"
    );
    assert!(
        justfile.contains("agent-tools-install-protocol-debug: check-rust-workspace-policy"),
        "developer publication must admit the Cargo-derived workspace policy exactly once before publishing"
    );
    assert_eq!(
        justfile.matches("check-rust-workspace-policy:").count(),
        1,
        "the root workspace must own exactly one full ASP Rust policy recipe"
    );
}
