"""ASP command flow metrics for agent observations."""

from __future__ import annotations

from collections import Counter
from typing import Any

from .agent_observation_asp import asp_args, normalize_command
from .agent_observation_commands import (
    asp_command_output_records_from_messages,
    asp_commands_from_messages,
)
from .agent_observation_frontier import frontier_context_metrics
from .agent_observation_read_loop import read_loop_memory, read_loop_stats


def pipe_flow_from_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    commands = asp_commands_from_messages(messages)
    if not commands:
        return {}
    normalized = [normalize_command(command) for command in commands]
    stats = _initial_pipe_flow_stats(commands, normalized)
    for command in commands:
        _classify_asp_command(command, stats)
    output_records = asp_command_output_records_from_messages(messages)
    _attach_read_loop_observations(stats, commands, output_records)
    _attach_output_record_observations(stats, output_records)
    _attach_frontier_context_observations(stats, commands, output_records)
    _attach_complex_pipe_stage_summary(stats)
    stats["commands"] = normalized[:12]
    return stats


def _initial_pipe_flow_stats(
    commands: list[str],
    normalized: list[str],
) -> dict[str, Any]:
    repeated = sum(count - 1 for count in Counter(normalized).values() if count > 1)
    return {
        "aspCommands": len(commands),
        "searchCommands": 0,
        "queryCommands": 0,
        "checkCommands": 0,
        "guideCommands": 0,
        "directReadCommands": 0,
        "searchPipeCommands": 0,
        "searchPrimeCommands": 0,
        "searchFzfCommands": 0,
        "searchReasoningCommands": 0,
        "searchFailureCommands": 0,
        "querySelectorCommands": 0,
        "treesitterQueryCommands": 0,
        "repeatedCommands": repeated,
    }


def _attach_read_loop_observations(
    stats: dict[str, Any],
    commands: list[str],
    output_records: list[dict[str, Any]],
) -> None:
    stats.update(read_loop_stats(commands))
    memory = read_loop_memory(commands, output_records)
    if not memory:
        return
    stats["readLoopMemory"] = memory
    stats["readLoopMemoryEntryCount"] = memory["entryCount"]
    stats["readLoopMemorySuppressibleReads"] = memory["suppressibleReadCount"]
    stats["readLoopMemoryAvoidReasons"] = memory["avoid"]


def _attach_output_record_observations(
    stats: dict[str, Any],
    output_records: list[dict[str, Any]],
) -> None:
    if output_records:
        stats["aspCommandOutputBytes"] = sum(
            record["outputBytes"] for record in output_records
        )
        stats["aspCommandOutputRecords"] = [
            _public_output_record(record) for record in output_records[:12]
        ]
    _attach_latest_precision(
        stats, output_records, "precision", "searchPipeOutputPrecision"
    )
    _attach_latest_precision(
        stats,
        output_records,
        "failurePrecision",
        "failureFrontierOutputPrecision",
    )
    failure_memory = _failure_loop_memory(output_records)
    if failure_memory:
        stats["failureLoopMemory"] = failure_memory
        stats["failureLoopMemoryEntryCount"] = failure_memory["entryCount"]


def _attach_frontier_context_observations(
    stats: dict[str, Any],
    commands: list[str],
    output_records: list[dict[str, Any]],
) -> None:
    metrics = frontier_context_metrics(commands, output_records)
    if metrics:
        stats.update(metrics)


def _public_output_record(record: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in record.items() if key != "output"}


def _attach_latest_precision(
    stats: dict[str, Any],
    records: list[dict[str, Any]],
    record_key: str,
    stats_key: str,
) -> None:
    precision = [
        record[record_key]
        for record in records
        if isinstance(record.get(record_key), dict) and record[record_key]
    ]
    if precision:
        stats[stats_key] = precision[-1]


def _attach_complex_pipe_stage_summary(stats: dict[str, Any]) -> None:
    missing = []
    if stats["searchPrimeCommands"] == 0:
        missing.append("search-prime")
    if stats["searchPipeCommands"] == 0:
        missing.append("search-pipe")
    if stats["querySelectorCommands"] == 0:
        missing.append("query-selector")
    stats["complexPipeFlow"] = not missing
    stats["missingComplexPipeStages"] = missing


def _classify_asp_command(command: str, stats: dict[str, Any]) -> None:
    args = asp_args(command)
    if len(args) < 2:
        return
    surface = args[1]
    if surface == "search":
        stats["searchCommands"] += 1
        profile = args[2] if len(args) > 2 else ""
        if profile == "pipe":
            stats["searchPipeCommands"] += 1
        elif profile == "prime":
            stats["searchPrimeCommands"] += 1
        elif profile == "fzf":
            stats["searchFzfCommands"] += 1
        elif profile == "reasoning":
            stats["searchReasoningCommands"] += 1
        elif profile == "failure":
            stats["searchFailureCommands"] += 1
    elif surface == "query":
        stats["queryCommands"] += 1
        if "--selector" in args:
            stats["querySelectorCommands"] += 1
        if "--treesitter-query" in args:
            stats["treesitterQueryCommands"] += 1
        if "direct-source-read" in args:
            stats["directReadCommands"] += 1
    elif surface == "check":
        stats["checkCommands"] += 1
    elif surface == "guide":
        stats["guideCommands"] += 1


def _failure_loop_memory(records: list[dict[str, Any]]) -> dict[str, Any]:
    entries = []
    for record in records:
        memory = record.get("failureMemory")
        if not isinstance(memory, dict):
            continue
        for entry in memory.get("entries", []):
            if isinstance(entry, dict):
                entries.append(entry)
    if not entries:
        return {}
    return {
        "schemaId": "agent.semantic-protocols.failure-loop-memory",
        "schemaVersion": "1",
        "policy": "failure-frontier-action-memory",
        "entryCount": len(entries),
        "entries": entries[:8],
    }
