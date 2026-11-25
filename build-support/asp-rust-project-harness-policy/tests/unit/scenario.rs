// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use asp_rust_project_harness_policy::asp_rust_project_harness_scenario;
use asp_rust_project_harness_policy::asp_rust_project_harness_scenario_package;

#[test]
fn scenario_macro_packages_custom_scenario_data() {
    let scenario = asp_rust_project_harness_scenario! {
        name: "source-index-warm-path",
        package: "agent-semantic-search",
        description: "warm source-index search stays inside the ASP performance gate",
        fixture_root: "tests/unit/scenarios/source_index_warm_path",
        tags: ["performance", "source-index"],
        commands: [
            {
                label: "warm-search",
                argv: ["cargo", "test", "-p", "agent-semantic-search"]
            }
        ],
    };

    assert_eq!(scenario.name, "source-index-warm-path");
    assert_eq!(scenario.package_name, "agent-semantic-search");
    assert_eq!(scenario.tags, &["performance", "source-index"]);
    assert_eq!(
        scenario.commands[0].argv,
        &["cargo", "test", "-p", "agent-semantic-search"]
    );
}

#[test]
fn scenario_package_macro_groups_custom_scenarios() {
    let scenario = asp_rust_project_harness_scenario! {
        name: "owner-items-frontier",
        package: "agent-semantic-topology",
        description: "owner-items frontier is available before direct source reads",
        fixture_root: "tests/unit/scenarios/owner_items_frontier",
        tags: ["owner-items"],
        commands: [
            {
                label: "owner-items",
                argv: ["cargo", "test", "-p", "agent-semantic-topology"]
            }
        ],
    };
    let package = asp_rust_project_harness_scenario_package! {
        package: "agent-semantic-topology",
        scenarios: [scenario],
    };

    assert_eq!(package.package_name, "agent-semantic-topology");
    assert_eq!(package.scenarios.len(), 1);
    assert_eq!(package.scenarios[0].name, "owner-items-frontier");
}
