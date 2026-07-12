"""Validate the semantic agent hook provider manifest schema contract."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_provider_manifest() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.hook.provider-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.hook",
        "protocolVersion": "1",
        "manifestId": "agent.semantic-protocols.languages.typescript.ts-harness",
        "manifestVersion": "v1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "binary": "ts-harness",
        "source": {
            "defaultExtensions": [".ts", ".tsx"],
            "defaultConfigFiles": ["package.json", "tsconfig.json"],
            "defaultSourceRoots": ["src", "tests"],
            "defaultIgnoredPathPrefixes": ["node_modules", "dist"],
        },
        "policy": {
            "directSourceRead": "block",
            "bulkSourceDump": "block",
            "rawSourceSearch": "block",
            "agentSearchJson": "block",
        },
        "routes": {
            "prime": {
                "argv": ["ts-harness", "search", "prime", "--view", "seeds", "."]
            },
            "owner": {
                "argv": [
                    "ts-harness",
                    "search",
                    "owner",
                    "{path}",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ]
            },
            "lexical": {
                "argv": [
                    "ts-harness",
                    "search",
                    "lexical",
                    "{query}",
                    "owner",
                    "tests",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ]
            },
            "ingest": {
                "argv": [
                    "ts-harness",
                    "search",
                    "ingest",
                    "owner",
                    "tests",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ],
                "stdinMode": "pipe-candidates",
            },
            "checkChanged": {"argv": ["ts-harness", "check", "--changed", "."]},
        },
    }


def minimal_activation() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.hook.activation",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.hook",
        "protocolVersion": "1",
        "projectRoot": ".",
        "generatedBy": {
            "runtime": "asp",
            "version": "0.1.0",
        },
        "providers": [
            {
                "manifestId": "agent.semantic-protocols.languages.typescript.ts-harness",
                "manifestDigest": "sha256:"
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "languageId": "typescript",
                "providerId": "ts-harness",
                "binary": "ts-harness",
                "providerCommandPrefix": ["ts-harness"],
                "coverage": {
                    "packageRoots": ["."],
                    "sourceRoots": ["src", "tests"],
                    "configFiles": ["package.json", "tsconfig.json"],
                    "sourceExtensions": [".ts", ".tsx"],
                    "ignoredPathPrefixes": ["node_modules", "dist"],
                },
            }
        ],
    }


class SemanticAgentHookManifestSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        manifest_schema_path = (
            _REPO_ROOT / "schemas" / "semantic-agent-hook-provider-manifest.v1.schema.json"
        )
        activation_schema_path = (
            _REPO_ROOT / "schemas" / "semantic-agent-hook-activation.v1.schema.json"
        )
        with manifest_schema_path.open("r", encoding="utf-8") as handle:
            self.manifest_validator = Draft202012Validator(json.load(handle))
        with activation_schema_path.open("r", encoding="utf-8") as handle:
            self.activation_validator = Draft202012Validator(json.load(handle))

    def manifest_errors(self, manifest: dict[str, object]) -> list[str]:
        return [error.message for error in self.manifest_validator.iter_errors(manifest)]

    def activation_errors(self, activation: dict[str, object]) -> list[str]:
        return [
            error.message for error in self.activation_validator.iter_errors(activation)
        ]

    def test_minimal_provider_manifest_is_valid(self) -> None:
        self.assertEqual([], self.manifest_errors(minimal_provider_manifest()))

    def test_provider_manifest_accepts_execution_mode(self) -> None:
        manifest = minimal_provider_manifest()
        manifest["execution"] = "embedded"

        self.assertEqual([], self.manifest_errors(manifest))

    def test_provider_manifest_rejects_unknown_execution_mode(self) -> None:
        manifest = minimal_provider_manifest()
        manifest["execution"] = "bin-wrap"

        self.assertTrue(
            any(
                "'bin-wrap' is not one of" in message
                for message in self.manifest_errors(manifest)
            )
        )

    def test_provider_manifest_accepts_export_index_route(self) -> None:
        manifest = minimal_provider_manifest()
        routes = copy.deepcopy(manifest["routes"])
        routes["exportIndex"] = {"argv": ["ts-harness", "export", "index", "."]}
        manifest["routes"] = routes

        self.assertEqual([], self.manifest_errors(manifest))

    def test_provider_manifest_rejects_absolute_source_defaults(self) -> None:
        manifest = minimal_provider_manifest()
        source = copy.deepcopy(manifest["source"])
        source["defaultConfigFiles"] = ["/etc/package.json"]
        manifest["source"] = source

        self.assertTrue(
            any("should not be valid" in message for message in self.manifest_errors(manifest))
        )

    def test_provider_manifest_requires_core_routes(self) -> None:
        manifest = minimal_provider_manifest()
        routes = copy.deepcopy(manifest["routes"])
        del routes["ingest"]
        manifest["routes"] = routes

        self.assertTrue(
            any("'ingest' is a required property" in message for message in self.manifest_errors(manifest))
        )

    def test_provider_manifest_rejects_retired_text_route(self) -> None:
        manifest = minimal_provider_manifest()
        routes = copy.deepcopy(manifest["routes"])
        routes["text"] = routes["lexical"]
        del routes["lexical"]
        manifest["routes"] = routes

        errors = self.manifest_errors(manifest)
        self.assertTrue(any("'lexical' is a required property" in message for message in errors))
        self.assertTrue(
            any("Additional properties are not allowed" in message for message in errors)
        )

    def test_provider_manifest_routes_require_argv_not_text(self) -> None:
        manifest = minimal_provider_manifest()
        routes = copy.deepcopy(manifest["routes"])
        routes["prime"] = {"text": "ts-harness search prime --workspace . --view seeds"}
        manifest["routes"] = routes

        errors = self.manifest_errors(manifest)
        self.assertTrue(any("'argv' is a required property" in message for message in errors))
        self.assertTrue(
            any("Additional properties are not allowed" in message for message in errors)
        )

    def test_provider_manifest_action_policy_rejects_unknown_modes(self) -> None:
        manifest = minimal_provider_manifest()
        policy = copy.deepcopy(manifest["policy"])
        policy["rawSourceSearch"] = "warn"
        manifest["policy"] = policy

        self.assertTrue(
            any("'warn' is not one of" in message for message in self.manifest_errors(manifest))
        )

    def test_project_activation_is_valid(self) -> None:
        self.assertEqual([], self.activation_errors(minimal_activation()))

    def test_project_activation_accepts_execution_mode(self) -> None:
        activation = minimal_activation()
        provider = copy.deepcopy(activation["providers"][0])
        provider["execution"] = "external-process"
        activation["providers"] = [provider]

        self.assertEqual([], self.activation_errors(activation))

    def test_project_activation_rejects_absolute_coverage_paths(self) -> None:
        activation = minimal_activation()
        provider = copy.deepcopy(activation["providers"][0])
        coverage = copy.deepcopy(provider["coverage"])
        coverage["sourceRoots"] = ["/tmp/src"]
        provider["coverage"] = coverage
        activation["providers"] = [provider]

        self.assertTrue(
            any(
                "should not be valid" in message
                for message in self.activation_errors(activation)
            )
        )

    def test_project_activation_requires_digest_for_installed_manifest(self) -> None:
        activation = minimal_activation()
        provider = copy.deepcopy(activation["providers"][0])
        del provider["manifestDigest"]
        activation["providers"] = [provider]

        self.assertTrue(
            any(
                "'manifestDigest' is a required property" in message
                for message in self.activation_errors(activation)
            )
        )


if __name__ == "__main__":
    unittest.main()
