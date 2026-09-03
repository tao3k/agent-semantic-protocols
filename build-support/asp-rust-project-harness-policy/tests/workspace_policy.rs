//! Explicit whole-workspace ASP Rust policy gate.

#![cfg(feature = "workspace-policy")]

#[test]
fn asp_workspace_source_policy_is_clean() {
    let _receipt = asp_rust_project_harness_policy::assert_asp_workspace_policy_from_env();
}
