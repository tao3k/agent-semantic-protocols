// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Explicit validation helpers for ASP Rust member harness policy.

use std::collections::BTreeSet;

use crate::member_policy::asp_workspace_member_forbidden_normal_dependencies;
use crate::member_policy::asp_workspace_member_policy_for;

/// Constant-time receipt for one registered member policy.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AspRustProjectHarnessMemberPolicyReceipt {
    pub schema_id: &'static str,
    pub schema_version: &'static str,
    pub package_name: String,
    pub crate_root: String,
    pub policy_digest: String,
}

/// Assert one package atom selected by Cargo's dependency DAG.
///
/// Every participating member build script calls this thin API exactly once.
/// It validates only package identity and manifest-level owner boundaries; the
/// explicit workspace-policy gate owns the one full source scan for Cargo's DAG.
pub fn assert_asp_rust_project_harness_member_policy(
    package_name: &str,
    project_root: &std::path::Path,
) -> Result<AspRustProjectHarnessMemberPolicyReceipt, String> {
    let member_policy = asp_workspace_member_policy_for(package_name);
    if member_policy.is_some_and(|policy| !project_root.ends_with(policy.crate_root)) {
        let expected_root = member_policy.expect("registered member policy").crate_root;
        return Err(format!(
            "ASP Rust harness member policy root mismatch for {package_name}: expectedSuffix={} observed={}",
            expected_root,
            project_root.display(),
        ));
    }
    let manifest_path = project_root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "read ASP Rust harness member manifest {}: {error}",
            manifest_path.display(),
        )
    })?;
    for dependency in asp_workspace_member_forbidden_normal_dependencies(package_name) {
        if manifest_declares_dependency(&manifest, dependency, &["dependencies"]) {
            return Err(format!(
                "ASP Rust harness member {package_name} violates its owner boundary: direct normal dependency {dependency} is forbidden",
            ));
        }
    }
    for full_harness_package in ["asp-rust", "rust-lang-project-harness"] {
        if manifest_declares_dependency(
            &manifest,
            full_harness_package,
            &["dependencies", "build-dependencies"],
        ) {
            return Err(format!(
                "ASP Rust harness member {package_name} must not compile the full ASP Rust scanner from normal or build dependencies: {full_harness_package}; use the shared Build Support dependency and run full verification once from the workspace gate",
            ));
        }
    }
    let policy_digest = member_policy.map_or_else(
        || {
            format!(
                "blake3-256:{}",
                blake3::hash(format!("asp-rust.default-package-policy:{package_name}").as_bytes())
                    .to_hex()
            )
        },
        |policy| policy.contract_digest(),
    );
    Ok(AspRustProjectHarnessMemberPolicyReceipt {
        schema_id: "agent.semantic-protocols.rust-harness-member-build-receipt",
        schema_version: "1",
        package_name: package_name.to_owned(),
        crate_root: member_policy.map_or_else(
            || project_root.display().to_string(),
            |policy| policy.crate_root.to_owned(),
        ),
        policy_digest,
    })
}

/// Assert the complete source policy for the Cargo package that owns the
/// calling build script.
///
/// This is the ordinary member `build.rs` entrypoint. The default Build
/// Support feature compiles one shared ASP Rust scanner and evaluates only the
/// current package root. The lower-level manifest receipt remains available
/// through [`assert_asp_rust_project_harness_member_policy`].
#[cfg(feature = "workspace-policy")]
pub fn assert_asp_rust_project_harness_member_policy_from_env() -> asp_rust::AspRustReport {
    crate::workspace_policy::assert_asp_rust_project_harness_member_source_policy_from_env()
}

/// Assert only the manifest identity when the caller deliberately disables
/// the default source-policy feature.
#[cfg(not(feature = "workspace-policy"))]
pub fn assert_asp_rust_project_harness_member_policy_from_env()
-> AspRustProjectHarnessMemberPolicyReceipt {
    let package_name = std::env::var("CARGO_PKG_NAME")
        .expect("CARGO_PKG_NAME is required for the ASP Rust package policy");
    let crate_root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .expect("CARGO_MANIFEST_DIR is required for the ASP Rust package policy");
    assert_asp_rust_project_harness_member_policy(&package_name, &crate_root)
        .unwrap_or_else(|error| panic!("{error}"))
}

fn manifest_declares_dependency(manifest: &str, dependency: &str, scopes: &[&str]) -> bool {
    let scopes = scopes.iter().copied().collect::<BTreeSet<_>>();
    let mut matched_dependency_section = false;
    manifest.lines().any(|line| {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            let dependency_scope = section.rsplit('.').next().unwrap_or(section);
            matched_dependency_section = scopes.contains(section)
                || (section.starts_with("target.") && scopes.contains(dependency_scope));
            return false;
        }
        if !matched_dependency_section || line.is_empty() || line.starts_with('#') {
            return false;
        }
        let Some((key, _)) = line.split_once('=') else {
            return false;
        };
        key.trim().trim_matches('"') == dependency
    })
}
