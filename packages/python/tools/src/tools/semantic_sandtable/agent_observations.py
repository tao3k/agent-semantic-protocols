"""Extract compact agent observations from Claude SDK sandtable output."""

from __future__ import annotations

import json
import re
import shlex
from collections import Counter
from typing import Any


_ASP_TEXT_COMMAND = re.compile(
    r"(?:(?:direnv\s+exec\s+\S+\s+)|(?:cd\s+\S+\s+&&\s+))?"
    r"(?:asp|\./\.bin/asp|\.bin/asp|/[^\s]+/\.bin/asp)\s+[^\n\r`]+"
)

_TOKEN_FIELDS = {
    "input_tokens": "inputTokens",
    "inputTokens": "inputTokens",
    "output_tokens": "outputTokens",
    "outputTokens": "outputTokens",
    "cache_creation_input_tokens": "cacheCreationInputTokens",
    "cacheCreationInputTokens": "cacheCreationInputTokens",
    "cache_read_input_tokens": "cacheReadInputTokens",
    "cacheReadInputTokens": "cacheReadInputTokens",
    "cache_write_input_tokens": "cacheWriteInputTokens",
    "cacheWriteInputTokens": "cacheWriteInputTokens",
}
_COST_FIELDS = {"costUsd", "cost_usd", "totalCostUsd", "total_cost_usd"}


def summarize_agent_stdout(stdout: str) -> dict[str, Any]:
    messages = _load_stdout_messages(stdout)
    if not messages:
        return {}
    if summary := _last_summary(messages):
        return summary
    return summarize_agent_messages(messages)


def summarize_agent_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    summary: dict[str, Any] = {"type": "SandtableAgentSdkSummary"}
    token_cost = token_cost_from_messages(messages)
    if token_cost:
        summary["tokenCost"] = token_cost
    pipe_flow = pipe_flow_from_messages(messages)
    if pipe_flow:
        summary["pipeFlow"] = pipe_flow
    return summary


def token_cost_from_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    totals = {field: 0 for field in set(_TOKEN_FIELDS.values())}
    usage_records = 0
    costs: list[float] = []
    for value in _walk(messages):
        if isinstance(value, dict):
            if _looks_like_usage(value):
                usage_records += 1
                for source, target in _TOKEN_FIELDS.items():
                    amount = _int_value(value.get(source))
                    if amount is not None:
                        totals[target] += amount
            for cost_field in _COST_FIELDS:
                cost = _float_value(value.get(cost_field))
                if cost is not None:
                    costs.append(cost)

    compact = {key: value for key, value in sorted(totals.items()) if value}
    if not compact and not costs:
        return {}
    compact["totalTokens"] = sum(compact.values())
    compact["usageRecords"] = usage_records
    if costs:
        compact["costUsd"] = max(costs)
    compact["source"] = "claude-sdk-stream"
    return compact


def pipe_flow_from_messages(messages: list[dict[str, Any]]) -> dict[str, Any]:
    commands = _asp_commands_from_messages(messages)
    if not commands:
        return {}
    normalized = [_normalize_command(command) for command in commands]
    repeated = sum(count - 1 for count in Counter(normalized).values() if count > 1)
    stats = {
        "aspCommands": len(commands),
        "searchCommands": 0,
        "queryCommands": 0,
        "checkCommands": 0,
        "directReadCommands": 0,
        "searchPipeCommands": 0,
        "searchPrimeCommands": 0,
        "searchFzfCommands": 0,
        "searchReasoningCommands": 0,
        "querySelectorCommands": 0,
        "treesitterQueryCommands": 0,
        "repeatedCommands": repeated,
    }
    for command in commands:
        _classify_asp_command(command, stats)

    missing = []
    if stats["searchPrimeCommands"] == 0:
        missing.append("search-prime")
    if stats["searchPipeCommands"] == 0:
        missing.append("search-pipe")
    if stats["querySelectorCommands"] == 0:
        missing.append("query-selector")
    stats["complexPipeFlow"] = not missing
    stats["missingComplexPipeStages"] = missing
    stats["commands"] = normalized[:12]
    return stats


def _classify_asp_command(command: str, stats: dict[str, Any]) -> None:
    args = _asp_args(command)
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


def _asp_args(command: str) -> list[str]:
    try:
        parts = shlex.split(command)
    except ValueError:
        parts = command.split()
    for index, part in enumerate(parts):
        if _is_asp_binary_token(part):
            return parts[index + 1 :]
    return []


def _is_asp_binary_token(value: str) -> bool:
    return value == "asp" or value.endswith("/asp") or value.endswith(".bin/asp")


def _asp_commands_from_messages(messages: list[dict[str, Any]]) -> list[str]:
    structured = [
        command
        for command in _structured_command_values(messages)
        if _command_contains_asp(command)
    ]
    if structured:
        return structured
    text_commands: list[str] = []
    for value in _walk(messages):
        if isinstance(value, str):
            text_commands.extend(
                match.group(0).strip() for match in _ASP_TEXT_COMMAND.finditer(value)
            )
    return text_commands


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


def _command_contains_asp(command: str) -> bool:
    return bool(_asp_args(command))


def _normalize_command(command: str) -> str:
    return " ".join(command.strip().split())


def _load_stdout_messages(stdout: str) -> list[dict[str, Any]]:
    stripped = stdout.strip()
    if not stripped:
        return []
    try:
        payload = json.loads(stripped)
    except json.JSONDecodeError:
        payload = None
    if isinstance(payload, list):
        return [item for item in payload if isinstance(item, dict)]
    if isinstance(payload, dict):
        return [payload]

    messages: list[dict[str, Any]] = []
    for line in stdout.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict):
            messages.append(item)
    return messages


def _last_summary(messages: list[dict[str, Any]]) -> dict[str, Any]:
    for message in reversed(messages):
        if message.get("type") == "SandtableAgentSdkSummary":
            return message
    return {}


def _looks_like_usage(value: dict[str, Any]) -> bool:
    return any(field in value for field in _TOKEN_FIELDS)


def _walk(value: Any):
    yield value
    if isinstance(value, dict):
        for item in value.values():
            yield from _walk(item)
    elif isinstance(value, list):
        for item in value:
            yield from _walk(item)


def _int_value(value: Any) -> int | None:
    if isinstance(value, bool):
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _float_value(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None
