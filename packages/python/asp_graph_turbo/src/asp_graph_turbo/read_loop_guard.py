"""Read-loop guard projection for graph turbo frontier results."""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping
from dataclasses import dataclass

from .model import FrontierEntry, Node, ReadLoopGuard


@dataclass(frozen=True)
class _ReadCandidate:
    selector: str
    owner_path: str
    path: str | None
    start_line: int | None
    end_line: int | None


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
    selector = _selector_for_node(node)
    if selector is None:
        return None
    locator = _parse_selector(selector)
    path = locator[0] if locator is not None else None
    owner_path = str(
        node.fields.get("ownerPath") or node.fields.get("path") or path or ""
    )
    if not owner_path:
        owner_path = selector
    return _ReadCandidate(
        selector=selector,
        owner_path=owner_path,
        path=path,
        start_line=locator[1] if locator is not None else None,
        end_line=locator[2] if locator is not None else None,
    )


def _selector_for_node(node: Node) -> str | None:
    if node.kind == "field":
        fields = node.fields.get("fields")
        if isinstance(fields, Mapping):
            context_locator = fields.get("contextLocator")
            if context_locator is not None:
                return str(context_locator)
    locator = node.fields.get("locator") or node.fields.get("location")
    if locator is not None:
        return str(locator)
    path = node.fields.get("path")
    start = node.fields.get("startLine") or node.fields.get("start")
    end = node.fields.get("endLine") or node.fields.get("end")
    if path is not None and start is not None and end is not None:
        return f"{path}:{start}:{end}"
    return None


def _parse_selector(selector: str) -> tuple[str, int, int] | None:
    path, start, end = _parse_colon_range(selector)
    if path is not None:
        return path, start, end
    path, start, end = _parse_dash_range(selector)
    if path is not None:
        return path, start, end
    return None


def _parse_colon_range(selector: str) -> tuple[str | None, int, int]:
    path, sep, end_text = selector.rpartition(":")
    if not sep:
        return None, 0, 0
    path, sep, start_text = path.rpartition(":")
    if not sep:
        return None, 0, 0
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError:
        return None, 0, 0
    if not path or end < start:
        return None, 0, 0
    return path, start, end


def _parse_dash_range(selector: str) -> tuple[str | None, int, int]:
    path, sep, range_text = selector.rpartition(":")
    if not sep:
        return None, 0, 0
    start_text, sep, end_text = range_text.partition("-")
    if not sep:
        return None, 0, 0
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError:
        return None, 0, 0
    if not path or end < start:
        return None, 0, 0
    return path, start, end


def _adjacent_range_window_count(
    candidates: tuple[_ReadCandidate, ...], *, max_gap_lines: int
) -> int:
    ranges_by_path: dict[str, list[tuple[int, int]]] = {}
    for candidate in candidates:
        if (
            candidate.path is None
            or candidate.start_line is None
            or candidate.end_line is None
        ):
            continue
        ranges_by_path.setdefault(candidate.path, []).append(
            (candidate.start_line, candidate.end_line)
        )
    adjacent_count = 0
    for ranges in ranges_by_path.values():
        ranges.sort()
        previous_end: int | None = None
        for start, end in ranges:
            if previous_end is not None and start <= previous_end + max_gap_lines:
                adjacent_count += 1
            previous_end = max(previous_end or end, end)
    return adjacent_count
