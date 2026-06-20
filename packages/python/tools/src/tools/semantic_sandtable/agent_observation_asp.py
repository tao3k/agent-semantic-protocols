"""Shared ASP command argument helpers for agent observations."""

from __future__ import annotations

import shlex


def asp_args(command: str) -> list[str]:
    parts = _split_command(command)
    index = _asp_binary_index(parts)
    if index is None:
        return []
    return parts[index + 1 :]


def asp_binary_provenance(command: str) -> dict[str, str]:
    token = asp_binary_token(command)
    if token is None:
        return {}
    return {
        "token": token,
        "kind": asp_binary_kind(token),
    }


def asp_binary_token(command: str) -> str | None:
    parts = _split_command(command)
    index = _asp_binary_index(parts)
    if index is None:
        return None
    return parts[index]


def asp_binary_kind(token: str) -> str:
    value = token.strip()
    if value == "asp":
        return "ambient-path"
    if value in {".bin/asp", "./.bin/asp"} or value.endswith("/.bin/asp"):
        return "project-bin"
    path_parts = [part for part in value.split("/") if part]
    if value.endswith("/asp") and "target" in path_parts:
        return "cargo-target"
    if value.startswith("/"):
        return "absolute-path"
    if "/" in value:
        return "relative-path"
    return "named-binary"


def _split_command(command: str) -> list[str]:
    try:
        return shlex.split(command)
    except ValueError:
        return command.split()


def _asp_binary_index(parts: list[str]) -> int | None:
    for index, part in enumerate(parts):
        if _is_asp_binary_token(part) and _is_asp_invocation_position(parts, index):
            return index
    return None


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
