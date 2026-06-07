"""Read-loop metrics for ASP direct-code command sequences."""

from __future__ import annotations

from collections import Counter
from typing import Any

from .agent_observation_read_selectors import (
    adjacent_range_window_count,
    adjacent_range_window_selectors,
    command_fingerprint,
    group_by_selector,
    normalize_command,
    read_loop_candidates,
)


def read_loop_stats(commands: list[str]) -> dict[str, int]:
    candidates = read_loop_candidates(commands)
    selector_counts = Counter(candidate.selector for candidate in candidates)
    duplicate_selectors = sum(
        count - 1 for count in selector_counts.values() if count > 1
    )
    unique_candidates = tuple(
        {candidate.selector: candidate for candidate in candidates}.values()
    )
    owner_counts = Counter(candidate.owner_path for candidate in unique_candidates)
    same_owner_scans = sum(count - 1 for count in owner_counts.values() if count >= 3)
    return {
        "readLoopDirectCodeCommands": len(candidates),
        "readLoopDuplicateSelectors": duplicate_selectors,
        "readLoopAdjacentRangeWindows": adjacent_range_window_count(unique_candidates),
        "readLoopSameOwnerScans": same_owner_scans,
    }


def read_loop_memory(
    commands: list[str], output_records: list[dict[str, Any]] | None = None
) -> dict[str, Any]:
    candidates = read_loop_candidates(commands)
    if not candidates:
        return {}
    output_by_command = _output_records_by_command(output_records)
    adjacent_selectors = adjacent_range_window_selectors(candidates)
    repeated_owner_paths = _repeated_owner_paths(candidates)
    entries, suppressible_reads = _read_loop_memory_entries(
        candidates,
        output_by_command,
        adjacent_selectors,
        repeated_owner_paths,
    )
    return _read_loop_memory_packet(entries, suppressible_reads)


def _output_records_by_command(
    output_records: list[dict[str, Any]] | None,
) -> dict[str, dict[str, Any]]:
    return {
        str(record.get("command", "")): record
        for record in output_records or []
        if isinstance(record, dict)
    }


def _repeated_owner_paths(candidates: tuple[Any, ...]) -> set[str]:
    unique_candidates = {
        candidate.selector: candidate for candidate in candidates
    }.values()
    return {
        owner_path
        for owner_path, count in Counter(
            candidate.owner_path for candidate in unique_candidates
        ).items()
        if count >= 3
    }


def _read_loop_memory_entries(
    candidates: tuple[Any, ...],
    output_by_command: dict[str, dict[str, Any]],
    adjacent_selectors: set[str],
    repeated_owner_paths: set[str],
) -> tuple[list[dict[str, Any]], int]:
    entries = []
    suppressible_reads = 0
    for selector, group in group_by_selector(candidates).items():
        entry, suppressible_count = _read_loop_memory_entry(
            selector,
            group,
            output_by_command,
            adjacent_selectors,
            repeated_owner_paths,
        )
        entries.append(entry)
        suppressible_reads += suppressible_count
    return entries, suppressible_reads


def _read_loop_memory_entry(
    selector: str,
    group: list[Any],
    output_by_command: dict[str, dict[str, Any]],
    adjacent_selectors: set[str],
    repeated_owner_paths: set[str],
) -> tuple[dict[str, Any], int]:
    first = group[0]
    output_records_for_selector = _selector_output_records(group, output_by_command)
    avoid_reasons, suppressible_count = _avoid_reasons(
        selector,
        group,
        first.owner_path,
        adjacent_selectors,
        repeated_owner_paths,
    )
    entry: dict[str, Any] = {
        "selector": selector,
        "ownerPath": first.owner_path,
        "readCount": len(group),
        "repeatCount": max(len(group) - 1, 0),
        "suppressible": bool(avoid_reasons),
        "avoidReasons": avoid_reasons,
        "commandFingerprint": command_fingerprint(first.command),
    }
    _attach_locator_fields(entry, first)
    _attach_output_fields(entry, output_records_for_selector)
    return entry, suppressible_count


def _selector_output_records(
    group: list[Any],
    output_by_command: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    return [
        output_by_command[normalized]
        for candidate in group
        if (normalized := normalize_command(candidate.command)) in output_by_command
    ]


def _avoid_reasons(
    selector: str,
    group: list[Any],
    owner_path: str,
    adjacent_selectors: set[str],
    repeated_owner_paths: set[str],
) -> tuple[list[str], int]:
    reasons = []
    suppressible_count = 0
    repeat_count = max(len(group) - 1, 0)
    if repeat_count:
        reasons.append("duplicate-read")
        suppressible_count += repeat_count
    if selector in adjacent_selectors:
        reasons.append("manual-window-scan")
        suppressible_count += 1
    if owner_path in repeated_owner_paths:
        reasons.append("repeat-owner")
        suppressible_count += 1
    return reasons, suppressible_count


def _attach_locator_fields(entry: dict[str, Any], candidate: Any) -> None:
    if candidate.path is not None:
        entry["path"] = candidate.path
    if candidate.start_line is not None and candidate.end_line is not None:
        entry["startLine"] = candidate.start_line
        entry["endLine"] = candidate.end_line


def _attach_output_fields(
    entry: dict[str, Any],
    output_records_for_selector: list[dict[str, Any]],
) -> None:
    if not output_records_for_selector:
        return
    entry["outputBytes"] = sum(
        int(record.get("outputBytes", 0)) for record in output_records_for_selector
    )
    entry["resultFingerprints"] = [
        str(record["outputFingerprint"])
        for record in output_records_for_selector
        if isinstance(record.get("outputFingerprint"), str)
    ][:4]


def _read_loop_memory_packet(
    entries: list[dict[str, Any]],
    suppressible_reads: int,
) -> dict[str, Any]:
    avoid = sorted(
        {
            reason
            for entry in entries
            for reason in entry["avoidReasons"]
            if isinstance(reason, str)
        }
    )
    return {
        "schemaId": "agent.semantic-protocols.read-loop-memory",
        "schemaVersion": "1",
        "policy": "selector-read-memory",
        "entryCount": len(entries),
        "suppressibleReadCount": suppressible_reads,
        "avoid": avoid,
        "entries": entries[:12],
    }
