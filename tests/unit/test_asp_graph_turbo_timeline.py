"""Timeline evaluation tests for ASP graph turbo artifacts."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

from asp_graph_turbo.artifact_timeline import (
    TimelineParameters,
    evaluate_artifact_timeline,
)
from unit.asp_graph_turbo_timeline_support import (
    write_microburst_repeat_artifacts,
)


_REPO_ROOT = Path(__file__).resolve().parents[2]


def test_timeline_reports_microbursts_fanout_and_repeats(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)

    report = evaluate_artifact_timeline(
        tmp_path,
        parameters=TimelineParameters(
            subagent_start_gap_seconds=10,
            subagent_soft_max_seconds=30,
            subagent_hard_max_seconds=60,
            session_gap_seconds=600,
            examples=5,
        ),
    )

    _assert_timeline_counts(report)
    _assert_action_counts(report)
    _assert_promotion_action(report)
    _assert_owner_action(report)
    _assert_optimization_targets(report)
    _assert_fanout_planning(report)
    _assert_action_summary(report)
    _assert_efficiency_estimate(report)


def test_timeline_cli_is_lightweight_artifact_entrypoint(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)
    env = os.environ.copy()
    env["PYTHONPATH"] = str(
        _REPO_ROOT / "packages" / "python" / "asp_graph_turbo" / "src"
    )

    completed = subprocess.run(
        [
            sys.executable,
            "-S",
            "-m",
            "asp_graph_turbo",
            "timeline",
            str(tmp_path),
            "--recent-sessions",
            "1",
            "--examples",
            "1",
        ],
        check=True,
        text=True,
        capture_output=True,
        env=env,
    )

    assert completed.stdout.startswith("[graph-turbo-timeline] ")
    assert "[graph-turbo-next-summary]" in completed.stdout


def _assert_timeline_counts(report: dict[str, object]) -> None:
    assert report["eventCount"] == 14
    assert report["actionEventCount"] == 14
    assert report["sessionCount"] == 1
    assert report["microburstCount"] == 2
    assert report["fanoutBurstCount"] == 2
    assert report["softOverrunMicrobursts"] == 0
    assert report["inferredSubagentStarts"] == 7
    assert report["repeatSearches"] == 3
    assert report["readLoopDirectCodeReads"] == 4
    assert report["readLoopDuplicateSelectors"] == 1
    assert report["readLoopAdjacentRangeWindows"] == 1
    assert report["readLoopSameOwnerScans"] == 2


def _assert_action_counts(report: dict[str, object]) -> None:
    assert report["topMicrobursts"][0]["fanoutWidth"] == 4
    assert report["fanoutHotspots"][0]["fanoutWidth"] == 4
    assert report["actionMethodCounts"]["rust:query"] == 6
    assert report["actionMethodCounts"]["rust:search/owner"] == 4
    assert "rust:query/--from-hook" not in report["actionMethodCounts"]
    assert "rust:query/--selector" not in report["actionMethodCounts"]
    assert "rust:search/--view" not in report["actionMethodCounts"]
    assert report["repeatGroups"][0]["language"] == "python"
    assert report["repeatGroups"][0]["method"] == "search/typed-frontier"
    assert report["repeatGroups"][0]["repeatCount"] == 1


def _assert_promotion_action(report: dict[str, object]) -> None:
    action = report["typedFrontierPromotion"]["actions"][0]
    assert report["promotableTypedFrontierSearches"] == 1
    assert report["typedFrontierPromotion"]["policy"] == "repeat-search-to-typed-frontier"
    assert action["decision"] == "promote"
    assert action["replacement"] == "promote-to-owner-item-test-frontier"
    assert action["query"] == "semantic type"
    assert action["profile"] == "owner-query"
    assert action["preferredCommand"] == (
        "asp python search typed-frontier 'semantic type' owner tests --workspace . --view seeds"
    )


def _assert_owner_action(report: dict[str, object]) -> None:
    action = report["ownerCollapse"]["actions"][0]
    assert report["collapsibleOwnerSearches"] == 2
    assert report["ownerCollapse"]["policy"] == "repeat-owner-to-item-test-frontier"
    assert action["decision"] == "collapse"
    assert action["replacement"] == "promote-to-owner-query-item-test-frontier"
    assert action["owner"] == "crates/agent-semantic-protocol/src/command/provider.rs"
    assert action["profile"] == "owner-query"
    assert action["preferredCommand"] == (
        "asp rust search owner "
        "crates/agent-semantic-protocol/src/command/provider.rs "
        "items --workspace . --view seeds"
    )


def _assert_optimization_targets(report: dict[str, object]) -> None:
    categories = {target["category"] for target in report["optimizationTargets"]}
    assert "repeat-search" in categories
    assert "repeat-owner" in categories
    assert "mixed-fanout" in categories
    mixed_targets = [
        target
        for target in report["optimizationTargets"]
        if target["category"] == "mixed-fanout"
    ]
    assert mixed_targets[0]["profile"] == "owner-query"
    assert "search owner" in mixed_targets[0]["preferredCommand"]


def _assert_fanout_planning(report: dict[str, object]) -> None:
    fanout = report["fanoutPlanning"]
    action = fanout["actions"][0]
    assert report["routableFanoutBursts"] >= 1
    assert report["avoidableFanoutBranches"] >= 1
    assert fanout["policy"] == "mixed-fanout-to-single-profile-frontier"
    assert action["decision"] == "route"
    assert action["profile"] == "owner-query"
    assert action["route"] == "single-profile-frontier-before-fanout"
    assert "search owner" in action["preferredCommand"]


def _assert_action_summary(report: dict[str, object]) -> None:
    summary = report["actionSummary"]
    action = summary["actions"][0]
    assert summary["policy"] == "ranked-next-action-summary"
    assert summary["replacement"] == "run-top-preferred-command-before-widening-search"
    assert summary["actionCount"] >= 1
    assert action["impactScore"] >= 1
    assert action["preferredCommand"]
    assert action["profile"] == "owner-query"


def _assert_efficiency_estimate(report: dict[str, object]) -> None:
    estimate = report["efficiencyEstimate"]
    typed_frontier_searches = (
        report["suppressiblePrimeSearches"]
        + report["promotableTypedFrontierSearches"]
        + report["collapsibleOwnerSearches"]
    )
    avoidable_upper_bound = typed_frontier_searches + report["avoidableFanoutBranches"]
    assert estimate["policy"] == "timeline-action-reduction-estimate"
    assert estimate["basis"] == "upper-bound-repeat-searches-plus-fanout-branches"
    assert estimate["typedFrontierAvoidableSearches"] == typed_frontier_searches
    assert estimate["estimatedAvoidableActionsUpperBound"] == avoidable_upper_bound
    assert estimate["observedActions"] == report["actionEventCount"]
    assert estimate["observedRounds"] == report["roundCount"]
    assert (
        estimate["recommendedFirstCommand"]
        == (report["actionSummary"]["actions"][0]["preferredCommand"])
    )


def test_timeline_reports_read_loop_risk_from_direct_code_reads(tmp_path) -> None:
    write_microburst_repeat_artifacts(tmp_path)

    report = evaluate_artifact_timeline(
        tmp_path,
        parameters=TimelineParameters(examples=5),
    )

    risk = report["readLoopRisk"]
    assert risk["policy"] == "direct-source-read-code-loop-guard"
    assert risk["directCodeReads"] == 4
    assert risk["duplicateSelectors"] == 1
    assert risk["adjacentRangeWindows"] == 1
    assert risk["sameOwnerScans"] == 2
    assert risk["riskCount"] == 4
    assert risk["examples"][0]["selector"] == "src/lib.rs:1:10"
