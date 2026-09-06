// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Thin ASP workspace policy adapter over the ASP Rust Build DAG API.

use std::path::Path;

use crate::build_gate::assert_asp_rust_project_harness_member_policy;
use crate::member_policy::asp_workspace_member_policy_for;

/// Validate the Cargo-derived identity of the workspace owning this build script.
///
/// This is the thin entrypoint for an external workspace. It parses no Cargo
/// manifest locally and performs no source scan.
#[track_caller]
pub fn assert_asp_workspace_build_identity_from_env() -> asp_rust::AspRustWorkspaceBuildDag {
    let build_dag =
        asp_rust::asp_rust_workspace_build_dag_from_env(&asp_rust::default_asp_rust_config())
            .unwrap_or_else(|error| panic!("derive ASP Rust workspace Build DAG: {error}"));
    for package in &build_dag.packages {
        if asp_workspace_member_policy_for(&package.package_name).is_some() {
            assert_asp_rust_project_harness_member_policy(
                &package.package_name,
                &package.package_root,
            )
            .unwrap_or_else(|error| panic!("{error}"));
        }
    }
    build_dag
}

/// Execute the complete ASP workspace policy once per Cargo package atom.
///
/// ASP Rust owns Cargo manifest parsing, membership, dependency ordering,
/// duplicate elimination, and content-addressed cache admission. This adapter
/// supplies only the package-local declarative policy projection.
#[track_caller]
pub fn assert_asp_workspace_policy(workspace_root: &Path) -> asp_rust::AspRustWorkspaceRunReport {
    let workspace_policy = asp_rust::AspRustWorkspacePolicy::new(
        "agent-semantic-protocols",
        asp_rust::default_asp_rust_config(),
    );
    asp_rust::assert_asp_rust_workspace_policy_with(
        workspace_root,
        &workspace_policy,
        |package_name, config| match asp_workspace_member_policy_for(package_name) {
            Some(policy) => policy.apply_to_asp_rust_config(config),
            None => config,
        },
    )
}

/// Execute the complete policy for the workspace owning this explicit gate.
#[track_caller]
pub fn assert_asp_workspace_policy_from_env() -> asp_rust::AspRustWorkspaceRunReport {
    let build_dag = assert_asp_workspace_build_identity_from_env();
    assert_asp_workspace_policy(&build_dag.workspace_root)
}
