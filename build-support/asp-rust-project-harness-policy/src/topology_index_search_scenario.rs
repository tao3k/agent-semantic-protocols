// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Repository Topology Index Scenario registration.

use crate::AspRustProjectHarnessScenario;

pub(super) fn topology_index_scenario() -> AspRustProjectHarnessScenario {
    crate::asp_rust_project_harness_scenario!(
        name: "topology-index-v1",
        package: crate::search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME,
        description: "All language and document providers contribute Merkle-bound directories, owners, and parser-native nodes to one body-free Topology Index.",
        fixture_root: "crates/agent-semantic-topology/tests/scenarios/topology_index_v1",
        tags: ["search", "topology", "heading", "symbol", "merkle", "cross-language", "performance"],
        commands: [
            {
                label: "topology-index",
                argv: ["cargo", "test", "-p", "agent-semantic-topology"]
            },
        ],
    )
}
