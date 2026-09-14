// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

#[test]
fn package_matrix_keeps_the_workspace_scanner_out_of_package_local_tests() {
    let workflow =
        std::fs::read_to_string(workspace_root().join(".github/workflows/rust-package-tests.yml"))
            .expect("read Rust package test workflow");

    assert!(
        workflow.contains("matrix.package != 'asp-rust-project-harness-policy'"),
        "ordinary all-feature package steps must exclude the workspace-policy adapter"
    );
    assert!(
        workflow.contains("cargo test -p asp-rust-project-harness-policy --no-default-features"),
        "the policy package matrix atom must test only its package-local API"
    );
    assert!(
        !workflow.contains("asp-rust-project-harness-policy --all-features"),
        "the package matrix must not trigger the full workspace scanner"
    );
}
