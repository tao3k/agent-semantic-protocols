"""Offline registry for the repository's shared JSON-schema authority."""

from __future__ import annotations

import json
from pathlib import Path

from referencing import Registry, Resource


def offline_schema_registry(schema_path: Path) -> Registry:
    """Resolve every checked-out schema authority without network access."""

    resources: list[tuple[str, Resource]] = []
    for local_path in sorted(schema_path.parent.glob("*.schema.json")):
        with local_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
        resource = Resource.from_contents(schema)
        if isinstance(schema.get("$id"), str):
            resources.append((schema["$id"], resource))
        resources.extend(
            (
                alias,
                resource,
            )
            for alias in (
                local_path.as_uri(),
                f"https://agent-semantic-protocols.local/{local_path.name}",
                f"https://agent-semantic-protocols.local/schemas/{local_path.name}",
                f"https://schemas.agent-semantic-protocols.local/{local_path.name}",
                f"https://schemas.agent-semantic-protocols.local/schemas/{local_path.name}",
                f"https://schemas.agent-semantic-protocols.dev/{local_path.name}",
                f"https://schemas.agent-semantic-protocols.dev/schemas/{local_path.name}",
                f"https://tao3k.github.io/agent-semantic-protocols/schemas/{local_path.name}",
            )
        )
    return Registry().with_resources(resources)
