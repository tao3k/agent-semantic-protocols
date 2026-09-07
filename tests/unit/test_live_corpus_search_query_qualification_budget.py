# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Explicit cold, load, warm, release, and concurrency budgets."""

from tests.unit.live_corpus_search_query_qualification_support import PLAN_PATH, load_json


def test_every_locked_corpus_has_fixed_search_query_and_telemetry_budgets() -> None:
    plan = load_json(PLAN_PATH)
    assert plan["residentSampleCount"] >= 128
    assert plan["sequentialSampleCount"] == 10
    assert plan["concurrentSampleCount"] == 32
    assert plan["cacheStates"] == [
        {
            "state": "cold-build",
            "prepareAction": "new-isolated-workspace-generation",
            "mutationScope": "benchmark-workspace-generation",
            "sampleCount": 1,
        },
        {
            "state": "cold-load",
            "prepareAction": "evict-resident-generation-only",
            "mutationScope": "benchmark-workspace-generation",
            "sampleCount": 10,
        },
        {
            "state": "warm-read",
            "prepareAction": "reuse-exact-resident-generation",
            "mutationScope": "none",
            "sampleCount": 128,
        },
        {
            "state": "released",
            "prepareAction": "release-exact-benchmark-generation",
            "mutationScope": "benchmark-workspace-generation",
            "sampleCount": 1,
        },
    ]
    assert set(plan["failureInjections"]) == {
        "cancel-before-terminal",
        "bounded-mailbox-saturation",
        "stale-content-binding",
    }
    case_ids = [entry["caseId"] for entry in plan["cases"]]
    assert len(case_ids) == len(set(case_ids))

    for entry in plan["cases"]:
        assert entry["search"]["maximumResidentMicros"] == 1_000
        assert entry["search"]["minimumCandidates"] >= 1
        assert entry["query"]["maximumResidentMicros"] == 1_000
        assert entry["query"]["selectorStrategy"] == "first-ranked-parser-owned"
        assert entry["query"]["projectionScope"] == "live-corpus"
        assert entry["zeroMatchTerms"]
        assert set(entry["requiredTelemetryEvents"]) == {
            "runtime_resident_search_terminal",
            "runtime_exact_projection_terminal",
        }
