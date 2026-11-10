// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process-cold mapped Topology owner-membership Scenario registration.

use crate::AspRustProjectHarnessScenario;

/// Process-cold mapped owner membership stays on the locator plane and does
/// not hydrate detailed selector or parser-anchor records.
pub const MAPPED_TOPOLOGY_OWNER_MEMBERSHIP_SCENARIO_ID: &str = "mapped-topology-owner-membership";

pub(super) fn mapped_topology_owner_membership_scenario() -> AspRustProjectHarnessScenario {
    crate::asp_rust_project_harness_scenario!(
        name: MAPPED_TOPOLOGY_OWNER_MEMBERSHIP_SCENARIO_ID,
        package: crate::search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME,
        description: "Mapped exact owner membership reads only the durable locator plane and leaves detailed Topology projection unmaterialized.",
        fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/mapped_topology_owner_membership",
        tags: ["search", "topology", "owner-membership", "mmap", "performance"],
        commands: [
            {
                label: "mapped-locator-plane-gate",
                argv: [
                    "cargo",
                    "test",
                    "-p",
                    "agent-semantic-client-db",
                    "--lib",
                    "runtime_server_workspace::registry::canonical_publication::tests::mapped_topology_owner_membership_is_scenario_measured",
                    "--",
                    "--exact",
                    "--nocapture",
                ]
            },
        ],
        benchmark: {
            harness: "libtest",
            test: "mapped_topology_owner_membership_is_scenario_measured",
            snapshot: "mapped_topology_owner_membership_v1",
            target_total: "50us",
            max_total: "2ms",
            regression_budget: "50us",
            memory_budget_bytes: 65_536,
            target_rationale: "Exact owner membership performs one BTree locator lookup over 4096 mapped owners without decoding selector, anchor, relation, or source payloads.",
            warmup_iterations: 16,
            measure_iterations: 128,
            metrics: [
                { name: "workspace_owner_count", unit: "owners", kind: Exact, target: 4096 },
                { name: "returned_owner_count", unit: "owners", kind: Exact, target: 1 },
                { name: "selector_hydration_count", unit: "records", kind: Exact, target: 0 },
                { name: "source_byte_read_count", unit: "reads", kind: Exact, target: 0 },
                { name: "detailed_topology_materialization_count", unit: "materializations", kind: Exact, target: 0 }
            ]
        }
    )
}
