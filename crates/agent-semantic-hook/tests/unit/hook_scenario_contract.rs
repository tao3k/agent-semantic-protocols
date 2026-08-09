use std::collections::BTreeSet;

#[test]
fn hook_scenarios_dispatch_only_function_scoped_cargo_test_atoms() {
    let package = asp_rust_project_harness_policy::hook_scenarios::asp_hook_scenario_package();
    assert!(
        !package.scenarios.is_empty(),
        "Hook scenario package is empty"
    );

    let mut scenario_names = BTreeSet::new();
    for scenario in package.scenarios {
        assert!(
            scenario_names.insert(scenario.name),
            "duplicate scenario atom"
        );
        assert!(
            !scenario.commands.is_empty(),
            "scenario has no executable atoms"
        );

        let mut command_labels = BTreeSet::new();
        for command in scenario.commands {
            assert!(
                command_labels.insert(command.label),
                "duplicate command atom"
            );
            let argv = command.argv;
            assert_eq!(
                argv.first(),
                Some(&"cargo"),
                "scenario must use Cargo authority"
            );
            assert_eq!(
                argv.get(1),
                Some(&"test"),
                "scenario must be a Cargo test atom"
            );
            assert!(
                argv.windows(2).any(|pair| pair[0] == "-p"),
                "missing package owner"
            );
            assert!(
                argv.windows(2).any(|pair| pair[0] == "--test"),
                "missing integration-test owner"
            );
            let separator = argv
                .iter()
                .position(|value| *value == "--")
                .expect("scenario atom must delimit harness arguments");
            assert!(
                separator >= 2 && !argv[separator - 1].starts_with('-'),
                "scenario atom must name one focused test before `--`"
            );
        }
    }
}
