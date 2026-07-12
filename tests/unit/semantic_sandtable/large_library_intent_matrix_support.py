"""Support helpers for large-library intent matrix tests."""

from __future__ import annotations

import unittest
from pathlib import Path
from typing import Any, Final

REQUIRED_LANGUAGES = {"julia", "python", "rust", "typescript"}
REQUIRED_INTENTS = {
    "feature-implementation",
    "api-usage",
    "implementation-principle",
}
_PROVIDER_BINARY_BY_LANGUAGE = {
    "julia": "asp-julia-harness",
    "python": "py-harness",
    "rust": "rs-harness",
    "typescript": "ts-harness",
}


def _dict_value(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _assert_provider_binary_commands(
    command_by_step_id: dict[str, list[str]],
    language: str,
    path: Path,
) -> None:
    if language not in {"julia", "python", "rust", "typescript"}:
        raise AssertionError(f"{path}: unsupported large-library language {language}")
    for step_id, command in command_by_step_id.items():
        if not command:
            raise AssertionError(f"{path}: {step_id} command must not be empty")
        if len(command) < 2 or command[0] != "asp" or command[1] != language:
            raise AssertionError(
                f"{path}: {step_id} must use asp {language}, got {' '.join(command)}"
            )
        command_offset = 2
        if (
            len(command) >= command_offset + 3
            and command[command_offset] == "search"
            and command[command_offset + 1]
            in {
                "api",
                "text",
            }
        ):
            raise AssertionError(
                f"{path}: {step_id} must use search lexical/query-set, got {' '.join(command)}"
            )


_KNOWN_REASONING_PROFILE_NAMES: Final[set[str]] = {
    "owner-query",
    "query-deps",
    "owner-tests",
    "finding-frontier",
    "feature-cfg",
}


def _assert_query_set_steps_include_entries(
    scenario: dict[str, Any], path: Path
) -> None:
    for step in _list_value(scenario.get("steps")):
        step_mapping = _dict_value(step)
        if not step_mapping:
            continue
        command = [str(part) for part in _list_value(step_mapping.get("command"))]
        if not (_is_seed_view_command(command) and _is_query_set_search(command)):
            continue
        expect = _dict_value(step_mapping.get("expect"))
        stdout_contains = [
            str(item) for item in _list_value(expect.get("stdoutContains"))
        ]
        step_id = step_mapping.get("id", "<unknown>")
        if expect.get("lineProtocol") is not True:
            raise AssertionError(
                f"{path}: {step_id} query-set --view seeds step must enable lineProtocol compact graph validation"
            )
        if not any(item.startswith("entries=") for item in stdout_contains):
            raise AssertionError(
                f"{path}: {step_id} query-set --view seeds step must assert compact graph entries"
            )
        for entry_line in stdout_contains:
            if not entry_line.startswith("entries=") or entry_line == "entries=":
                continue
            for segment in entry_line.removeprefix("entries=").split(")"):
                if "(" not in segment:
                    continue
                profile_name = segment.lstrip(",").split("(", 1)[0]
                if profile_name not in _KNOWN_REASONING_PROFILE_NAMES:
                    raise AssertionError(
                        f"{path}: {step_id} entries profile {profile_name!r} is not in the shared reasoning profile catalog"
                    )


def _assert_prime_steps_include_entries_and_status(
    scenario: dict[str, Any], path: Path
) -> None:
    required_status_fields = [
        "analysis=structure",
        "nativeSyntaxFacts=skipped",
        "policyFindings=skipped",
    ]
    for step in _list_value(scenario.get("steps")):
        step_mapping = _dict_value(step)
        if not step_mapping:
            continue
        command = [str(part) for part in _list_value(step_mapping.get("command"))]
        if not (_is_seed_view_command(command) and _is_prime_command(command)):
            continue
        expect = _dict_value(step_mapping.get("expect"))
        stdout_contains = [
            str(item) for item in _list_value(expect.get("stdoutContains"))
        ]
        step_id = step_mapping.get("id", "<unknown>")
        if expect.get("lineProtocol") is not True:
            raise AssertionError(
                f"{path}: {step_id} prime --view seeds step must enable lineProtocol compact graph validation"
            )
        has_entries = any(item.startswith("entries=") for item in stdout_contains)
        has_budgeted_prime_frontier = (
            "alg=budgeted-prime-frontier-v1" in stdout_contains
            and "|decision purpose=decision-primer" in stdout_contains
            and "omit=items,blocks,code,full-test-list" in stdout_contains
            and "avoid=raw-read" in stdout_contains
        )
        if not has_entries and not has_budgeted_prime_frontier:
            raise AssertionError(
                f"{path}: {step_id} prime --view seeds step must assert compact graph entries or budgeted prime frontier controls"
            )
        for entry_line in stdout_contains:
            if not entry_line.startswith("entries=") or entry_line == "entries=":
                continue
            for segment in entry_line.removeprefix("entries=").split(")"):
                if "(" not in segment:
                    continue
                profile_name = segment.lstrip(",").split("(", 1)[0]
                if profile_name not in _KNOWN_REASONING_PROFILE_NAMES:
                    raise AssertionError(
                        f"{path}: {step_id} entries profile {profile_name!r} is not in the shared reasoning profile catalog"
                    )
        if "analysis=structure" in stdout_contains:
            for field in required_status_fields:
                if field not in stdout_contains:
                    raise AssertionError(
                        f"{path}: {step_id} optimized prime step must assert {field}"
                    )

def _is_seed_view_command(command: list[str]) -> bool:
    return "--view=seeds" in command or any(
        arg == "--view" and command[index + 1 : index + 2] == ["seeds"]
        for index, arg in enumerate(command)
    )


def _assert_intent_uses_query_set(
    command_by_step_id: dict[str, list[str]],
    case_step_ids: list[str],
    path: Path,
) -> None:
    intent_commands = [
        command_by_step_id[step_id]
        for step_id in case_step_ids
        if not _is_prime_command(command_by_step_id[step_id])
    ]
    if not intent_commands:
        raise AssertionError(f"{path}: intent must reference a non-prime search step")
    if not any(
        _is_query_set_search(command, include_owner_items=True)
        for command in intent_commands
    ):
        rendered = [" ".join(command) for command in intent_commands]
        raise AssertionError(
            f"{path}: intent search must use query-set or owner-items: {rendered}"
        )


def _search_offset(command: list[str]) -> int | None:
    for index, part in enumerate(command):
        if part == "search":
            return index
    return None


def _is_prime_command(command: list[str]) -> bool:
    search_offset = _search_offset(command)
    return search_offset is not None and command[
        search_offset + 1 : search_offset + 2
    ] == ["prime"]


def _is_query_set_search(
    command: list[str], *, include_owner_items: bool = False
) -> bool:
    search_offset = _search_offset(command)
    if search_offset is None or len(command) <= search_offset + 1:
        return False
    subcommand = command[search_offset + 1]
    if subcommand == "lexical" and "--query-set" in command:
        return True
    return (
        include_owner_items
        and (
            subcommand == "pipe"
            or (
                subcommand == "owner"
                and "items" in command[search_offset + 2 :]
                and "--query" in command
            )
        )
    )






def _list_value(value: Any) -> list[Any]:
    return value if isinstance(value, list) else []


def _required_str(mapping: dict[str, Any], key: str, path: Path) -> str:
    value = mapping.get(key)
    if not isinstance(value, str) or not value:
        raise AssertionError(f"{path}: {key} must be a non-empty string")
    return value


def _required_str_list(mapping: dict[str, Any], key: str, path: Path) -> list[str]:
    value = mapping.get(key)
    if not isinstance(value, list) or not value:
        raise AssertionError(f"{path}: {key} must be a non-empty list")
    result = []
    for item in value:
        if not isinstance(item, str) or not item:
            raise AssertionError(f"{path}: {key} entries must be non-empty strings")
        result.append(item)
    return result


if __name__ == "__main__":
    unittest.main()
