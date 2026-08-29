"""Efficiency estimates for graph turbo artifact timeline reports."""

from __future__ import annotations

from collections.abc import Mapping


def efficiency_estimate(report: Mapping[str, object]) -> dict[str, object]:
    typed_frontier_searches = (
        _int_value(report, "suppressiblePrimeSearches")
        + _int_value(report, "promotableTypedFrontierSearches")
        + _int_value(report, "collapsibleOwnerSearches")
    )
    avoidable_fanout_branches = _int_value(report, "avoidableFanoutBranches")
    avoidable_upper_bound = typed_frontier_searches + avoidable_fanout_branches
    observed_actions = _int_value(report, "actionEventCount")
    return {
        "policy": "timeline-action-reduction-estimate",
        "basis": "upper-bound-repeat-searches-plus-fanout-branches",
        "observedActions": observed_actions,
        "observedRounds": _int_value(report, "roundCount"),
        "repeatSearches": _int_value(report, "repeatSearches"),
        "routableFanoutBursts": _int_value(report, "routableFanoutBursts"),
        "typedFrontierAvoidableSearches": typed_frontier_searches,
        "avoidableFanoutBranches": avoidable_fanout_branches,
        "estimatedAvoidableActionsUpperBound": avoidable_upper_bound,
        "estimatedActionReductionRatioUpperBound": _ratio(
            avoidable_upper_bound,
            observed_actions,
        ),
        "recommendedFirstCommand": _recommended_first_command(report),
        "acceptance": "compare-recent-session-after-running-top-preferred-command",
    }


def _recommended_first_command(report: Mapping[str, object]) -> str:
    action_summary = report.get("actionSummary")
    if not isinstance(action_summary, Mapping):
        return ""
    actions = action_summary.get("actions")
    if not isinstance(actions, list) or not actions:
        return ""
    first = actions[0]
    if not isinstance(first, Mapping):
        return ""
    return str(first.get("preferredCommand") or "")


def _int_value(report: Mapping[str, object], key: str) -> int:
    value = report.get(key)
    return int(value) if isinstance(value, int) else 0


def _ratio(numerator: int, denominator: int) -> float:
    if denominator <= 0:
        return 0.0
    return round(numerator / denominator, 4)
