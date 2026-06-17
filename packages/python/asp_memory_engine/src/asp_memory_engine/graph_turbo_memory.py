"""Graph-turbo read-memory suppression helpers owned by the memory engine."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class SelectorRange:
    path: str
    start_line: int
    end_line: int


@dataclass(frozen=True)
class GraphTurboReadMemoryProjection:
    seen_selectors: tuple[str, ...]
    suppressed_selectors: tuple[str, ...]


def normalize_seen_selectors(selectors: object) -> tuple[str, ...]:
    if not isinstance(selectors, (list, tuple, set, frozenset)):
        return ()
    seen: dict[str, None] = {}
    for selector in selectors:
        if isinstance(selector, str) and selector:
            seen.setdefault(selector, None)
    return tuple(seen)


def suppressed_selectors_for_candidates(
    candidate_selectors: list[str | None],
    seen_selectors: tuple[str, ...],
    *,
    max_gap_lines: int = 8,
) -> tuple[str, ...]:
    seen = frozenset(seen_selectors)
    if not seen:
        return ()
    seen_ranges = tuple(
        parsed
        for selector in seen
        if (parsed := _graph_turbo_parse_selector(selector)) is not None
    )
    suppressed: set[str] = set()
    for selector in candidate_selectors:
        if selector is None:
            continue
        if selector in seen:
            suppressed.add(selector)
            continue
        candidate_range = _graph_turbo_parse_selector(selector)
        if candidate_range is None:
            continue
        if any(ranges_adjacent(candidate_range, seen_range, max_gap_lines=max_gap_lines) for seen_range in seen_ranges):
            suppressed.add(selector)
    return tuple(sorted(suppressed))


def read_memory_projection(
    candidate_selectors: list[str | None],
    seen_selectors: object,
    *,
    max_gap_lines: int = 8,
) -> GraphTurboReadMemoryProjection:
    normalized = normalize_seen_selectors(seen_selectors)
    return GraphTurboReadMemoryProjection(
        seen_selectors=normalized,
        suppressed_selectors=suppressed_selectors_for_candidates(
            candidate_selectors,
            normalized,
            max_gap_lines=max_gap_lines,
        ),
    )


def _graph_turbo_parse_selector(selector: str) -> SelectorRange | None:
    parsed = _parse_colon_range(selector)
    return parsed if parsed is not None else _parse_dash_range(selector)


def ranges_adjacent(
    left: SelectorRange,
    right: SelectorRange,
    *,
    max_gap_lines: int,
) -> bool:
    return (
        left.path == right.path
        and left.start_line <= right.end_line + max_gap_lines
        and right.start_line <= left.end_line + max_gap_lines
    )


def _parse_colon_range(selector: str) -> SelectorRange | None:
    path, sep, end_text = selector.rpartition(":")
    if not sep:
        return None
    path, sep, start_text = path.rpartition(":")
    if not sep:
        return None
    return _parsed_range(path, start_text, end_text)


def _parse_dash_range(selector: str) -> SelectorRange | None:
    path, sep, range_text = selector.rpartition(":")
    if not sep:
        return None
    start_text, sep, end_text = range_text.partition("-")
    if not sep:
        return None
    return _parsed_range(path, start_text, end_text)


def _parsed_range(path: str, start_text: str, end_text: str) -> SelectorRange | None:
    try:
        start = int(start_text)
        end = int(end_text)
    except ValueError:
        return None
    if not path or end < start:
        return None
    return SelectorRange(path, start, end)
