"""Provider registry contract validator tests."""

from __future__ import annotations

import sys
from pathlib import Path


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages/python/tools/src"))

from tools.provider_registry_contracts import _language_registry_validator  # noqa: E402


def test_language_registry_validator_resolves_query_pack_schema_locally() -> None:
    descriptor = {
        "method": "agent/guide",
        "command": "agent",
        "supportsJson": False,
        "supportsCompact": True,
        "invocation": {"argv": ["rs-harness", "agent", "guide"]},
    }
    registry = {
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
                "schemas": [],
                "queryPackDescriptor": {
                    "descriptorId": "rs-harness.query-pack",
                    "descriptorVersion": "1",
                    "languageId": "rust",
                    "recipes": [
                        {
                            "recipeId": "owner-items",
                            "trigger": {"terms": ["owner"], "match": "any"},
                            "clauses": [{"terms": ["owner"]}],
                        }
                    ],
                },
            }
        ],
    }

    _language_registry_validator(_ROOT).validate(registry)
