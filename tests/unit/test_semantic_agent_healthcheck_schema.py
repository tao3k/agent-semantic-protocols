"""Validate the agent healthcheck schema for runtime repair reports."""

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


class SemanticAgentHealthcheckSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            Path(__file__).resolve().parents[2]
            / "schemas"
            / "semantic-agent-healthcheck.v1.schema.json"
        )
        with open(schema_path, "r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, report: dict[str, Any]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(report)]

    def valid_report(self) -> dict[str, Any]:
        return {
            "schemaId": "agent.semantic-protocols.healthcheck",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.runtime",
            "protocolVersion": "1",
            "status": "degraded",
            "projectRoot": ".",
            "gitToplevel": "/tmp/project",
            "cacheHome": "/tmp/project/.cache",
            "cacheSource": "prj-cache-home",
            "env": {"PRJ_CACHE_HOME": "/tmp/project/.cache"},
            "paths": {
                "agentsDir": {
                    "path": "/tmp/project/.agents",
                    "status": "ok",
                    "providerCount": None,
                    "error": None,
                },
                "agentsSkill": {
                    "path": "/tmp/project/.agents/skills/agent-semantic-protocols/SKILL.org",
                    "status": "ok",
                    "providerCount": None,
                    "error": None,
                },
                "activation": {
                    "path": "/tmp/project/.cache/agent-semantic-protocol/hooks/activation.json",
                    "status": "ok",
                    "providerCount": 1,
                    "error": None,
                },
                "runtimeHome": {
                    "path": "/tmp/project/.cache/agent-semantic-protocol/runtime",
                    "status": "ok",
                    "providerCount": None,
                    "error": None,
                },
                "runtimeProfiles": {
                    "path": "/tmp/project/.cache/agent-semantic-protocol/runtime/profiles.json",
                    "status": "ok",
                    "providerCount": 1,
                    "error": None,
                },
            },
            "binary": {
                "currentAsp": "/tmp/project/target/debug/asp",
                "pathAsp": "/tmp/project/.bin/asp",
                "status": "mismatch",
            },
            "providers": [
                {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "manifestId": "agent.semantic-protocols.providers.rust.rs-harness",
                    "binary": "rs-harness",
                    "resolvedBinary": "/tmp/project/.bin/rs-harness",
                    "argv": ["/tmp/project/.bin/rs-harness"],
                    "healthStatus": "available",
                }
            ],
            "issues": [
                {
                    "severity": "warn",
                    "code": "missing-agent-skill",
                    "message": "git toplevel agent-semantic-protocols skill is missing",
                }
            ],
        }

    def test_healthcheck_accepts_runtime_layout_report(self):
        self.assertEqual([], self.validation_errors(self.valid_report()))

    def test_healthcheck_rejects_unknown_env_fields(self):
        report = self.valid_report()
        report["env"]["PRJ_CACHE_HOME_USED"] = "/tmp/project/.cache"

        self.assertTrue(
            any(
                "Additional properties are not allowed" in error
                for error in self.validation_errors(report)
            )
        )

    def test_healthcheck_rejects_unknown_path_status(self):
        report = self.valid_report()
        report["paths"]["runtimeProfiles"]["status"] = "stale"

        self.assertTrue(
            any("is not one of" in error for error in self.validation_errors(report))
        )


if __name__ == "__main__":
    unittest.main()
