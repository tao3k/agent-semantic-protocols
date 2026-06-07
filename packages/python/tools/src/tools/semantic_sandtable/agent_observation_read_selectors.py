"""Selector parsing helpers for ASP read-loop observations."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass

from .agent_observation_asp import asp_args

_READ_LOOP_MAX_GAP_LINES = 8


@dataclass(frozen=True)
class ReadLoopCandidate:
    selector: str
    owner_path: str
    path: str | None
    start_line: int | None
    end_line: int | None
    command: str


def read_loop_candidates(commands: list[str]) -> tuple[ReadLoopCandidate, ...]:
    return tuple(
        candidate
        for command in commands
        if (candidate := _read_loop_candidate(command)) is not None
    )


def adjacent_range_window_count(candidates: tuple[ReadLoopCandidate, ...]) -> int:
    adjacent_count = 0
    for ranges in _ranges_by_path(candidates).values():
        ranges.sort()
        previous_end: int | None = None
        for start, end in ranges:
            if (
                previous_end is not None
                and start <= previous_end + _READ_LOOP_MAX_GAP_LINES
            ):
                adjacent_count += 1
            previous_end = max(previous_end or end, end)
    return adjacent_count


def adjacent_range_window_selectors(
    candidates: tuple[ReadLoopCandidate, ...],
) -> set[str]:
    adjacent: set[str] = set()
    for ranges in _candidate_ranges_by_path(candidates).values():
        ranges.sort(
            key=lambda candidate: (candidate.start_line or 0, candidate.end_line or 0)
        )
        previous: ReadLoopCandidate | None = None
        for candidate in ranges:
            if (
                previous is not None
                and candidate.start_line is not None
                and previous.end_line is not None
                and candidate.start_line <= previous.end_line + _READ_LOOP_MAX_GAP_LINES
            ):
                adjacent.add(candidate.selector)
            if previous is None or (
                candidate.end_line is not None
                and previous.end_line is not None
                and candidate.end_line > previous.end_line
            ):
                previous = candidate
    return adjacent


def group_by_selector(
    candidates: tuple[ReadLoopCandidate, ...],
) -> dict[str, list[ReadLoopCandidate]]:
    groups: dict[str, list[ReadLoopCandidate]] = {}
    for candidate in candidates:
        groups.setdefault(candidate.selector, []).append(candidate)
    return groups


def normalize_command(command: str) -> str:
    return " ".join(command.strip().split())


def command_fingerprint(command: str) -> str:
    return "sha256:" + hashlib.sha256(normalize_command(command).encode()).hexdigest()


def _read_loop_candidate(command: str) -> ReadLoopCandidate | None:
    args = asp_args(command)
    if len(args) < 2 or args[1] != "query":
        return None
    if "--code" not in args:
        return None
    selector = _option_value(args, "--selector")
    if selector is None:
        return None
    locator = _parse_selector(selector)
    path = locator[0] if locator is not None else _whole_file_selector_path(selector)
    owner_path = path or selector
    return ReadLoopCandidate(
        selector=selector,
        owner_path=owner_path,
        path=path,
        start_line=locator[1] if locator is not None else None,
        end_line=locator[2] if locator is not None else None,
        command=command,
    )


def _option_value(args: list[str], option: str) -> str | None:
    for index, arg in enumerate(args):
        if arg == option and index + 1 < len(args):
            return args[index + 1]
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


def _ranges_by_path(
    candidates: tuple[ReadLoopCandidate, ...],
) -> dict[str, list[tuple[int, int]]]:
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
    return ranges_by_path


def _candidate_ranges_by_path(
    candidates: tuple[ReadLoopCandidate, ...],
) -> dict[str, list[ReadLoopCandidate]]:
    ranges_by_path: dict[str, list[ReadLoopCandidate]] = {}
    for candidate in candidates:
        if (
            candidate.path is None
            or candidate.start_line is None
            or candidate.end_line is None
        ):
            continue
        ranges_by_path.setdefault(candidate.path, []).append(candidate)
    return ranges_by_path
