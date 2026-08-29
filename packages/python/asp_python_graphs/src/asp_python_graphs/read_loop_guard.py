"""Detect repeated direct-code frontier shapes before rendering compact actions."""

from __future__ import annotations

from collections import Counter

from dataclasses import dataclass

from .model import FrontierEntry, Node, ReadLoopGuard

from .selector import (
    GraphTurboSelectorRange,
    graph_turbo_node_range,
    graph_turbo_owner_path_for_node,
    graph_turbo_parse_selector,
    graph_turbo_selector_for_node,
)


@dataclass(frozen=True)
class _ReadCandidate:
    selector: str
    owner_path: str
    node_range: GraphTurboSelectorRange | None


def evaluate_read_loop_guard(
    frontier: tuple[FrontierEntry, ...], *, max_gap_lines: int
) -> ReadLoopGuard:
    candidates = tuple(
        candidate
        for entry in frontier
        if entry.action == "code"
        if (candidate := _candidate_from_node(entry.node)) is not None
    )
    selector_counts = Counter(candidate.selector for candidate in candidates)
    duplicate_selector_count = sum(
        count - 1 for count in selector_counts.values() if count > 1
    )
    unique_candidates = tuple(
        {candidate.selector: candidate for candidate in candidates}.values()
    )
    adjacent_range_window_count = _adjacent_range_window_count(
        unique_candidates, max_gap_lines=max_gap_lines
    )
    owner_counts = Counter(candidate.owner_path for candidate in unique_candidates)
    same_owner_scan_count = sum(
        count - 1 for count in owner_counts.values() if count >= 3
    )
    avoid: list[str] = []
    if duplicate_selector_count:
        avoid.append("duplicate-read")
    if adjacent_range_window_count:
        avoid.append("manual-window-scan")
    if same_owner_scan_count:
        avoid.append("repeat-owner")
    return ReadLoopGuard(
        direct_code_action_count=len(candidates),
        duplicate_selector_count=duplicate_selector_count,
        adjacent_range_window_count=adjacent_range_window_count,
        same_owner_scan_count=same_owner_scan_count,
        avoid=tuple(avoid),
    )


def _candidate_from_node(node: Node) -> _ReadCandidate | None:
    selector = graph_turbo_selector_for_node(node)
    if selector is None:
        return None
    locator = graph_turbo_parse_selector(selector) or graph_turbo_node_range(node)
    owner_path = graph_turbo_owner_path_for_node(node) or (
        locator.path if locator is not None else ""
    )
    if not owner_path:
        owner_path = selector
    return _ReadCandidate(
        selector=selector,
        owner_path=owner_path,
        node_range=locator,
    )


def _adjacent_range_window_count(
    candidates: tuple[_ReadCandidate, ...], *, max_gap_lines: int
) -> int:
    ranges_by_path: dict[str, list[GraphTurboSelectorRange]] = {}
    for candidate in candidates:
        if candidate.node_range is None:
            continue
        ranges_by_path.setdefault(candidate.node_range.path, []).append(
            candidate.node_range
        )
    adjacent_count = 0
    for ranges in ranges_by_path.values():
        ranges.sort(key=lambda item: (item.start_line, item.end_line))
        previous_end: int | None = None
        for node_range in ranges:
            if (
                previous_end is not None
                and node_range.start_line <= previous_end + max_gap_lines
            ):
                adjacent_count += 1
            previous_end = max(previous_end or node_range.end_line, node_range.end_line)
    return adjacent_count
