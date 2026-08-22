from __future__ import annotations

import json
from pathlib import Path

from tests.unit.schema_validator_support import local_schema_validator


_ROOT = Path(__file__).resolve().parents[2]


def test_registry_schema_accepts_owner_item_query_fallback_descriptor() -> None:
    validator = local_schema_validator(
        _ROOT / "schemas" / "semantic-language-registry.v1.schema.json",
        _ROOT / "schemas" / "provider-query-pack-descriptor.schema.json",
    )
    provider = json.loads(
        (
            _ROOT
            / "languages"
            / "typescript-lang-project-harness"
            / "schemas"
            / "asp-provider.json"
        ).read_text()
    )
    registry = {
        "registryId": "agent.semantic-protocols.semantic-language-registry",
        "registryVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languages": [
            {
                "languageId": "typescript",
                "providerId": "asp-typescript",
                "binary": "asp-typescript",
                "execution": "external-process",
                "namespace": "agent.semantic-protocols.languages.typescript.asp-typescript",
                "methods": ["search/owner"],
                "methodDescriptors": [
                    {
                        "method": "search/owner",
                        "command": "search",
                        "invocation": {
                            "argv": ["asp-typescript", "search", "owner"]
                        },
                        "view": "owner",
                        "outputSchemaIds": [
                            "agent.semantic-protocols.semantic-search-packet"
                        ],
                        "requiresQuery": True,
                        "acceptsStdin": False,
                        "supportsPackageScope": True,
                        "acceptedPipes": ["items"],
                        "capabilities": [
                            {
                                "languageId": "typescript",
                                "namespace": "typescript",
                                "name": "owner-item-query",
                            }
                        ],
                        "fallbacks": [
                            {
                                "name": "owner-top-items",
                                "trigger": "item-query-miss",
                                "appliesToPipes": ["items"],
                                "maxItems": 4,
                            }
                        ],
                        "supportsJson": True,
                        "supportsCompact": True,
                    }
                ],
                "schemas": [
                    {
                        "schemaId": "agent.semantic-protocols.semantic-language-registry",
                        "schemaVersion": "1",
                        "path": "schemas/semantic-language-registry.v1.schema.json",
                    }
                ],
                "queryPackDescriptor": provider["queryPackDescriptor"],
            }
        ],
    }
    validator.validate(registry)
