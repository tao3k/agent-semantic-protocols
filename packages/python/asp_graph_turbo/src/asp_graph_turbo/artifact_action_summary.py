"""Top action recommendations for graph turbo artifact timelines."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any


def action_summary(report_actions: Mapping[str, object], *, limit: int) -> dict[str, object]:
    actions = [
        *_fanout_actions(report_actions.get("fanoutPlanning")),
        *_owner_actions(report_actions.get("ownerCollapse")),
        *_fzf_actions(report_actions.get("fzfPromotion")),
        *_prime_actions(report_actions.get("primeSuppression")),
    ]
    actions.sort(
        key=lambda action: (
            -int(action["impactScore"]),
            str(action["source"]),
            str(action["preferredCommand"]),
        )
    )
    return {
        "policy": "ranked-next-action-summary",
        "replacement": "run-top-preferred-command-before-widening-search",
        "actionCount": len(actions),
        "actions": actions[:limit],
    }


def _fanout_actions(value: object) -> list[dict[str, object]]:
    if not isinstance(value, Mapping):
        return []
    return [
        _summary_row(
            action,
            source="fanoutPlanning",
            category="mixed-fanout",
            impact_score=(int(action["fanoutWidth"]) * 10) + int(action["events"]),
        )
        for action in value.get("actions", [])
        if isinstance(action, Mapping)
    ]


def _owner_actions(value: object) -> list[dict[str, object]]:
    if not isinstance(value, Mapping):
        return []
    return [
        _summary_row(
            action,
            source="ownerCollapse",
            category="repeat-owner",
            impact_score=(int(action["repeatCount"]) * 10) + int(action["count"]),
        )
        for action in value.get("actions", [])
        if isinstance(action, Mapping)
    ]


def _fzf_actions(value: object) -> list[dict[str, object]]:
    if not isinstance(value, Mapping):
        return []
    return [
        _summary_row(
            action,
            source="fzfPromotion",
            category="repeat-fzf",
            impact_score=(int(action["repeatCount"]) * 8) + int(action["count"]),
        )
        for action in value.get("actions", [])
        if isinstance(action, Mapping)
    ]


def _prime_actions(value: object) -> list[dict[str, object]]:
    if not isinstance(value, Mapping):
        return []
    return [
        _summary_row(
            action,
            source="primeSuppression",
            category="repeat-prime",
            impact_score=max(1, int(float(action["ageSeconds"]) // 60)),
        )
        for action in value.get("actions", [])
        if isinstance(action, Mapping)
    ]


def _summary_row(
    action: Mapping[str, Any],
    *,
    source: str,
    category: str,
    impact_score: int,
) -> dict[str, object]:
    row = {
        "source": source,
        "category": category,
        "decision": action["decision"],
        "replacement": action["replacement"],
        "impactScore": impact_score,
        "preferredCommand": str(action.get("preferredCommand") or ""),
        "profile": str(action.get("profile") or ""),
        "route": str(action.get("route") or ""),
    }
    for key in ("language", "method", "subject", "owner", "query", "projectRootArg"):
        if action.get(key):
            row[key] = action[key]
    return row
