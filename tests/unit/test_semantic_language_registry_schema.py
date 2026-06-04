"""Schema contract tests for semantic-language registry descriptors."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def registry_with_descriptor(
    descriptor: dict[str, object],
    *,
    schemas: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    return {
        "registryId": "agent.semantic-protocols.semantic-language-registry",
        "registryVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languages": [
            {
                "languageId": "rust",
                "providerId": "rs-harness",
                "binary": "rs-harness",
                "namespace": "agent.semantic-protocols.rust",
                "methods": [descriptor["method"]],
                "methodDescriptors": [descriptor],
                "schemas": [] if schemas is None else schemas,
            }
        ],
    }


class SemanticLanguageRegistrySchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT
            / "schemas"
            / "semantic-language-registry.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, descriptor: dict[str, object]) -> list[str]:
        errors = self.validator.iter_errors(registry_with_descriptor(descriptor))
        return [error.message for error in errors]

    def registry_validation_errors(
        self,
        registry: dict[str, object],
    ) -> list[str]:
        return [error.message for error in self.validator.iter_errors(registry)]

    def test_agent_compact_method_can_omit_output_schema_when_no_json(self) -> None:
        errors = self.validation_errors(
            {
                "method": "agent/guide",
                "command": "agent",
                "supportsJson": False,
                "supportsCompact": True,
                "clients": ["codex"],
                "requiredOptions": ["--client codex"],
            }
        )

        self.assertEqual([], errors)

    def test_language_registration_accepts_provider_command_prefix(self) -> None:
        registry = registry_with_descriptor(
            {
                "method": "agent/guide",
                "command": "agent",
                "supportsJson": False,
                "supportsCompact": True,
            }
        )
        language = registry["languages"][0]
        language["languageId"] = "julia"
        language["providerId"] = "julia-project-harness"
        language["binary"] = "julia-project-harness"
        language["providerCommandPrefix"] = [
            "julia",
            "--project=languages/JuliaLangProjectHarness.jl",
            "languages/JuliaLangProjectHarness.jl/bin/julia-project-harness.jl",
        ]
        language["namespace"] = (
            "agent.semantic-protocols.languages.julia.julia-project-harness"
        )
        self.assertEqual([], self.registry_validation_errors(registry))

    def test_agent_json_method_requires_output_schema_ids(self) -> None:
        errors = self.validation_errors(
            {
                "method": "agent/hook",
                "command": "agent",
                "supportsJson": True,
                "supportsCompact": False,
                "clients": ["codex"],
            }
        )

        self.assertIn("'outputSchemaIds' is a required property", errors)

    def test_evidence_review_and_proof_methods_validate(self) -> None:
        for method, command, schema_id in [
            (
                "proof/pilot",
                "proof",
                "agent.semantic-protocols.semantic-formal-proof-pilot",
            ),
            (
                "review/packet",
                "review",
                "agent.semantic-protocols.semantic-review-packet",
            ),
            (
                "evidence/assurance",
                "evidence",
                "agent.semantic-protocols.semantic-assurance-case",
            ),
        ]:
            with self.subTest(method=method):
                registry = registry_with_descriptor(
                    {
                        "method": method,
                        "command": command,
                        "input": method.split("/", maxsplit=1)[1],
                        "outputSchemaIds": [schema_id],
                        "supportsJson": True,
                        "supportsCompact": True,
                    },
                    schemas=[
                        {
                            "schemaId": schema_id,
                            "schemaVersion": "1",
                            "path": f"schemas/{schema_id.rsplit('.', maxsplit=1)[1]}.v1.schema.json",
                        }
                    ],
                )
                self.assertEqual([], self.registry_validation_errors(registry))

    def test_evidence_json_method_requires_output_schema_ids(self) -> None:
        errors = self.validation_errors(
            {
                "method": "evidence/assurance",
                "command": "evidence",
                "input": "assurance",
                "supportsJson": True,
                "supportsCompact": True,
            }
        )

        self.assertIn("'outputSchemaIds' is a required property", errors)

    def test_ast_patch_dry_run_method_declares_non_mutating_receipt(self) -> None:
        registry = registry_with_descriptor(
            {
                "method": "ast-patch/dry-run",
                "command": "ast-patch",
                "input": "semantic-ast-patch packet",
                "requiredOptions": ["--packet"],
                "outputSchemaIds": [
                    "agent.semantic-protocols.semantic-ast-patch-receipt"
                ],
                "supportsJson": True,
                "supportsCompact": False,
                "mutationAvailable": False,
            },
            schemas=[
                {
                    "schemaId": "agent.semantic-protocols.semantic-ast-patch-receipt",
                    "schemaVersion": "1",
                    "path": "schemas/semantic-ast-patch-receipt.v1.schema.json",
                },
            ],
        )
        self.assertEqual([], self.registry_validation_errors(registry))

    def test_query_method_can_declare_query_packet_schema(self) -> None:
        registry = registry_with_descriptor(
            {
                "method": "query/owner-items",
                "command": "query",
                "input": "owner-path",
                "requiredOptions": ["--term"],
                "outputSchemaIds": [
                    "agent.semantic-protocols.semantic-query-packet"
                ],
                "supportsJson": True,
                "supportsCompact": True,
                "supportsQuerySet": True,
                "acceptedQuerySetSelectors": ["exact-set"],
                "querySetScopes": ["owner"],
                "outputModes": ["compact", "json", "code", "names"],
            },
            schemas=[
                {
                    "schemaId": "agent.semantic-protocols.semantic-query-packet",
                    "schemaVersion": "1",
                    "path": "schemas/semantic-query-packet.v1.schema.json",
                },
            ],
        )
        self.assertEqual([], self.registry_validation_errors(registry))

    def test_query_method_can_declare_provider_read_packet_schema(self) -> None:
        registry = registry_with_descriptor(
            {
                "method": "query/direct-source-read",
                "command": "query",
                "input": "owner-path",
                "requiredOptions": ["--from-hook", "--selector"],
                "outputSchemaIds": [
                    "agent.semantic-protocols.semantic-query-packet",
                    "agent.semantic-protocols.semantic-read-packet",
                ],
                "supportsJson": True,
                "supportsCompact": True,
                "outputModes": ["compact", "json", "names", "read-packet"],
            },
            schemas=[
                {
                    "schemaId": "agent.semantic-protocols.semantic-query-packet",
                    "schemaVersion": "1",
                    "path": "schemas/semantic-query-packet.v1.schema.json",
                },
                {
                    "schemaId": "agent.semantic-protocols.semantic-read-packet",
                    "schemaVersion": "1",
                    "path": "schemas/semantic-read-packet.v1.schema.json",
                },
            ],
        )
        self.assertEqual([], self.registry_validation_errors(registry))

    def test_search_method_can_declare_secondary_type_surface_schema(self) -> None:
        registry = registry_with_descriptor(
            {
                "method": "search/public-external-types",
                "command": "search",
                "view": "public-external-types",
                "outputSchemaIds": [
                    "agent.semantic-protocols.semantic-search-packet",
                    "agent.semantic-protocols.semantic-type-surface",
                ],
                "requiresQuery": True,
                "acceptsStdin": False,
                "supportsPackageScope": True,
                "supportsJson": True,
                "supportsCompact": True,
            },
            schemas=[
                {
                    "schemaId": "agent.semantic-protocols.semantic-search-packet",
                    "schemaVersion": "1",
                    "path": "schemas/semantic-search-packet.v1.schema.json",
                },
                {
                    "schemaId": "agent.semantic-protocols.semantic-type-surface",
                    "schemaVersion": "1",
                    "path": "schemas/semantic-type-surface.v1.schema.json",
                },
            ],
        )

        self.assertEqual([], self.registry_validation_errors(registry))
        language = registry["languages"][0]
        registered_schema_ids = {
            schema["schemaId"] for schema in language["schemas"]
        }
        declared_schema_ids = set(
            language["methodDescriptors"][0]["outputSchemaIds"]
        )

        self.assertLessEqual(declared_schema_ids, registered_schema_ids)


if __name__ == "__main__":
    unittest.main()
