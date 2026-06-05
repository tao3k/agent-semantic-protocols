from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from typing import Callable


_REPO_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_VALIDATION_PATH = _REPO_ROOT / "tests" / "unit" / "schema_validation.py"
_SCHEMA_VALIDATION_SPEC = importlib.util.spec_from_file_location(
    "schema_validation", _SCHEMA_VALIDATION_PATH
)
assert _SCHEMA_VALIDATION_SPEC is not None
assert _SCHEMA_VALIDATION_SPEC.loader is not None
_SCHEMA_VALIDATION_MODULE = importlib.util.module_from_spec(_SCHEMA_VALIDATION_SPEC)
_SCHEMA_VALIDATION_SPEC.loader.exec_module(_SCHEMA_VALIDATION_MODULE)

schema_validator_for: Callable[[Path], object] = (
    _SCHEMA_VALIDATION_MODULE.schema_validator_for
)


class AgentSemanticProjectConfigSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT / "schemas" / "agent-semantic-project-config.v1.schema.json"
        )
        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, config: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(config)]

    def test_empty_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.project-config",
            "schemaVersion": "1",
        }
        self.assertEqual([], self.validation_errors(config))

    def test_discovery_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.project-config",
            "schemaVersion": "1",
            "discovery": {
                "ignoredDirNames": ["vendor", "generated"],
                "includeHiddenDirNames": [".agent-fixtures"],
            },
        }
        self.assertEqual([], self.validation_errors(config))

    def test_provider_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.project-config",
            "schemaVersion": "1",
            "providers": {
                "rust": {"enabled": False},
                "python": {"enabled": True, "binary": ".bin/custom-py-harness"},
            },
        }
        self.assertEqual([], self.validation_errors(config))

    def test_rejects_path_like_ignored_dir_name(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "discovery": {"ignoredDirNames": ["vendor/generated"]},
            }
        )
        self.assertTrue(errors)

    def test_rejects_unknown_provider_language(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "providers": {"ruby": {"enabled": False}},
            }
        )
        self.assertTrue(errors)

    def test_rejects_empty_provider_binary(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "providers": {"python": {"binary": ""}},
            }
        )
        self.assertTrue(errors)

    def test_rejects_non_hidden_include_dir_name(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "discovery": {"includeHiddenDirNames": ["fixtures"]},
            }
        )
        self.assertTrue(errors)

    def test_rejects_parent_dir_include_name(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "discovery": {"includeHiddenDirNames": ["..fixtures"]},
            }
        )
        self.assertTrue(errors)


if __name__ == "__main__":
    unittest.main()
