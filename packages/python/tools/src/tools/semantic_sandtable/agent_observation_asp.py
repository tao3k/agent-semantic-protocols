"""Shared ASP command argument helpers for agent observations."""

from __future__ import annotations

import shlex


def asp_args(command: str) -> list[str]:
    try:
        parts = shlex.split(command)
    except ValueError:
        parts = command.split()
    for index, part in enumerate(parts):
        if _is_asp_binary_token(part) and _is_asp_invocation_position(parts, index):
            return parts[index + 1 :]
    return []


def command_contains_asp(command: str) -> bool:
    return bool(asp_args(command))


def normalize_command(command: str) -> str:
    return " ".join(command.strip().split())


def _is_asp_binary_token(value: str) -> bool:
    return value == "asp" or value.endswith("/asp") or value.endswith(".bin/asp")


def _is_asp_invocation_position(parts: list[str], index: int) -> bool:
    if index == 0:
        return True
    previous = parts[index - 1]
    if previous in {"&&", ";", "|", "||", "rtk"}:
        return True
    if all(_is_env_assignment(part) for part in parts[:index]):
        return True
    if _is_env_assignment(previous):
        return True
    return index >= 3 and parts[index - 3 : index - 1] == ["direnv", "exec"]


def _is_env_assignment(value: str) -> bool:
    name, separator, _raw_value = value.partition("=")
    return bool(separator and name and not name.startswith("-") and name.replace("_", "").isalnum())
