# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared JSON schema validation helpers for protocol schema unit tests."""

from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path
from urllib.parse import unquote, urlparse

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


@lru_cache(maxsize=None)
def schema_validator_for(schema_path: Path) -> Draft202012Validator:
    schema = _load_schema(schema_path)
    return Draft202012Validator(schema, registry=_schema_registry(schema_path))


@lru_cache(maxsize=None)
def _schema_registry(schema_path: Path) -> Registry:
    resources = []
    for loaded_path in _reachable_schema_paths(schema_path):
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
        resources.append(
            (
                f"https://agent-semantic-protocols.dev/schemas/{loaded_path.name}",
                resource,
            )
        )
        # Some older schema families intentionally use the repository-local
        # authority.  Unit validation is offline and must resolve that
        # authority from the same checked-out bundle, rather than attempting a
        # network fetch (or treating a valid local reference as missing).
        resources.append(
            (
                f"https://agent-semantic-protocols.local/{loaded_path.name}",
                resource,
            )
        )
        resources.append(
            (
                f"https://agent-semantic-protocols.local/schemas/{loaded_path.name}",
                resource,
            )
        )
        resources.append(
            (
                f"https://schemas.agent-semantic-protocols.local/{loaded_path.name}",
                resource,
            )
        )
        resources.append(
            (
                f"https://schemas.agent-semantic-protocols.local/schemas/{loaded_path.name}",
                resource,
            )
        )
    registry = Registry().with_resources(resources)
    return registry


def _reachable_schema_paths(schema_path: Path) -> tuple[Path, ...]:
    """Return only the local schema dependency closure for ``schema_path``."""

    schema_dir = schema_path.parent
    pending = [schema_path]
    visited: set[Path] = set()
    while pending:
        loaded_path = pending.pop()
        if loaded_path in visited:
            continue
        visited.add(loaded_path)
        for reference in _external_references(_load_schema(loaded_path)):
            parsed = urlparse(reference)
            if not parsed.path:
                continue
            reference_name = Path(unquote(parsed.path)).name
            candidate = schema_dir / reference_name
            if not candidate.is_file() and not reference_name.endswith(".schema.json"):
                candidate = schema_dir / f"{reference_name}.schema.json"
            if candidate.is_file() and candidate not in visited:
                pending.append(candidate)
    return tuple(sorted(visited))


def _external_references(value: object) -> set[str]:
    if isinstance(value, dict):
        references = {
            reference
            for key in ("$ref", "$dynamicRef")
            if isinstance((reference := value.get(key)), str)
            and not reference.startswith("#")
        }
        for child in value.values():
            references.update(_external_references(child))
        return references
    if isinstance(value, list):
        references: set[str] = set()
        for child in value:
            references.update(_external_references(child))
        return references
    return set()


def _load_local_schemas(schema_dir: Path) -> list[dict[str, object]]:
    return [
        _load_schema(local_schema_path)
        for local_schema_path in sorted(schema_dir.glob("*.schema.json"))
    ]


@lru_cache(maxsize=None)
def _load_schema(schema_path: Path) -> dict[str, object]:
    with schema_path.open("r", encoding="utf-8") as handle:
        return json.load(handle)
