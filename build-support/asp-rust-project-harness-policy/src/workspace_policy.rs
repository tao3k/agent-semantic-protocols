// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Thin ASP workspace policy adapter over the ASP Rust Build DAG API.

use std::path::Path;

use crate::build_gate::assert_asp_rust_project_harness_member_policy;
use crate::member_policy::asp_workspace_member_policy_for;

/// Evaluate the complete parser-owned source policy for one registered member.
///
/// Unlike the constant-time manifest receipt, this is the package-local source
/// scan that produces modularity, agent-policy, and project-policy findings.
pub fn evaluate_asp_rust_project_harness_member_source_policy(
    package_name: &str,
    project_root: &Path,
) -> Result<asp_rust::AspRustReport, String> {
    let config = asp_workspace_member_policy_for(package_name)
        .map_or_else(asp_rust::default_asp_rust_config, |policy| {
            policy.apply_to_asp_rust_config(asp_rust::default_asp_rust_config())
        });
    asp_rust::run_asp_rust_with_config_for_scope(
        project_root,
        &config,
        asp_rust::AspRustRunScope::Package,
    )
}

/// Assert the complete parser-owned source policy for the Cargo package whose
/// `build.rs` is currently executing.
///
/// The ASP Rust downstream gate owns content-addressed caching and emits
/// `cargo:rerun-if-changed` for every scanned input, so ordinary package builds
/// cannot retain a manifest-only policy receipt after source changes.
#[track_caller]
pub fn assert_asp_rust_project_harness_member_source_policy_from_env() -> asp_rust::AspRustReport {
    let package_name = std::env::var("CARGO_PKG_NAME")
        .expect("CARGO_PKG_NAME is required for the ASP Rust member source policy");
    let config = asp_workspace_member_policy_for(&package_name)
        .map_or_else(asp_rust::default_asp_rust_config, |policy| {
            policy.apply_to_asp_rust_config(asp_rust::default_asp_rust_config())
        });
    let policy = asp_rust::AspRustDownstreamPolicy::new(
        format!("agent-semantic-protocols::{package_name}"),
        config,
    );
    let project_root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .expect("CARGO_MANIFEST_DIR is required for the ASP Rust member source policy");
    let report = asp_rust::evaluate_asp_rust_downstream_policy(&project_root, &policy);
    emit_member_policy_warnings(&report);
    let errors = report
        .findings
        .iter()
        .filter(|finding| finding.severity == asp_rust::RustDiagnosticSeverity::Error)
        .map(|finding| format!("[{}] {}", finding.rule_id, finding.summary))
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "ASP Rust member source policy errors:\n{}",
        errors.join("\n")
    );
    report
}

fn emit_member_policy_warnings(report: &asp_rust::AspRustReport) {
    for finding in report
        .findings
        .iter()
        .filter(|finding| finding.severity == asp_rust::RustDiagnosticSeverity::Warning)
    {
        let location = finding.location.path.as_ref().map_or_else(
            || "<no-location>".to_owned(),
            |path| path.display().to_string(),
        );
        println!(
            "cargo:warning=[{}] {} @ {}: {}",
            finding.rule_id, finding.title, location, finding.summary
        );
    }
}

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
