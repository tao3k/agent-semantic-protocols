# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Explicit cold, load, warm, release, and concurrency budgets."""

from tests.unit.live_corpus_search_query_qualification_support import load_plan


def test_every_locked_corpus_has_fixed_search_query_and_telemetry_budgets() -> None:
    plan = load_plan()
    assert plan["resident_sample_count"] >= 128
    assert plan["sequential_sample_count"] == 10
    assert plan["concurrent_sample_count"] == 32
    assert plan["cache_states"] == [
        {
            "state": "cold-build",
            "prepare_action": "new-isolated-workspace-generation",
            "mutation_scope": "benchmark-workspace-generation",
            "sample_count": 1,
        },
        {
            "state": "cold-load",
            "prepare_action": "evict-resident-generation-only",
            "mutation_scope": "benchmark-workspace-generation",
            "sample_count": 10,
        },
        {
            "state": "warm-read",
            "prepare_action": "reuse-exact-resident-generation",
            "mutation_scope": "none",
            "sample_count": 128,
        },
        {
            "state": "released",
            "prepare_action": "release-exact-benchmark-generation",
            "mutation_scope": "benchmark-workspace-generation",
            "sample_count": 1,
        },
    ]
    assert set(plan["failure_injections"]) == {
        "cancel-before-terminal",
        "bounded-mailbox-saturation",
        "stale-content-binding",
    }
    case_ids = [entry["case_id"] for entry in plan["cases"]]
    assert len(case_ids) == len(set(case_ids))

    for entry in plan["cases"]:
        assert entry["maximum_search_micros"] == 500_000
        assert entry["minimum_candidates"] >= 1
        assert entry["maximum_resident_query_micros"] == 1_000
        assert entry["search"].startswith("(search ")
        assert entry["zero_match_search"].startswith("(search ")
        assert entry["source_query"].count("{{selector}}") == 1
        assert entry["callable_skeleton_query"].count("{{selector}}") == 1
        assert set(entry["scenario_classes"]) - {
            "exact-parser-owner", "zero-match"
        } in [
            {"regex-truth"},
            {"ranked-text"},
            {"structural-syntax"},
            {"topology-membership"},
            {"explicit-conjunction"},
        ]
        assert set(entry["required_telemetry_events"]) == {
            "runtime_resident_search_terminal",
            "runtime_query_playbook_terminal",
        }

    assert {
        next(
            value
            for value in entry["scenario_classes"]
            if value in {
                "regex-truth",
                "ranked-text",
                "structural-syntax",
                "topology-membership",
                "explicit-conjunction",
            }
        )
        for entry in plan["cases"]
    } == {
        "regex-truth",
        "ranked-text",
        "structural-syntax",
        "topology-membership",
        "explicit-conjunction",
    }
