//! Registers Git-tracked hook matcher and resident-session replay scenarios.

use crate::AspRustProjectHarnessScenarioPackage;

/// Package that owns the canonical ASP hook scenarios.
pub const ASP_HOOK_SCENARIO_PACKAGE_NAME: &str = "asp-hook-v1";
/// Scenario that verifies resident dispatch for arbitrary wrappers around Cargo test.
pub const GENERIC_WRAPPER_TESTING_RESIDENT_DISPATCH_SCENARIO_ID: &str =
    "generic-wrapper-testing-resident-dispatch";

/// Git-tracked hook/session scenarios consumed by the Rust harness policy.
#[must_use]
pub fn asp_hook_scenario_package() -> AspRustProjectHarnessScenarioPackage {
    crate::asp_rust_project_harness_scenario_package!(
        package: ASP_HOOK_SCENARIO_PACKAGE_NAME,
        scenarios: [
            crate::asp_rust_project_harness_scenario!(
                name: GENERIC_WRAPPER_TESTING_RESIDENT_DISPATCH_SCENARIO_ID,
                package: ASP_HOOK_SCENARIO_PACKAGE_NAME,
                description: "An arbitrary wrapper around cargo test routes through the configured asp-testing resident interactive loop.",
                fixture_root: "crates/agent-semantic-hook/tests/fixtures/scenarios/generic_wrapper_testing_resident_dispatch",
                tags: ["hook", "command-match", "session", "resident-dispatch", "performance"],
                commands: [
                    {
                        label: "command-match-snapshot",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-command-match",
                            "--test",
                            "integration_test",
                            "wrapper_match_enable_accepts_arbitrary_wrapper_names",
                            "--",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "hook-session-snapshot",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-hook",
                            "--test",
                            "unit_test",
                            "generic_wrapper_testing_resident_dispatch_matches_git_snapshot",
                            "--",
                            "--nocapture",
                        ]
                    },
                    {
                        label: "wrapper-match-performance-gate",
                        argv: [
                            "cargo",
                            "test",
                            "-p",
                            "agent-semantic-command-match",
                            "--test",
                            "integration_test",
                            "wrapped_command_match_stays_within_git_snapshot_budget",
                            "--",
                            "--nocapture",
                        ]
                    },
                ],
            ),
        ],
    )
}
