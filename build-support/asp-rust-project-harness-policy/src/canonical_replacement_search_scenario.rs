// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Resident-first canonical replacement Scenario registration.

use crate::AspRustProjectHarnessScenario;

pub(super) fn canonical_replacement_resident_first_scenario() -> AspRustProjectHarnessScenario {
    crate::asp_rust_project_harness_scenario!(
        name: "canonical-replacement-resident-first",
        package: crate::search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME,
        description: "Validated canonical replacement inherits pointer epoch metadata and publishes resident query authority without restoring the superseded mmap payload.",
        fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/canonical_replacement_resident_first",
        tags: ["search", "runtime", "generation", "resident-first", "performance"],
        commands: [
            {
                label: "corrupt-old-mmap-replacement",
                argv: [
                    "cargo",
                    "test",
                    "-p",
                    "agent-semantic-client-db",
                    "--test",
                    "unit_test",
                    "runtime_server_workspace_recovery::validated_canonical_replacement_does_not_restore_superseded_mmap",
                    "--",
                    "--exact",
                    "--nocapture",
                ]
            },
        ],
    )
}
