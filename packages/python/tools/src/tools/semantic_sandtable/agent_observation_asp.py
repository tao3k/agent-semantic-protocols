"""Shared ASP command argument helpers for agent observations."""

from __future__ import annotations

import shlex


def asp_args(command: str) -> list[str]:
    try:
        parts = shlex.split(command)
    except ValueError:
        parts = command.split()
    for index, part in enumerate(parts):
        if _is_asp_binary_token(part):
            return parts[index + 1 :]
    return []


def command_contains_asp(command: str) -> bool:
    return bool(asp_args(command))


def normalize_command(command: str) -> str:
    return " ".join(command.strip().split())


def _is_asp_binary_token(value: str) -> bool:
    return value == "asp" or value.endswith("/asp") or value.endswith(".bin/asp")
