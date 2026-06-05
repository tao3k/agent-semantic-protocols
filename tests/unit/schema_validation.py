"""Shared JSON schema validation helpers for protocol schema unit tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


def schema_validator_for(schema_path: Path) -> Draft202012Validator:
    schema = _load_schema(schema_path)
    registry = Registry().with_resources(
        (loaded_schema["$id"], Resource.from_contents(loaded_schema))
        for loaded_schema in _load_local_schemas(schema_path.parent)
    )
    return Draft202012Validator(schema, registry=registry)


def _load_local_schemas(schema_dir: Path) -> list[dict[str, object]]:
    return [
        _load_schema(local_schema_path)
        for local_schema_path in sorted(schema_dir.glob("*.schema.json"))
    ]


def _load_schema(schema_path: Path) -> dict[str, object]:
    with schema_path.open("r", encoding="utf-8") as handle:
        return json.load(handle)
