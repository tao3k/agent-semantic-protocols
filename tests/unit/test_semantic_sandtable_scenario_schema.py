from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


class SemanticSandtableScenarioSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

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
                        }
                    },
                }
            ],
        }

        self.assertEqual([], self.validation_errors(scenario))


if __name__ == "__main__":
    unittest.main()
