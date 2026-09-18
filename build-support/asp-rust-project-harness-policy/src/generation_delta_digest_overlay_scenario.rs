// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Generation-successor digest overlay Scenario registration.

use crate::AspRustProjectHarnessScenario;

/// One-owner successor construction reuses unchanged digest leaves and reads
/// zero unchanged source bytes.
pub const GENERATION_DELTA_DIGEST_OVERLAY_SCENARIO_ID: &str = "generation-delta-digest-overlay";

pub(super) fn generation_delta_digest_overlay_scenario() -> AspRustProjectHarnessScenario {
    crate::asp_rust_project_harness_scenario!(
        name: GENERATION_DELTA_DIGEST_OVERLAY_SCENARIO_ID,
        package: crate::search_scenarios::ASP_SEARCH_SCENARIO_PACKAGE_NAME,
        description: "A one-owner generation successor derives its canonical root from the active digest leaves and performs zero unchanged source-byte reads.",
        fixture_root: "crates/agent-semantic-client-db/tests/unit/scenarios/generation_delta_digest_overlay",
        tags: ["search", "generation", "merkle", "incremental", "performance"],
        commands: [
            {
                label: "delta-digest-overlay-gate",
                argv: [
                    "cargo",
                    "test",
                    "-p",
                    "agent-semantic-client-db",
                    "--lib",
                    "runtime_server_workspace::registry::canonical_publication::generation_delta_digest_overlay_tests::generation_delta_digest_overlay_is_scenario_measured",
                    "--",
                    "--exact",
                    "--nocapture",
                ]
            },
        ],
        benchmark: {
            harness: "libtest",
            test: "generation_delta_digest_overlay_is_scenario_measured",
            snapshot: "generation_delta_digest_overlay_v1",
            target_total: "1ms",
            max_total: "5ms",
            regression_budget: "500us",
            memory_budget_bytes: 1_048_576,
            target_rationale: "A fixed-membership one-owner mutation rehashes only the cached V1 Merkle leaf-to-root path and hashes no unchanged source bytes; the remaining O(N) immutable leaf-map copy is recorded explicitly for the next persistent-map slice.",
            warmup_iterations: 8,
            measure_iterations: 64,
            metrics: [
                { name: "workspace_owner_count", unit: "owners", kind: Exact, target: 4096 },
                { name: "changed_owner_count", unit: "owners", kind: Exact, target: 1 },
                { name: "unchanged_source_byte_read_count", unit: "reads", kind: Exact, target: 0 },
                { name: "full_merkle_rebuild_count", unit: "rebuilds", kind: Exact, target: 0 },
                { name: "digest_leaf_copy_count", unit: "leaves", kind: Exact, target: 4096 },
                { name: "successor_leaf_count", unit: "leaves", kind: Exact, target: 4096 }
            ]
        }
    )
}
