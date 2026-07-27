use asp_rust_project_harness_policy::hook_scenarios::{
    ASP_HOOK_SCENARIO_PACKAGE_NAME, GENERIC_WRAPPER_TESTING_RESIDENT_DISPATCH_SCENARIO_ID,
    asp_hook_scenario_package,
};

#[test]
fn hook_scenario_package_tracks_wrapper_match_and_session_snapshot() {
    let package = asp_hook_scenario_package();
    assert_eq!(package.package_name, ASP_HOOK_SCENARIO_PACKAGE_NAME);
    let scenario = package
        .scenarios
        .iter()
        .find(|scenario| scenario.name == GENERIC_WRAPPER_TESTING_RESIDENT_DISPATCH_SCENARIO_ID)
        .expect("generic wrapper testing resident dispatch scenario");
    assert_eq!(
        scenario.fixture_root,
        "crates/agent-semantic-hook/tests/fixtures/scenarios/generic_wrapper_testing_resident_dispatch"
    );
    assert_eq!(scenario.commands.len(), 3);
    assert!(
        scenario
            .commands
            .iter()
            .any(|command| command.label == "command-match-snapshot")
    );
    assert!(
        scenario
            .commands
            .iter()
            .any(|command| command.label == "hook-session-snapshot")
    );
    assert!(
        scenario
            .commands
            .iter()
            .any(|command| command.label == "wrapper-match-performance-gate")
    );
}
