"""Extract ASP command invocations from agent messages."""

from __future__ import annotations

import hashlib
import re
from typing import Any

from .agent_observation_asp import (
    command_contains_asp as _command_contains_asp,
    normalize_command as _normalize_command,
)
from .agent_observation_command_precision import (
    failure_frontier_precision_facts as _failure_frontier_precision_facts,
    search_pipe_precision_facts as _search_pipe_precision_facts,
)
from .agent_observation_failure_memory import (
    failure_frontier_memory as _failure_frontier_memory,
)
from .agent_observation_json import walk

_ASP_TEXT_COMMAND = re.compile(
    r"(?<![A-Za-z0-9_./-])"
    r"(?:(?:direnv\s+exec\s+\S+\s+)|(?:cd\s+\S+\s+&&\s+))?"
    r"(?:asp|\./\.bin/asp|\.bin/asp|/[^\s]+/\.bin/asp)\s+[^\n\r`]+"
)


def asp_commands_from_messages(messages: list[dict[str, Any]]) -> list[str]:
    structured = [
        command
        for command in _structured_command_values(messages)
        if _command_contains_asp(command)
    ]
    if structured:
        return structured
    text_commands: list[str] = []
    for value in walk(messages):
        if isinstance(value, str):
            text_commands.extend(
                match.group(0).strip() for match in _ASP_TEXT_COMMAND.finditer(value)
            )
    return text_commands


def asp_command_output_records_from_messages(
    messages: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    command_by_tool_id: dict[str, str] = {}
    records: list[dict[str, Any]] = []
    for value in walk(messages):
        if not isinstance(value, dict):
            continue
        if _is_tool_use_block(value):
            tool_id = value.get("id")
            command = _tool_use_command(value)
            if isinstance(tool_id, str) and command and _command_contains_asp(command):
                command_by_tool_id[tool_id] = command
        elif _is_tool_result_block(value):
            tool_use_id = value.get("tool_use_id")
            if not isinstance(tool_use_id, str):
                continue
            command = command_by_tool_id.get(tool_use_id)
            if not command:
                continue
            output = _tool_result_text(value.get("content"))
            records.append(
                {
                    "command": _normalize_command(command),
                    "output": output,
                    "outputBytes": len(output.encode()),
                    "outputLines": len(output.splitlines()),
                    "outputFingerprint": _text_fingerprint(output),
                    "precision": _search_pipe_precision_facts(command, output),
                    "failurePrecision": _failure_frontier_precision_facts(
                        command, output
                    ),
                    "failureMemory": _failure_frontier_memory(command, output),
                }
            )
    return records


def _structured_command_values(value: Any) -> list[str]:
    commands: list[str] = []
    if isinstance(value, dict):
        for key, item in value.items():
            if key == "command" and isinstance(item, str):
                commands.append(item)
            else:
                commands.extend(_structured_command_values(item))
    elif isinstance(value, list):
        for item in value:
            commands.extend(_structured_command_values(item))
    return commands


def _tool_use_command(value: dict[str, Any]) -> str:
    tool_input = value.get("input")
    if isinstance(tool_input, dict):
        command = tool_input.get("command")
        if isinstance(command, str):
            return command
    return ""


def _is_tool_use_block(value: dict[str, Any]) -> bool:
    return value.get("type") == "tool_use" or (
        isinstance(value.get("id"), str)
        and isinstance(value.get("input"), dict)
        and "command" in value.get("input", {})
    )


def _is_tool_result_block(value: dict[str, Any]) -> bool:
    return value.get("type") == "tool_result" or (
        isinstance(value.get("tool_use_id"), str) and "content" in value
    )


def _tool_result_text(value: Any) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        text = value.get("text")
        if isinstance(text, str):
            return text
        return "".join(_tool_result_text(item) for item in value.values())
    if isinstance(value, list):
        return "".join(_tool_result_text(item) for item in value)
    return ""


def _text_fingerprint(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()
