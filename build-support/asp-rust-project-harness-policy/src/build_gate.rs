//! Build-time activation helpers for ASP Rust member harness policy.

use crate::RustProjectHarnessDownstreamPolicy;
use crate::assert_rust_project_harness_downstream_policy_from_env;
use crate::member_policy::asp_workspace_member_policy_for;

/// Applies the registered ASP Rust member policy for `package_name` from `build.rs`.
pub fn assert_asp_rust_project_harness_member_policy_from_env(package_name: &str) {
    let member_policy = asp_workspace_member_policy_for(package_name).unwrap_or_else(|| {
        panic!("no ASP Rust project harness member policy registered for {package_name}")
    });
    let harness_config = member_policy
        .to_harness_config()
        .with_latency_sensitive_performance_owner(
            "src/lib.rs",
            "Package root owns the baseline ASP Rust harness performance contract",
        )
        .with_availability_stability_owner(
            "src/lib.rs",
            "Package root owns the baseline ASP Rust harness stability contract",
        );
    let downstream_policy = RustProjectHarnessDownstreamPolicy::new(package_name, harness_config);
    assert_rust_project_harness_downstream_policy_from_env(&downstream_policy);
}
