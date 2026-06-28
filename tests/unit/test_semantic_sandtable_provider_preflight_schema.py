"""Validate provider preflight scenario contract shape."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


class SemanticSandtableProviderPreflightSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
        )
        self.validator = Draft202012Validator(_load_json(schema_path))

    def validation_errors(self, scenario: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(scenario)]

    def test_install_from_workspace_languages_is_valid(self) -> None:
        scenario: dict[str, object] = {
            "id": "python.provider-preflight",
            "language": "python",
            "workdir": ".",
            "providerPreflight": {"installFromWorkspaceLanguages": ["python", "julia"]},
            "steps": [
                {
                    "id": "probe",
                    "command": ["asp", "python", "query", "--help"],
                }
            ],
        }

        self.assertEqual([], self.validation_errors(scenario))

    def test_isolation_control_is_valid(self) -> None:
        scenario: dict[str, object] = {
            "id": "python.provider-preflight",
            "language": "python",
            "workdir": ".",
            "isolation": {"enabled": False},
            "steps": [
                {
                    "id": "probe",
                    "command": ["asp", "python", "query", "--help"],
                }
            ],
        }

        self.assertEqual([], self.validation_errors(scenario))

    def test_rejects_unknown_properties(self) -> None:
        scenario: dict[str, object] = {
            "id": "python.provider-preflight",
            "language": "python",
            "workdir": ".",
            "providerPreflight": {"installPinnedReleaseLanguages": ["python"]},
            "steps": [
                {
                    "id": "probe",
                    "command": ["asp", "python", "query", "--help"],
                }
            ],
        }

        self.assertTrue(
            any(
                "Additional properties are not allowed" in error
                for error in self.validation_errors(scenario)
            )
        )
