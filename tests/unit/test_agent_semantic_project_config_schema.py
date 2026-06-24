"""Validate the shared asp.toml project configuration schema."""

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
                "org": {"enabled": False},
                "md": {"enabled": True},
            },
        }
        self.assertEqual([], self.validation_errors(config))

    def test_hook_agent_org_artifacts_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.project-config",
            "schemaVersion": "1",
            "hook": {
                "agentOrgArtifacts": {
                    "enabled": True,
                    "inactiveAfterMinutes": 30,
                    "artifactsPath": ".cache/agent-semantic-protocol/artifacts/org",
                    "entrySkillPath": ".cache/agent-semantic-protocol/org/templates/ASP_ORG_SKILL.org",
                    "archiveWarning": {
                        "enabled": True,
                        "activeOrgFileThreshold": 10,
                        "archivesDir": "archives",
                        "maxReportedFiles": 5,
                    },
                }
            },
        }
        self.assertEqual([], self.validation_errors(config))

    def test_skills_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.project-config",
            "schemaVersion": "1",
            "skills": {
                "agent-semantic-protocols": {
                    "template": "SKILL.org",
                    "pluginSkill": "asp-codex-plugin/skills/agent-semantic-protocols/SKILL.org",
                    "projectSkill": ".agents/skills/agent-semantic-protocols/SKILL.org",
                    "aspOrg": ".cache/agent-semantic-protocol/org/templates/ASP_ORG_SKILL.org#asp-org",
                    "orgArtifacts": ".cache/agent-semantic-protocol/artifacts/org",
                }
            },
        }
        self.assertEqual([], self.validation_errors(config))

    def test_codeql_extension_config_is_valid(self) -> None:
        config = {
            "schemaId": "agent.semantic-protocols.project-config",
            "schemaVersion": "1",
            "extensions": {
                "codeql": {
                    "enabled": False,
                    "experimental": True,
                    "mode": "cache-only",
                    "allowDatabaseCreate": False,
                    "allowHotPath": False,
                    "cacheDir": ".cache/agent-semantic-protocol/codeql",
                    "profiles": ["metadata", "local-flow"],
                }
            },
        }
        self.assertEqual([], self.validation_errors(config))

    def test_codeql_extension_defaults_are_experimental_and_disabled(self) -> None:
        codeql = self.validator.schema["$defs"]["codeqlExtensionConfig"]["properties"]

        self.assertEqual(False, codeql["enabled"]["default"])
        self.assertEqual(True, codeql["experimental"]["default"])
        self.assertEqual("disabled", codeql["mode"]["default"])
        self.assertEqual(False, codeql["allowDatabaseCreate"]["default"])
        self.assertEqual(False, codeql["allowHotPath"]["default"])

    def test_rejects_path_like_ignored_dir_name(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "discovery": {"ignoredDirNames": ["vendor/generated"]},
            }
        )
        self.assertTrue(errors)

    def test_rejects_invalid_provider_language_id(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "providers": {"Ruby": {"enabled": False}},
            }
        )
        self.assertTrue(errors)

    def test_rejects_codeql_hot_path_enablement(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "extensions": {"codeql": {"allowHotPath": True}},
            }
        )
        self.assertTrue(errors)

    def test_rejects_zero_agent_org_artifact_minutes(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "hook": {"agentOrgArtifacts": {"inactiveAfterMinutes": 0}},
            }
        )
        self.assertTrue(errors)

    def test_rejects_invalid_agent_org_archive_warning(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "hook": {
                    "agentOrgArtifacts": {
                        "archiveWarning": {
                            "activeOrgFileThreshold": 0,
                            "archivesDir": "",
                            "maxReportedFiles": 0,
                        }
                    }
                },
            }
        )
        self.assertTrue(errors)

    def test_rejects_unknown_extension(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.project-config",
                "schemaVersion": "1",
                "extensions": {"semgrep": {"enabled": True}},
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
