"""Large-library intent matrix coverage tests."""

from __future__ import annotations

import unittest
from collections import defaultdict
from pathlib import Path

from tools.semantic_sandtable.scenario_io import discover_scenarios, load_scenario

from .large_library_intent_matrix_support import (
    REQUIRED_INTENTS as _REQUIRED_INTENTS,
    REQUIRED_LANGUAGES as _REQUIRED_LANGUAGES,
    _assert_intent_uses_query_set,
    _assert_prime_steps_include_entries_and_status,
    _assert_provider_binary_commands,
    _assert_query_set_steps_include_entries,
    _dict_value,
    _list_value,
    _required_str,
    _required_str_list,
)


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]
_REQUIRED_SEARCH_SUBCOMMANDS_BY_LANGUAGE = {
    "julia": {"deps", "lexical", "owner", "prime"},
    "python": {"deps", "lexical", "owner", "prime"},
    "rust": {"deps", "lexical", "owner", "prime"},
    "typescript": {"deps", "lexical", "owner", "prime"},
}


def _search_subcommands(command_by_step_id: dict[str, list[str]]) -> set[str]:
    subcommands: set[str] = set()
    for command in command_by_step_id.values():
        if "search" in command:
            search_index = command.index("search")
            if command[search_index + 1 : search_index + 2]:
                subcommands.add(command[search_index + 1])
    return subcommands


def _assert_language_search_subcommand_coverage(
    subcommands_by_language: dict[str, set[str]],
) -> None:
    for language, required_subcommands in sorted(
        _REQUIRED_SEARCH_SUBCOMMANDS_BY_LANGUAGE.items()
    ):
        actual_subcommands = subcommands_by_language.get(language, set())
        missing = required_subcommands - actual_subcommands
        if missing:
            raise AssertionError(
                f"{language} large-library scenarios must cover search subcommands "
                f"{sorted(missing)}; actual={sorted(actual_subcommands)}"
            )


class LargeLibraryIntentMatrixTests(unittest.TestCase):
    def test_each_language_has_three_large_libraries_with_all_intents(self) -> None:
        matrix: dict[str, dict[str, set[str]]] = defaultdict(lambda: defaultdict(set))
        search_subcommands_by_language: dict[str, set[str]] = defaultdict(set)

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
            _assert_prime_steps_include_entries_and_status(scenario, path)
            search_subcommands_by_language[language].update(
                _search_subcommands(command_by_step_id)
            )
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
        _assert_language_search_subcommand_coverage(search_subcommands_by_language)
