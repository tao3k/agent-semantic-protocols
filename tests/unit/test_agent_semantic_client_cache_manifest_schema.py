# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Schema tests for agent semantic client cache manifests."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from tests.unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]


def artifact_hash(value: str = "a") -> dict:
    return {
        "algorithm": "blake3",
        "value": value * 64,
    }


def artifact_root(root_kind: str = "sourceIndexBundle") -> dict:
    return {
        "repoId": "repo_123",
        "workspaceId": "workspace_456",
        "scopeId": "default",
        "generation": "g1",
        "rootKind": root_kind,
        "rootHash": artifact_hash("b"),
        "nodeHash": artifact_hash("c"),
        "producerHash": artifact_hash("d"),
        "schemaHash": artifact_hash("e"),
        "contentHash": artifact_hash("f"),
    }


class SemanticAgentClientCacheManifestSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _REPO_ROOT
            / "schemas"
            / "agent-semantic-client-cache-manifest.v1.schema.json"
        )
        self.validator = schema_validator_for(schema_path)

    def validation_errors(self, manifest: dict) -> list[str]:
        return [error.message for error in self.validator.iter_errors(manifest)]

    def test_cache_manifest_is_valid(self) -> None:
        manifest = {
            "schemaId": "agent.semantic-protocols.client-cache-manifest",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "cacheRoot": "/repo/.cache/agent-semantic-protocol/client",
            "generations": [
                {
                    "generationId": "rust-main-1",
                    "languageId": "rust",
                    "providerId": "asp-rust",
                    "providerVersion": "0.1.0",
                    "exportMethod": "search/playbook",
                    "projectRoot": "/repo",
                    "packageRoot": ".",
                    "schemaIds": [
                        "agent.semantic-protocols.workspace-search-playbook-result"
                    ],
                    "cacheStatus": "miss",
                    "rawSourceStored": False,
                    "requestFingerprint": "fnv64:0123456789abcdef",
                    "fileHashes": [
                        {
                            "path": "src/lib.rs",
                            "sha256": "a" * 64,
                        }
                    ],
                    "artifactIds": ["search-playbook/rust-main-1.json"],
                    "artifactRoots": [
                        artifact_root("sourceSnapshot"),
                        artifact_root("sourceIndexBundle"),
                    ],
                }
            ],
        }

        self.assertEqual(self.validation_errors(manifest), [])

    def test_rejects_raw_source_storage(self) -> None:
        manifest = {
            "schemaId": "agent.semantic-protocols.client-cache-manifest",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "cacheRoot": "/repo/.cache/agent-semantic-protocol/client",
            "generations": [
                {
                    "generationId": "rust-main-1",
                    "languageId": "rust",
                    "providerId": "asp-rust",
                    "projectRoot": "/repo",
                    "schemaIds": [
                        "agent.semantic-protocols.workspace-search-playbook-result"
                    ],
                    "cacheStatus": "miss",
                    "rawSourceStored": True,
                }
            ],
        }

        errors = self.validation_errors(manifest)

        self.assertTrue(any("False was expected" in error for error in errors))

    def test_rejects_empty_request_fingerprint(self) -> None:
        manifest = {
            "schemaId": "agent.semantic-protocols.client-cache-manifest",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.client",
            "protocolVersion": "1",
            "cacheRoot": "/repo/.cache/agent-semantic-protocol/client",
            "generations": [
                {
                    "generationId": "rust-main-1",
                    "languageId": "rust",
                    "providerId": "asp-rust",
                    "projectRoot": "/repo",
                    "schemaIds": [
                        "agent.semantic-protocols.client-prompt-output"
                    ],
                    "cacheStatus": "hit",
                    "rawSourceStored": False,
                    "requestFingerprint": "",
                    "artifactIds": ["prompt-output/rust-main-1.txt"],
                }
            ],
        }

        errors = self.validation_errors(manifest)

        self.assertTrue(any("'' should be non-empty" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
