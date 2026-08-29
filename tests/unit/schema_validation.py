"""Shared JSON schema validation helpers for protocol schema unit tests."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


def schema_validator_for(schema_path: Path) -> Draft202012Validator:
    schema = _load_schema(schema_path)
    resources = []
    for loaded_path in sorted(schema_path.parent.glob("*.schema.json")):
        loaded_schema = _load_schema(loaded_path)
        resource = Resource.from_contents(loaded_schema)
        if "$id" in loaded_schema:
            resources.append((loaded_schema["$id"], resource))
        # Resolve relative references against the local bundle deterministically
        # instead of dereferencing the hosted document URL during validation.
        resources.append((loaded_path.as_uri(), resource))
        resources.append(
            (
                f"https://tao3k.github.io/agent-semantic-protocols/schemas/{loaded_path.name}",
                resource,
            )
        )
        resources.append(
            (
                f"https://schemas.agent-semantic-protocols.dev/{loaded_path.name}",
                resource,
            )
        )
        resources.append(
            (
                f"https://schemas.agent-semantic-protocols.dev/schemas/{loaded_path.name}",
                resource,
            )
        )
    registry = Registry().with_resources(resources)
    return Draft202012Validator(schema, registry=registry)


def _load_local_schemas(schema_dir: Path) -> list[dict[str, object]]:
    return [
        _load_schema(local_schema_path)
        for local_schema_path in sorted(schema_dir.glob("*.schema.json"))
    ]


def _load_schema(schema_path: Path) -> dict[str, object]:
    with schema_path.open("r", encoding="utf-8") as handle:
        return json.load(handle)
