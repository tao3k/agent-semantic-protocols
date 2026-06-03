"""Schema contract tests for type surfaces in search packets."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def packet_with_type_surface() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "typescript",
        "providerId": "ts-harness",
        "binary": "ts-harness",
        "namespace": "agent.semantic-protocols.languages.typescript.ts-harness",
        "method": "search/public-external-types",
        "projectRoot": ".",
        "view": "public-external-types",
        "renderMode": "hits",
        "header": {"kind": "search-public-external-types", "fields": {}},
        "nodes": [],
        "edges": [],
        "owners": [],
        "hits": [],
        "findings": [],
        "nextActions": [],
        "notes": [],
        "typeSurfaces": [
            {
                "id": "TS:src/api.ts:ReactNode:param:children",
                "name": "children",
                "languageName": "children",
                "qualifiedName": "import(\"react\").ReactNode",
                "kind": "alias",
                "role": "api-input",
                "ownerPath": "src/api.ts",
                "location": {"path": "src/api.ts", "lineRange": "12:12"},
                "visibility": "public",
                "external": True,
                "source": "native-parser",
                "package": "react",
                "module": "react",
                "symbol": "ReactNode",
                "versionScope": "external",
                "carrier": {
                    "name": "import(\"react\").ReactNode",
                    "languageName": "ReactNode",
                    "qualifiedName": "import(\"react\").ReactNode",
                    "carrier": "external",
                    "package": "react",
                    "module": "react",
                    "symbol": "ReactNode",
                    "versionScope": "external",
                    "external": True,
                },
                "fields": {"dependency": "react", "confidence": "direct"},
            }
        ],
    }


class SemanticSearchPacketTypeSurfacesSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_dir = _PROTOCOL_REPO_ROOT / "schemas"
        with (schema_dir / "semantic-search-packet.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            packet_schema = json.load(handle)
        with (schema_dir / "semantic-type-surface.v1.schema.json").open(
            "r", encoding="utf-8"
        ) as handle:
            type_surface_schema = json.load(handle)
        registry = Registry().with_resources(
            [
                (packet_schema["$id"], Resource.from_contents(packet_schema)),
                (type_surface_schema["$id"], Resource.from_contents(type_surface_schema)),
            ]
        )
        self.validator = Draft202012Validator(packet_schema, registry=registry)

    def validation_errors(self, payload: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(payload)]

    def test_search_packet_accepts_shared_type_surfaces(self) -> None:
        self.assertEqual([], self.validation_errors(packet_with_type_surface()))


if __name__ == "__main__":
    unittest.main()
