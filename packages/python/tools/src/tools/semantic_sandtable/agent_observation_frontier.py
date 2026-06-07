"""Frontier follow and context-use metrics for ASP agent command flows."""

from __future__ import annotations

import re
from collections.abc import Iterable
from typing import Any

from .agent_observation_asp import asp_args

_SELECTOR_IN_OUTPUT = re.compile(
    r"(?:selector=|--selector(?:=|\s+))(?P<selector>[^\s,\)]+)"
)


def frontier_context_metrics(
    commands: list[str], output_records: list[dict[str, Any]]
) -> dict[str, Any]:
    projected_selectors = _unique(
        selector
        for record in output_records
        for selector in _projected_selectors(record)
    )
    if not projected_selectors:
        return {}

    query_selectors = _unique(
        selector for command in commands if (selector := _query_selector(command))
    )
    projected_set = set(projected_selectors)
    followed_selectors = tuple(
        selector for selector in query_selectors if selector in projected_set
    )
    off_frontier_selectors = tuple(
        selector for selector in query_selectors if selector not in projected_set
    )

    followed_count = len(followed_selectors)
    projected_count = len(projected_selectors)
    query_count = len(query_selectors)
    utilization_denominator = max(projected_count, query_count)
    return {
        "frontierProjectedSelectors": projected_count,
        "frontierFollowedSelectors": followed_count,
        "frontierUnfollowedSelectors": max(projected_count - followed_count, 0),
        "frontierOffPathSelectors": len(off_frontier_selectors),
        "frontierFollowRate": _ratio(followed_count, projected_count),
        "contextPrecision": _ratio(followed_count, query_count),
        "contextUtilization": _ratio(followed_count, utilization_denominator),
        "frontierFollow": {
            "projectedSelectors": list(projected_selectors[:12]),
            "followedSelectors": list(followed_selectors[:12]),
            "offFrontierSelectors": list(off_frontier_selectors[:12]),
        },
    }


def _projected_selectors(record: dict[str, Any]) -> tuple[str, ...]:
    command = str(record.get("command", ""))
    args = asp_args(command)
    if len(args) < 3 or args[1] != "search" or args[2] not in {"pipe", "failure"}:
        return ()
    output = str(record.get("output", ""))
    return tuple(match.group("selector") for match in _SELECTOR_IN_OUTPUT.finditer(output))


def _query_selector(command: str) -> str | None:
    args = asp_args(command)
    if len(args) < 2 or args[1] != "query":
        return None
    for index, arg in enumerate(args):
        if arg == "--selector" and index + 1 < len(args):
            return args[index + 1]
        if arg.startswith("--selector="):
            return arg.split("=", 1)[1]
    return None


def _unique(values: Iterable[str]) -> tuple[str, ...]:
    seen: set[str] = set()
    unique_values: list[str] = []
    for value in values:
        if not value or value in seen:
            continue
        seen.add(value)
        unique_values.append(value)
    return tuple(unique_values)


def _ratio(numerator: int, denominator: int) -> float:
    if denominator <= 0:
        return 0.0
    return round(numerator / denominator, 4)
