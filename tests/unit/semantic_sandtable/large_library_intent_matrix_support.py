# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Support helpers for large-library intent matrix tests."""

from __future__ import annotations

import unittest
from pathlib import Path
from typing import Any

REQUIRED_LANGUAGES = {"julia", "python", "rust", "typescript"}
REQUIRED_INTENTS = {
    "feature-implementation",
    "api-usage",
    "implementation-principle",
}


def _dict_value(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _assert_search_playbook_commands(
    command_by_step_id: dict[str, list[str]],
    language: str,
    path: Path,
) -> None:
    if language not in {"julia", "python", "rust", "typescript"}:
        raise AssertionError(f"{path}: unsupported large-library language {language}")
    for step_id, command in command_by_step_id.items():
        if not command:
            raise AssertionError(f"{path}: {step_id} command must not be empty")
        if command[:3] != ["asp", "search", "playbook"]:
            raise AssertionError(
                f"{path}: {step_id} must use asp search playbook, got {' '.join(command)}"
            )
        try:
            selected_language = command[command.index("--language") + 1]
        except (ValueError, IndexError) as error:
            raise AssertionError(
                f"{path}: {step_id} must select --language {language}"
            ) from error
        if selected_language != language:
            raise AssertionError(
                f"{path}: {step_id} selects {selected_language}, expected {language}"
            )
        for required_axis in ("--rg", "--tantivy"):
            if required_axis not in command:
                raise AssertionError(
                    f"{path}: {step_id} is missing required {required_axis} input"
                )
        for removed_option in ("--view", "--seeds", "--query", "--query-set"):
            if removed_option in command:
                raise AssertionError(
                    f"{path}: {step_id} retains removed option {removed_option}"
                )


def _assert_search_playbook_steps_assert_gql(
    scenario: dict[str, Any], path: Path
) -> None:
    for step in _list_value(scenario.get("steps")):
        step_mapping = _dict_value(step)
        if not step_mapping:
            continue
        command = [str(part) for part in _list_value(step_mapping.get("command"))]
        if not _is_search_playbook_command(command):
            continue
        expect = _dict_value(step_mapping.get("expect"))
        stdout_contains = [
            str(item) for item in _list_value(expect.get("stdoutContains"))
        ]
        step_id = step_mapping.get("id", "<unknown>")
        if "lineProtocol" in expect:
            raise AssertionError(
                f"{path}: {step_id} must not validate the removed compact line protocol"
            )
        required = {
            "#+begin_src gql :name result",
            f"({scenario['language']}:Language)-[:RESULTS]->[",
            "#+end_src",
        }
        missing = required - set(stdout_contains)
        if missing:
            raise AssertionError(
                f"{path}: {step_id} must assert GQL result framing {sorted(missing)}"
            )
        stdout_not_contains = {
            str(item) for item in _list_value(expect.get("stdoutNotContains"))
        }
        if "MaterializationSet" not in stdout_not_contains:
            raise AssertionError(
                f"{path}: {step_id} must reject removed MaterializationSet output"
            )


def _assert_intent_uses_search_playbook(
    command_by_step_id: dict[str, list[str]],
    case_step_ids: list[str],
    path: Path,
) -> None:
    intent_commands = [
        command_by_step_id[step_id]
        for step_id in case_step_ids
        if step_id in command_by_step_id
    ]
    if not intent_commands:
        raise AssertionError(f"{path}: intent must reference a Search Playbook step")
    if not any(_is_search_playbook_command(command) for command in intent_commands):
        rendered = [" ".join(command) for command in intent_commands]
        raise AssertionError(
            f"{path}: intent search must use asp search playbook: {rendered}"
        )


def _is_search_playbook_command(command: list[str]) -> bool:
    return command[:3] == ["asp", "search", "playbook"]


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
