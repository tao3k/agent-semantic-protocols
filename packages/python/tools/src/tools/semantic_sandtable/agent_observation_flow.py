# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
from .direct_read_shape import direct_source_read_shape


def command_flow_from_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    output_records = asp_command_output_records_from_messages(messages)
    commands = _flow_commands(output_records, asp_commands_from_messages(messages))
    if not commands:
        if output_records:
            stats = _initial_command_flow_stats([], [])
            _attach_denied_command_observations(stats, output_records)
            return stats if stats.get("deniedAspCommands") else {}
        return {}
    normalized = [normalize_command(command) for command in commands]
    stats = _initial_command_flow_stats(commands, normalized)
    for command in commands:
        _classify_asp_command(command, stats)
    _attach_denied_command_observations(stats, output_records)
    _attach_read_loop_observations(stats, commands, output_records)
    _attach_output_record_observations(stats, output_records)
    _attach_frontier_context_observations(stats, commands, output_records)
    stats["commands"] = normalized[:12]
    return stats


def _initial_command_flow_stats(
    commands: list[str],
    normalized: list[str],
) -> dict[str, Any]:
    repeated = sum(count - 1 for count in Counter(normalized).values() if count > 1)
    return {
        "aspCommands": len(commands),
        "searchCommands": 0,
        "queryCommands": 0,
        "guideCommands": 0,
        "directReadCommands": 0,
        "directReadBoundedCommands": 0,
        "directReadBroadCommands": 0,
        "directReadUnboundedCommands": 0,
        "directReadRiskCommands": 0,
        "searchPlaybookCommands": 0,
        "querySelectorCommands": 0,
        "treesitterQueryCommands": 0,
        "repeatedCommands": repeated,
    }


def _flow_commands(
    output_records: list[dict[str, Any]],
    structured_commands: list[str],
) -> list[str]:
    if not output_records:
        return structured_commands
    record_commands = {
        str(record["command"])
        for record in output_records
        if isinstance(record.get("command"), str)
    }
    commands = _executed_commands_from_output_records(output_records)
    commands.extend(
        command
        for command in structured_commands
        if normalize_command(command) not in record_commands
    )
    return commands


def _executed_commands_from_output_records(
    output_records: list[dict[str, Any]],
) -> list[str]:
    return [
        str(record["command"])
        for record in output_records
        if isinstance(record.get("command"), str) and not record.get("denied")
    ]


def _attach_denied_command_observations(
    stats: dict[str, Any],
    output_records: list[dict[str, Any]],
) -> None:
    denied = [record for record in output_records if record.get("denied")]
    if not denied:
        return
    stats["deniedAspCommands"] = len(denied)
    feedback = sorted(
        {
            str(record["hookFeedback"])
            for record in denied
            if isinstance(record.get("hookFeedback"), str)
        }
    )
    if feedback:
        stats["deniedHookFeedback"] = feedback
    stats["deniedCommands"] = [
        str(record["command"])
        for record in denied[:8]
        if isinstance(record.get("command"), str)
    ]


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
        _attach_binary_provenance(stats, output_records)
        stats["aspCommandOutputBytes"] = sum(
            record["outputBytes"] for record in output_records
        )
        stats["aspCommandOutputRecords"] = [
            _public_output_record(record) for record in output_records[:12]
        ]


def _attach_binary_provenance(
    stats: dict[str, Any],
    output_records: list[dict[str, Any]],
) -> None:
    binaries = [
        record["aspBinary"]
        for record in output_records
        if isinstance(record.get("aspBinary"), dict)
    ]
    if not binaries:
        return
    kind_counts = Counter(
        str(binary.get("kind")) for binary in binaries if binary.get("kind")
    )
    token_counts = Counter(
        str(binary.get("token")) for binary in binaries if binary.get("token")
    )
    workspace_count = sum(
        count
        for kind, count in kind_counts.items()
        if kind in {"project-bin", "cargo-target"}
    )
    risk_count = sum(
        count
        for kind, count in kind_counts.items()
        if kind not in {"project-bin", "cargo-target"}
    )
    stats["aspBinaryProvenance"] = {
        "commandCount": len(binaries),
        "workspaceBinaryCommands": workspace_count,
        "freshnessRiskCommands": risk_count,
        "kindCounts": dict(sorted(kind_counts.items())),
        "tokens": dict(sorted(token_counts.items())),
    }


def _attach_frontier_context_observations(
    stats: dict[str, Any],
    commands: list[str],
    output_records: list[dict[str, Any]],
) -> None:
    metrics = frontier_context_metrics(commands, output_records)
    if metrics:
        stats.update(metrics)


def _public_output_record(record: dict[str, Any]) -> dict[str, Any]:
    public = {
        key: value
        for key, value in record.items()
        if key != "output"
        and (key != "hookFeedback" or value is not None)
        and (key != "denied" or value)
    }
    preview = _safe_output_preview(record)
    if preview:
        public["outputPreview"] = preview
    return public


def _safe_output_preview(record: dict[str, Any]) -> str:
    output = record.get("output")
    command = record.get("command")
    if not isinstance(output, str) or not isinstance(command, str):
        return ""
    if not record.get("denied") and _command_may_emit_code(command):
        return ""
    preview = " ".join(output.split())
    return preview if len(preview) <= 180 else f"{preview[:177]}..."


def _command_may_emit_code(command: str) -> bool:
    args = asp_args(command)
    return "--code" in args or "direct-source-read" in args


def _classify_asp_command(command: str, stats: dict[str, Any]) -> None:
    args = asp_args(command)
    if len(args) < 2:
        return
    surface = args[1]
    if surface == "search":
        stats["searchCommands"] += 1
        if args[2:3] == ["playbook"]:
            stats["searchPlaybookCommands"] += 1
    elif surface == "query":
        stats["queryCommands"] += 1
        if "--selector" in args:
            stats["querySelectorCommands"] += 1
        if "--treesitter-query" in args:
            stats["treesitterQueryCommands"] += 1
        if "direct-source-read" in args:
            stats["directReadCommands"] += 1
            direct_read_shape = direct_source_read_shape(args)
            if direct_read_shape == "bounded":
                stats["directReadBoundedCommands"] += 1
            elif direct_read_shape == "broad":
                stats["directReadBroadCommands"] += 1
                stats["directReadRiskCommands"] += 1
            else:
                stats["directReadUnboundedCommands"] += 1
                stats["directReadRiskCommands"] += 1
    elif surface == "guide":
        stats["guideCommands"] += 1
