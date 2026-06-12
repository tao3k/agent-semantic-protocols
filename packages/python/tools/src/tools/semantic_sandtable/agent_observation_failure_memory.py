"""Memory records for ASP failure-frontier actions."""

from __future__ import annotations

from typing import Any

from .agent_observation_asp import asp_args


def failure_frontier_memory(command: str, output: str) -> dict[str, Any]:
    args = asp_args(command)
    if len(args) < 3 or args[1:3] != ["search", "failure"]:
        return {}
    actions = _failure_frontier_actions(output)
    if not actions:
        return {}
    return _failure_frontier_memory_packet(actions)


def _failure_frontier_actions(output: str) -> list[dict[str, str]]:
    actions = []
    for line in output.splitlines():
        value = line.removeprefix("frontierActions=")
        if value == line:
            continue
        actions.extend(_projected_query_actions(value))
    return actions


def _projected_query_actions(value: str) -> list[dict[str, str]]:
    actions = []
    for segment in _action_segments(value):
        action = _projected_query_action(segment)
        if action is not None:
            actions.append(action)
    return actions


def _projected_query_action(segment: str) -> dict[str, str] | None:
    typed_action = _typed_query_code_action(segment)
    if typed_action is not None:
        return typed_action
    return None


def _typed_query_code_action(segment: str) -> dict[str, str] | None:
    label, separator, rest = segment.strip().partition(".query-code(")
    if not separator:
        return None
    fields_text, separator, next_action = rest.partition(")!")
    if not separator or next_action != "query-code":
        return None
    fields = _action_fields(fields_text)
    selector = fields.get("selector")
    if selector is None:
        return None
    return {
        "label": label.strip(),
        "selector": selector,
        "actionKind": "query-code",
        "targetRole": "selector",
    }


def _failure_frontier_memory_packet(actions: list[dict[str, str]]) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.failure-loop-memory",
        "schemaVersion": "1",
        "policy": "failure-frontier-action-memory",
        "entryCount": len(actions),
        "entries": actions[:8],
    }


def _action_fields(fields_text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for field in _action_segments(fields_text):
        key, separator, value = field.strip().partition("=")
        if separator:
            fields[key] = value
    return fields


def _action_segments(value: str) -> list[str]:
    segments: list[str] = []
    depth = 0
    start = 0
    for index, character in enumerate(value):
        if character == "(":
            depth += 1
        elif character == ")":
            depth = max(depth - 1, 0)
        elif character == "," and depth == 0:
            segments.append(value[start:index])
            start = index + 1
    segments.append(value[start:])
    return segments
