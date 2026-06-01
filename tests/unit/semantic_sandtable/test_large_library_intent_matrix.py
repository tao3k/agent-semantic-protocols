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
            command_by_step_id: dict[str, list[str]] = {}
            for step in _list_value(scenario.get("steps")):
                step_mapping = _dict_value(step)
                if not step_mapping:
                    continue
                step_id = _required_str(step_mapping, "id", path)
                command_by_step_id[step_id] = [
                    str(part) for part in _list_value(step_mapping.get("command"))
                ]
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
