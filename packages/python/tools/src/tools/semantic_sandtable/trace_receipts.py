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


def _project_source(value: str) -> str:
    return value if value in _PROJECT_SOURCES else "unknown"
