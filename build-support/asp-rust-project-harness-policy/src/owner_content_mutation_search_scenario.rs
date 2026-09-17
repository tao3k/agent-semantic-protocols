// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Owner-local PostToolUse content transaction Scenario registration.

use crate::AspRustProjectHarnessScenario;

pub(super) fn owner_content_mutation_scenario() -> AspRustProjectHarnessScenario {
    crate::asp_rust_project_harness_scenario!(
        name: "owner-content-mutation-v1",
        package: crate::search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME,
        description: "One edited owner atomically advances content identity and GREP/path coverage, tombstones stale parser symbols, then one V1 parser rebind publishes exact-selector and Tantivy symbol views together without rebuilding the workspace generation.",
        fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/owner_content_mutation_v1",
        tags: ["search", "runtime", "post-tool", "owner-local", "tantivy", "performance"],
        commands: [
            {
                label: "owner-content-transaction",
                argv: [
                    "cargo",
                    "test",
                    "-p",
                    "agent-semantic-client-db",
                    "--test",
                    "runtime_server_owner_content_mutation",
                ]
            },
        ],
    )
}
