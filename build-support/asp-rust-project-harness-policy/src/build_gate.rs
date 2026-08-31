//! Build-time activation helpers for ASP Rust member harness policy.

use std::path::{Path, PathBuf};

use crate::assert_rust_project_harness_downstream_policy_with_authority;
use crate::member_policy::asp_workspace_member_policy_for;
use crate::{RustProjectHarnessBuildGateAuthority, RustProjectHarnessDownstreamPolicy};

/// Applies the registered ASP Rust member policy for `package_name` from `build.rs`.
pub fn assert_asp_rust_project_harness_member_policy_from_env(package_name: &str) {
    let member_policy = asp_workspace_member_policy_for(package_name).unwrap_or_else(|| {
        panic!("no ASP Rust project harness member policy registered for {package_name}")
    });
    let harness_config = member_policy.to_harness_config();
    let verification_label = member_policy.verification_label.unwrap_or(package_name);
    let downstream_policy =
        RustProjectHarnessDownstreamPolicy::new(verification_label, harness_config);
    let authority =
        member_build_gate_authority_from_cargo_env(member_policy).unwrap_or_else(|error| {
            panic!("resolve ASP Rust harness member build authority for {package_name}: {error}")
        });
    let project_root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("CARGO_MANIFEST_DIR is required for {package_name}"));
    assert_rust_project_harness_downstream_policy_with_authority(
        &project_root,
        &downstream_policy,
        &authority,
    );
}

fn member_build_gate_authority_from_cargo_env(
    member_policy: &crate::member_policy::AspRustProjectHarnessMemberPolicy,
) -> Result<RustProjectHarnessBuildGateAuthority, String> {
    let out_dir = std::env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "Cargo OUT_DIR is required; implicit cache fallback is forbidden".to_string()
        })?;
    member_build_gate_authority(member_policy, &out_dir)
}

fn member_build_gate_authority(
    member_policy: &crate::member_policy::AspRustProjectHarnessMemberPolicy,
    cargo_out_dir: &Path,
) -> Result<RustProjectHarnessBuildGateAuthority, String> {
    RustProjectHarnessBuildGateAuthority::new(
        cargo_out_dir
            .join("asp-rust-project-harness")
            .join("member-policy-cache"),
        member_policy.contract_digest(),
    )
}
