"""Schema contract tests for semantic type surfaces."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_type_surface() -> dict[str, object]:
    return {
        "id": "RS:src/lib.rs:PublicWire:field:serializer",
        "name": "serializer",
        "languageName": "serializer",
        "qualifiedName": "serde::Serialize",
        "kind": "object",
        "role": "api-field",
        "ownerPath": "src/lib.rs",
        "location": {"path": "src/lib.rs", "lineRange": "12:12"},
        "visibility": "public",
        "external": True,
        "source": "native-parser",
        "package": "serde",
        "module": "serde",
        "symbol": "Serialize",
        "versionScope": "external",
        "carrier": {
            "name": "serde::Serialize",
            "languageName": "Serialize",
            "qualifiedName": "serde::Serialize",
            "carrier": "external",
            "package": "serde",
            "module": "serde",
            "symbol": "Serialize",
            "versionScope": "external",
            "external": True,
        },
        "fields": {
            "dependency": "serde",
            "surface": "field:serializer",
            "confidence": "direct",
        },
    }


class SemanticTypeSurfaceSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = (
            _PROTOCOL_REPO_ROOT / "schemas" / "semantic-type-surface.v1.schema.json"
        )
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
        registry = Registry().with_resources(
            [(schema["$id"], Resource.from_contents(schema))]
        )
        self.validator = Draft202012Validator(
            {"$ref": f"{schema['$id']}#/$defs/typeSurface"},
            registry=registry,
        )

    def validation_errors(self, payload: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(payload)]

    def test_type_surface_accepts_external_api_carrier(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_type_surface()))

    def test_type_surface_rejects_rank_prefixed_owner_path(self) -> None:
        payload = copy.deepcopy(minimal_type_surface())
        payload["ownerPath"] = "1:src/lib.rs"

        self.assertIn(
            "'1:src/lib.rs' does not match",
            "\n".join(self.validation_errors(payload)),
        )


if __name__ == "__main__":
    unittest.main()
