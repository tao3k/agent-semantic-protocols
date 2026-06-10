"""Build sandtable receipts from recorded agent command traces."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .receipts import (
    SEARCH_COMMAND_KINDS,
    receipt_command_output_mode,
    validate_receipt_consistency,
)
from .direct_read_shape import direct_source_read_shape
from .trace_receipt_events import TraceCommandFilter, trace_commands_from_path
from .utils import dict_value, optional_int


_PROJECT_SOURCES = {"checkout", "registry", "fixture", "unknown"}


@dataclass(frozen=True)
class TraceReceiptConfig:
    scenario_id: str
    language: str
    project_name: str
    intent: str
    edit_boundary: str = "before-edit"
    project_source: str = "checkout"
    recorded_at: str | None = None


def build_receipt_from_trace_path(
    trace_path: Path,
    *,
    config: TraceReceiptConfig,
    filters: TraceCommandFilter | None = None,
) -> dict[str, Any]:
    commands = trace_commands_from_path(trace_path, filters=filters)
    receipt: dict[str, Any] = {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
        "schemaVersion": "1",
        "scenarioId": config.scenario_id,
        "language": config.language,
        "project": {
            "name": config.project_name,
            "source": _project_source(config.project_source),
        },
        "intent": config.intent,
        "editBoundary": config.edit_boundary,
        "commands": commands,
        "summary": _summary(commands),
    }
    if config.recorded_at:
        receipt["recordedAt"] = config.recorded_at
    validate_receipt_consistency(receipt)
    return receipt


def write_receipt_from_trace_path(
    trace_path: Path,
    output_path: Path,
    *,
    config: TraceReceiptConfig,
    filters: TraceCommandFilter | None = None,
) -> dict[str, Any]:
    receipt = build_receipt_from_trace_path(trace_path, config=config, filters=filters)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as handle:
        json.dump(receipt, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return receipt


def _summary(commands: list[dict[str, Any]]) -> dict[str, int]:
    return {
        "commandCount": len(commands),
        "stdoutBytes": sum(_metric(command, "stdoutBytes") for command in commands),
        "stderrBytes": sum(_metric(command, "stderrBytes") for command in commands),
        "elapsedMs": sum(_metric(command, "elapsedMs") for command in commands),
        "aspCommands": _semantic_command_count(commands),
        "searchCommands": _semantic_command_count(commands, verb="search"),
        "queryCommands": _semantic_command_count(commands, verb="query"),
        "directReadCommands": _direct_read_command_count(commands),
        "directReadBoundedCommands": _direct_read_command_count(commands, "bounded"),
        "directReadBroadCommands": _direct_read_command_count(commands, "broad"),
        "directReadUnboundedCommands": _direct_read_command_count(
            commands, "unbounded"
        ),
        "directReadRiskCommands": _direct_read_risk_command_count(commands),
        "repeatedCommands": _repeated_semantic_command_count(commands),
        "repeatedSearches": _repeated_semantic_command_count(commands, verb="search"),
        "jsonSearches": _search_output_mode_count(commands, "json"),
        "compactSearches": _search_output_mode_count(commands, "compact"),
    }


def _metric(command: dict[str, Any], field: str) -> int:
    return optional_int(dict_value(command.get("metrics")).get(field)) or 0


def _search_output_mode_count(commands: list[dict[str, Any]], output_mode: str) -> int:
    return sum(
        1
        for command in commands
        if command.get("kind") in SEARCH_COMMAND_KINDS
        and receipt_command_output_mode(command) == output_mode
    )


def _semantic_command_count(
    commands: list[dict[str, Any]], *, verb: str | None = None
) -> int:
    return sum(
        1
        for command in commands
        if _is_semantic_command(command) and (verb is None or verb in _argv(command))
    )


def _direct_read_command_count(
    commands: list[dict[str, Any]], shape: str | None = None
) -> int:
    return sum(
        1
        for command in commands
        if _is_semantic_command(command)
        and "--from-hook" in _argv(command)
        and "direct-source-read" in _argv(command)
        and (shape is None or direct_source_read_shape(_argv(command)) == shape)
    )


def _direct_read_risk_command_count(commands: list[dict[str, Any]]) -> int:
    return _direct_read_command_count(commands, "broad") + _direct_read_command_count(
        commands, "unbounded"
    )


def _repeated_semantic_command_count(
    commands: list[dict[str, Any]], *, verb: str | None = None
) -> int:
    counts: dict[tuple[str, ...], int] = {}
    for command in commands:
        argv = _argv(command)
        if not _is_semantic_command(command) or (verb is not None and verb not in argv):
            continue
        key = tuple(argv)
        counts[key] = counts.get(key, 0) + 1
    return sum(count - 1 for count in counts.values() if count > 1)


def _is_semantic_command(command: dict[str, Any]) -> bool:
    argv = _argv(command)
    if not argv:
        return False
    binary = Path(argv[0]).name
    return binary == "asp" or binary.endswith("-harness")


def _argv(command: dict[str, Any]) -> list[str]:
    argv = command.get("argv")
    if not isinstance(argv, list):
        return []
    return [str(part) for part in argv]


def _project_source(value: str) -> str:
    return value if value in _PROJECT_SOURCES else "unknown"
