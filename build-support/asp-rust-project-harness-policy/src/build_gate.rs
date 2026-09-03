//! Explicit validation helpers for ASP Rust member harness policy.

use crate::member_policy::{
    asp_workspace_member_forbidden_normal_dependencies, asp_workspace_member_policy_for,
};

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

/// Validate one downstream member without loading or running the full Harness.
///
/// This reads only the member manifest. It deliberately rejects the full
/// scanner as a normal or build dependency so a multi-crate Cargo graph cannot
/// multiply Harness compilation and source scans.
pub fn validate_asp_rust_project_harness_member_manifest(
    package_name: &str,
    project_root: &std::path::Path,
) -> Result<AspRustProjectHarnessMemberPolicyReceipt, String> {
    let member_policy = asp_workspace_member_policy_for(package_name).ok_or_else(|| {
        format!("no ASP Rust project harness member policy registered for {package_name}")
    })?;
    if !project_root.ends_with(member_policy.crate_root) {
        return Err(format!(
            "ASP Rust harness member policy root mismatch for {package_name}: expectedSuffix={} observed={}",
            member_policy.crate_root,
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
    Ok(AspRustProjectHarnessMemberPolicyReceipt {
        schema_id: "agent.semantic-protocols.rust-harness-member-build-receipt",
        schema_version: "1",
        package_name: package_name.to_owned(),
        crate_root: member_policy.crate_root.to_owned(),
        policy_digest: member_policy.contract_digest(),
    })
}

fn manifest_declares_dependency(manifest: &str, dependency: &str, scopes: &[&str]) -> bool {
    let mut matched_dependency_section = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            matched_dependency_section = scopes.iter().any(|scope| {
                section == *scope
                    || (section.starts_with("target.") && section.ends_with(&format!(".{scope}")))
            });
            continue;
        }
        if !matched_dependency_section || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        if key.trim().trim_matches('"') == dependency {
            return true;
        }
    }
    false
}
