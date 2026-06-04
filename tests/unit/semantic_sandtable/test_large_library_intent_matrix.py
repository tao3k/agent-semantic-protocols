"""Large-library intent matrix coverage tests."""

from __future__ import annotations

import unittest
from collections import defaultdict
from pathlib import Path
from typing import Any

from tools.semantic_sandtable.scenario_io import discover_scenarios, load_scenario


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]
_REQUIRED_LANGUAGES = {"python", "rust", "typescript"}
_REQUIRED_INTENTS = {
    "feature-implementation",
    "api-usage",
    "implementation-principle",
}
_PROVIDER_BINARY_BY_LANGUAGE = {
    "python": "py-harness",
    "rust": "rs-harness",
    "typescript": "ts-harness",
}


class LargeLibraryIntentMatrixTests(unittest.TestCase):
    def test_each_language_has_three_large_libraries_with_all_intents(self) -> None:
        matrix: dict[str, dict[str, set[str]]] = defaultdict(lambda: defaultdict(set))

        for path in discover_scenarios(_PROTOCOL_REPO_ROOT, []):
            scenario = load_scenario(path, _PROTOCOL_REPO_ROOT)
            evidence = _dict_value(scenario.get("evidence"))
            if evidence.get("fixtureTier") != "large-library":
                continue
            self.assertIn("large-library", scenario.get("coverage", []), str(path))
            language = _required_str(scenario, "language", path)
            target_library = _dict_value(evidence.get("targetLibrary"))
            self.assertEqual(language, target_library.get("language"), str(path))
            library_name = _required_str(target_library, "package", path)
            _required_str(target_library, "repository", path)
            command_by_step_id: dict[str, list[str]] = {}
            for step in _list_value(scenario.get("steps")):
                step_mapping = _dict_value(step)
                if not step_mapping:
                    continue
                step_id = _required_str(step_mapping, "id", path)
                command_by_step_id[step_id] = [
                    str(part) for part in _list_value(step_mapping.get("command"))
                ]
            _assert_provider_binary_commands(command_by_step_id, language, path)
            _assert_query_set_steps_include_entries(scenario, path)
            step_ids = set(command_by_step_id)
            for intent_case in _list_value(evidence.get("intentCases")):
                case = _dict_value(intent_case)
                intent_kind = _required_str(case, "intentKind", path)
                self.assertIn(intent_kind, _REQUIRED_INTENTS, str(path))
                case_step_ids = _required_str_list(case, "stepIds", path)
                self.assertTrue(
                    set(case_step_ids).issubset(step_ids),
                    f"{path}: intent case references unknown steps {case_step_ids}",
                )
                _assert_intent_uses_query_set(command_by_step_id, case_step_ids, path)
                query_terms = _required_str_list(case, "queryTerms", path)
                command_text = " ".join(
                    " ".join(command_by_step_id[step_id])
                    for step_id in case_step_ids
                    if step_id in command_by_step_id
                )
                for query_term in query_terms:
                    self.assertIn(
                        query_term,
                        command_text,
                        f"{path}: query term {query_term!r} is not present in "
                        f"the referenced step commands {case_step_ids}",
                    )
                matrix[language][library_name].add(intent_kind)

        for language in sorted(_REQUIRED_LANGUAGES):
            self.assertGreaterEqual(
                len(matrix[language]),
                3,
                f"{language} needs at least three large-library fixtures",
            )
            for library_name, intents in sorted(matrix[language].items()):
                self.assertTrue(
                    _REQUIRED_INTENTS.issubset(intents),
                    f"{language}/{library_name} missing intents "
                    f"{sorted(_REQUIRED_INTENTS - intents)}",
                )


def _dict_value(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _assert_provider_binary_commands(
    command_by_step_id: dict[str, list[str]],
    language: str,
    path: Path,
) -> None:
    provider_binary = _PROVIDER_BINARY_BY_LANGUAGE.get(language)
    if provider_binary is None:
        raise AssertionError(f"{path}: unsupported large-library language {language}")
    for step_id, command in command_by_step_id.items():
        if not command:
            raise AssertionError(f"{path}: {step_id} command must not be empty")
        if command[0] != provider_binary:
            raise AssertionError(
                f"{path}: {step_id} must use {provider_binary}, got {' '.join(command)}"
            )
        if (
            len(command) >= 3
            and command[1] == "search"
            and command[2]
            in {
                "api",
                "text",
            }
        ):
            raise AssertionError(
                f"{path}: {step_id} must use search fzf/query-set, got {' '.join(command)}"
            )


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
        if not any(item.startswith("entries=") for item in stdout_contains):
            step_id = step_mapping.get("id", "<unknown>")
            raise AssertionError(
                f"{path}: {step_id} query-set --view seeds step must assert compact graph entries"
            )


def _is_seed_view_command(command: list[str]) -> bool:
    for index, arg in enumerate(command):
        if arg == "--view=seeds":
            return True
        if (
            arg == "--view"
            and index + 1 < len(command)
            and command[index + 1] == "seeds"
        ):
            return True
    return False


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
    if not any(_is_query_set_search(command) for command in intent_commands):
        rendered = [" ".join(command) for command in intent_commands]
        raise AssertionError(f"{path}: intent search must use query-set: {rendered}")


def _is_prime_command(command: list[str]) -> bool:
    return len(command) >= 3 and command[1:3] == ["search", "prime"]


def _is_query_set_search(command: list[str]) -> bool:
    return (
        len(command) >= 4
        and command[1:3] == ["search", "fzf"]
        and "--query-set" in command
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
