"""Schema tests for semantic sandtable scenarios."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


class SemanticSandtableScenarioSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
        )
        self.validator = Draft202012Validator(_load_json(schema_path))

    def validation_errors(self, scenario: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(scenario)]

    def test_guide_quality_route_command_assertions_are_valid(self) -> None:
        scenario: dict[str, object] = {
            "id": "typescript.codex-guide",
            "language": "typescript",
            "workdir": ".",
            "steps": [
                {
                    "id": "guide",
                    "command": ["node", "-e", "console.log('{}')"],
                    "expect": {
                        "guideQuality": {
                            "reasonKind": "raw-broad-search",
                            "languageId": "typescript",
                            "routeKind": "query",
                            "routeCommandContains": [
                                "ts-harness query --from-hook",
                                "--surface owners,tests",
                            ],
                            "routeCommandNotContains": [
                                "search query",
                                "--surface owner,tests",
                            ],
                            "outputContains": [
                                "entries=owner-query(O,Q=>items+tests+dependency-usage)"
                            ],
                            "outputNotContains": ["profiles" + "="],
                        }
                    },
                }
            ],
        }

        self.assertEqual([], self.validation_errors(scenario))

    def test_large_library_evidence_requires_complete_matrix_metadata(self) -> None:
        scenario: dict[str, object] = _large_library_scenario()

        self.assertEqual([], self.validation_errors(scenario))

    def test_large_library_evidence_requires_coverage_and_repository(self) -> None:
        scenario = _large_library_scenario()
        scenario.pop("coverage")
        evidence = scenario["evidence"]
        assert isinstance(evidence, dict)
        target = evidence["targetLibrary"]
        assert isinstance(target, dict)
        target.pop("repository")

        errors = self.validation_errors(scenario)

        self.assertIn("'coverage' is a required property", errors)
        self.assertIn("'repository' is a required property", errors)

    def test_large_library_evidence_rejects_unknown_workdir_kind(self) -> None:
        scenario = _large_library_scenario()
        evidence = scenario["evidence"]
        assert isinstance(evidence, dict)
        target = evidence["targetLibrary"]
        assert isinstance(target, dict)
        target["workdirKind"] = "unknown"

        errors = self.validation_errors(scenario)

        self.assertIn("'unknown' is not one of ['checkout', 'registry']", errors)

    def test_semantic_sandtable_scenario_schema_copies_stay_synchronized(self) -> None:
        root_schema = _load_json(
            _REPO_ROOT / "schemas/semantic-sandtable-scenario.v1.schema.json"
        )
        schema_copies = sorted(
            path
            for path in (_REPO_ROOT / "languages").glob(
                "*/schemas/semantic-sandtable-scenario.v1.schema.json"
            )
        )

        self.assertTrue(schema_copies)
        self.assertEqual(
            {schema_path: root_schema for schema_path in schema_copies},
            {schema_path: _load_json(schema_path) for schema_path in schema_copies},
        )


def _large_library_scenario() -> dict[str, object]:
    return {
        "id": "python.demo-large-library",
        "language": "python",
        "coverage": ["large-library"],
        "workdir": ".",
        "evidence": {
            "source": "handwritten",
            "fixtureTier": "large-library",
            "targetLibrary": {
                "language": "python",
                "name": "demo",
                "package": "demo",
                "repository": "example/demo",
                "workdirKind": "checkout",
            },
            "intentCases": [
                {
                    "intentKind": "feature-implementation",
                    "intent": "feature",
                    "stepIds": ["intent-query-set"],
                    "queryTerms": ["Feature"],
                },
                {
                    "intentKind": "api-usage",
                    "intent": "api",
                    "stepIds": ["intent-query-set"],
                    "queryTerms": ["Api"],
                },
                {
                    "intentKind": "implementation-principle",
                    "intent": "principle",
                    "stepIds": ["intent-query-set"],
                    "queryTerms": ["Principle"],
                },
            ],
        },
        "steps": [
            {
                "id": "intent-query-set",
                "command": [
                    "py-harness",
                    "search",
                    "fzf",
                    "--query-set",
                    "Feature",
                    "--query-set",
                    "Api",
                    "--query-set",
                    "Principle",
                    "--view",
                    "seeds",
                    ".",
                ],
            }
        ],
    }


if __name__ == "__main__":
    unittest.main()
