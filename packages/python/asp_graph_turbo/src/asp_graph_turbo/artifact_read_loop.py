"""Read-loop risk summary for graph turbo artifact timelines."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass

from .artifact_event_model import ArtifactEvent

_MAX_GAP_LINES = 8


@dataclass(frozen=True)
class _ReadLoopCandidate:
    selector: str
    owner_path: str
    path: str | None
    start_line: int | None
    end_line: int | None
    event: ArtifactEvent


def read_loop_risk_summary(
    events: tuple[ArtifactEvent, ...], *, limit: int
) -> dict[str, object]:
    candidates = tuple(
        candidate
        for event in events
        if (candidate := _candidate_from_event(event)) is not None
    )
    selector_counts = Counter(candidate.selector for candidate in candidates)
    duplicate_selectors = sum(
        count - 1 for count in selector_counts.values() if count > 1
    )
    unique_candidates = tuple(
        {candidate.selector: candidate for candidate in candidates}.values()
    )
    owner_counts = Counter(candidate.owner_path for candidate in unique_candidates)
    same_owner_scans = sum(count - 1 for count in owner_counts.values() if count >= 3)
    adjacent_windows = _adjacent_range_window_count(unique_candidates)
    return {
        "policy": "direct-source-read-code-loop-guard",
        "directCodeReads": len(candidates),
        "duplicateSelectors": duplicate_selectors,
        "adjacentRangeWindows": adjacent_windows,
        "sameOwnerScans": same_owner_scans,
        "riskCount": duplicate_selectors + adjacent_windows + same_owner_scans,
        "examples": [
            _candidate_row(candidate) for candidate in candidates[: max(limit, 0)]
        ],
    }


def _candidate_from_event(event: ArtifactEvent) -> _ReadLoopCandidate | None:
    argv = event.argv
    if not argv or "query" not in argv:
        return None
    if "direct-source-read" not in argv or "--code" not in argv:
        return None
    selector = _option_value(argv, "--selector")
    if selector is None:
        return None
    locator = _parse_selector(selector)
    path = locator[0] if locator is not None else _whole_file_selector_path(selector)
    owner_path = path or selector
    return _ReadLoopCandidate(
        selector=selector,
        owner_path=owner_path,
        path=path,
        start_line=locator[1] if locator is not None else None,
        end_line=locator[2] if locator is not None else None,
        event=event,
    )


def _option_value(argv: tuple[str, ...], option: str) -> str | None:
    for index, arg in enumerate(argv):
        if arg == option and index + 1 < len(argv):
            return argv[index + 1]
        if arg.startswith(f"{option}="):
            return arg.split("=", 1)[1]
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


def _whole_file_selector_path(selector: str) -> str | None:
    if selector.startswith("-"):
        return None
    if "/" in selector or selector.endswith(
        (".rs", ".py", ".ts", ".jl", ".md", ".org")
    ):
        return selector
    return None


def _adjacent_range_window_count(
    candidates: tuple[_ReadLoopCandidate, ...],
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
            if previous_end is not None and start <= previous_end + _MAX_GAP_LINES:
                adjacent_count += 1
            previous_end = max(previous_end or end, end)
    return adjacent_count


def _candidate_row(candidate: _ReadLoopCandidate) -> dict[str, object]:
    row = {
        "language": candidate.event.language,
        "selector": candidate.selector,
        "ownerPath": candidate.owner_path,
        "path": candidate.event.path,
    }
    if candidate.event.project_root_arg:
        row["projectRootArg"] = candidate.event.project_root_arg
    return row
