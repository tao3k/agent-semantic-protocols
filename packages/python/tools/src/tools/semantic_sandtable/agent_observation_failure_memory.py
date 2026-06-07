"""Memory records for ASP failure-frontier actions."""

from __future__ import annotations

from typing import Any

from .agent_observation_asp import asp_args, normalize_command


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
    if "=>asp " not in segment or " query --selector " not in segment:
        return None
    label, projected_command = segment.split("=>", 1)
    selector = _selector_from_projected_command(projected_command)
    if selector is None:
        return None
    return {
        "label": label.strip(),
        "selector": selector,
        "command": normalize_command(projected_command.strip()),
    }


def _failure_frontier_memory_packet(actions: list[dict[str, str]]) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.failure-loop-memory",
        "schemaVersion": "1",
        "policy": "failure-frontier-action-memory",
        "entryCount": len(actions),
        "entries": actions[:8],
    }


def _selector_from_projected_command(command: str) -> str | None:
    args = asp_args(command)
    for index, arg in enumerate(args):
        if arg == "--selector" and index + 1 < len(args):
            return args[index + 1]
        if arg.startswith("--selector="):
            return arg.split("=", 1)[1]
    return None


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
