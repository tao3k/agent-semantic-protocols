from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


class SemanticAgentClientReceiptSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "agent-semantic-client-receipt.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, receipt: dict) -> list[str]:
        return [error.message for error in self.validator.iter_errors(receipt)]

    def test_local_native_receipt_is_valid(self) -> None:
        receipt = {
            "schemaId": "agent.semantic-protocols.client-receipt",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "method": "search",
            "route": "local-native",
            "cacheStatus": "miss",
            "providerCommandCount": 1,
            "providerProcessesSpawned": 1,
            "providerCommands": [
                {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "argv": ["direnv", "exec", ".", "rs-harness", "search", "prime", "."],
                    "exitCode": 0,
                    "stdoutBytes": 300,
                    "stderrBytes": 0,
                    "elapsedMs": 12,
                }
            ],
            "nativeProvenance": [
                {
                    "languageId": "rust",
                    "providerId": "rs-harness",
                    "providerBinary": "rs-harness",
                    "schemaIds": [
                        "agent.semantic-protocols.semantic-search-packet"
                    ],
                }
            ],
            "compactArtifactId": None,
            "elapsedMs": 12,
            "stdoutBytes": 300,
            "stderrBytes": 0,
        }

        self.assertEqual(self.validation_errors(receipt), [])

    def test_cache_disabled_receipt_is_valid(self) -> None:
        receipt = {
            "schemaId": "agent.semantic-protocols.client-receipt",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "method": "cache-status",
            "route": "local-cache",
            "cacheStatus": "disabled",
            "providerCommandCount": 0,
            "providerProcessesSpawned": 0,
            "providerCommands": [],
            "nativeProvenance": [],
            "cacheRoot": "/repo/.cache/agent-semantic-protocol/client",
            "cacheManifestPath": "/repo/.cache/agent-semantic-protocol/client/cache-manifest.json",
            "cacheManifestStatus": "missing",
            "cacheGenerationCount": 0,
            "rawSourceStored": False,
            "clientDbPath": "/repo/.cache/agent-semantic-protocol/client/client.sqlite3",
            "clientDbStatus": "missing",
            "clientDbGenerationCount": 0,
            "clientDbRawSourceStored": False,
        }

        self.assertEqual(self.validation_errors(receipt), [])

    def test_cache_import_receipt_is_valid(self) -> None:
        receipt = {
            "schemaId": "agent.semantic-protocols.client-receipt",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "method": "cache-import",
            "route": "local-cache",
            "cacheStatus": "disabled",
            "providerCommandCount": 0,
            "providerProcessesSpawned": 0,
            "providerCommands": [],
            "nativeProvenance": [],
            "cacheRoot": "/repo/.cache/agent-semantic-protocol/client",
            "cacheManifestPath": "/repo/.cache/agent-semantic-protocol/client/cache-manifest.json",
            "cacheManifestStatus": "present",
            "cacheGenerationCount": 1,
            "rawSourceStored": False,
            "clientDbPath": "/repo/.cache/agent-semantic-protocol/client/client.sqlite3",
            "clientDbStatus": "present",
            "clientDbGenerationCount": 1,
            "clientDbRawSourceStored": False,
        }

        self.assertEqual(self.validation_errors(receipt), [])

    def test_rejects_unknown_route(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.client-receipt",
                "schemaVersion": "1",
                "protocolId": "agent.semantic-protocols.client",
                "protocolVersion": "1",
                "method": "search",
                "route": "text-fallback",
                "cacheStatus": "miss",
                "providerCommandCount": 1,
                "providerProcessesSpawned": 1,
                "providerCommands": [],
                "nativeProvenance": [],
            }
        )

        self.assertTrue(any("is not one of" in error for error in errors))

    def test_rejects_missing_native_provenance(self) -> None:
        errors = self.validation_errors(
            {
                "schemaId": "agent.semantic-protocols.client-receipt",
                "schemaVersion": "1",
                "protocolId": "agent.semantic-protocols.client",
                "protocolVersion": "1",
                "method": "search",
                "route": "local-native",
                "cacheStatus": "miss",
                "providerCommandCount": 1,
                "providerProcessesSpawned": 1,
                "providerCommands": [],
            }
        )

        self.assertTrue(any("'nativeProvenance' is a required property" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
