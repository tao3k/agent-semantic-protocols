"""Schema validator helpers for native syntax fact index tests."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

_REPO_ROOT = Path(__file__).resolve().parents[3]


@dataclass(frozen=True)
class SchemaValidators:
    index: Draft202012Validator
    fact: Draft202012Validator
    search: Draft202012Validator


def schema_validators() -> SchemaValidators:
    schema_dir = _REPO_ROOT / "schemas"
    native_schema = _load_schema(
        schema_dir / "semantic-native-syntax-fact-index.v1.schema.json"
    )
    search_schema = _load_schema(schema_dir / "semantic-search-packet.v1.schema.json")
    source_location_schema = _load_schema(
        schema_dir / "semantic-source-location.v1.schema.json"
    )
    tree_sitter_provenance_schema = _load_schema(
        schema_dir / "semantic-tree-sitter-provenance.v1.schema.json"
    )
    registry = Registry().with_resources(
        [
            (native_schema["$id"], Resource.from_contents(native_schema)),
            (search_schema["$id"], Resource.from_contents(search_schema)),
            (
                source_location_schema["$id"],
                Resource.from_contents(source_location_schema),
            ),
            (
                tree_sitter_provenance_schema["$id"],
                Resource.from_contents(tree_sitter_provenance_schema),
            ),
        ]
    )
    return SchemaValidators(
        index=Draft202012Validator(native_schema, registry=registry),
        fact=Draft202012Validator(
            {"$ref": f"{native_schema['$id']}#/$defs/nativeSyntaxFact"},
            registry=registry,
        ),
        search=Draft202012Validator(search_schema, registry=registry),
    )


def validation_errors(
    validator: Draft202012Validator, payload: dict[str, object]
) -> list[str]:
    return [error.message for error in validator.iter_errors(payload)]


def _load_schema(schema_path: Path) -> dict[str, object]:
    with schema_path.open("r", encoding="utf-8") as handle:
        return json.load(handle)
