//! Registers Git-tracked Hook matcher and Host role-dispatch scenarios.

use asp_rust_project_harness_policy::AspRustProjectHarnessScenarioPackage;

pub const ASP_HOOK_SCENARIO_PACKAGE_NAME: &str = "asp-hook-v1";
pub const GENERIC_WRAPPER_TESTING_ROLE_DISPATCH_SCENARIO_ID: &str =
    "generic-wrapper-testing-role-dispatch";
pub const GIT_HISTORY_TESTING_DISPATCH_SCENARIO_ID: &str = "git-history-testing-dispatch";

#[must_use]
pub fn asp_hook_scenario_package() -> AspRustProjectHarnessScenarioPackage {
    asp_rust_project_harness_policy::asp_rust_project_harness_scenario_package!(
        package: ASP_HOOK_SCENARIO_PACKAGE_NAME,
        scenarios: [
            asp_rust_project_harness_policy::asp_rust_project_harness_scenario!(
                name: GENERIC_WRAPPER_TESTING_ROLE_DISPATCH_SCENARIO_ID,
                package: ASP_HOOK_SCENARIO_PACKAGE_NAME,
                description: "An arbitrary wrapper around cargo test routes through the configured asp-testing Host role dispatch.",
                fixture_root: "crates/agent-semantic-hook/tests/fixtures/scenarios/generic_wrapper_testing_role_dispatch",
                tags: ["hook", "shell-parser", "session", "role-dispatch", "host-agent", "performance"],
                commands: [
                    { label: "shell-parser-snapshot", argv: ["cargo", "test", "-p", "agent-semantic-shell-parser", "--test", "integration_test", "wrapper_match_enable_accepts_arbitrary_wrapper_names", "--", "--nocapture"] },
                    { label: "hook-session-snapshot", argv: ["cargo", "test", "-p", "agent-semantic-hook", "--test", "unit_test", "generic_wrapper_testing_role_dispatch_matches_git_snapshot", "--", "--nocapture"] },
                    { label: "wrapper-match-performance-gate", argv: ["cargo", "test", "-p", "agent-semantic-shell-parser", "--test", "integration_test", "wrapped_command_match_stays_within_git_snapshot_budget", "--", "--nocapture"] },
                ],
            ),
            asp_rust_project_harness_policy::asp_rust_project_harness_scenario!(
                name: GIT_HISTORY_TESTING_DISPATCH_SCENARIO_ID,
                package: ASP_HOOK_SCENARIO_PACKAGE_NAME,
                description: "Repository-wide Git history inspection routes to ASP Testing without claiming language-provider ownership.",
                fixture_root: "crates/agent-semantic-hook/tests/fixtures/scenarios/git_history_testing_dispatch",
                tags: ["hook", "git", "history", "role-dispatch", "host-agent"],
                commands: [
                    { label: "git-history-command-set", argv: ["cargo", "test", "-p", "agent-semantic-hook", "--test", "unit_test", "repository_git_history_command_set_routes_only_history_inspection_to_testing", "--", "--nocapture"] },
                ],
            ),
        ],
    )
}
