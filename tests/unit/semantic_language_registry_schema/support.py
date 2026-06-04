"""Helpers for semantic language registry schema tests."""

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


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


def language_registry_schema_validator() -> Draft202012Validator:
    schema_path = (
        _PROTOCOL_REPO_ROOT / "schemas" / "semantic-language-registry.v1.schema.json"
    )
    with schema_path.open("r", encoding="utf-8") as handle:
        return Draft202012Validator(json.load(handle))


def language_registry_errors(registry: dict[str, Any]) -> list[str]:
    return [
        error.message
        for error in language_registry_schema_validator().iter_errors(registry)
    ]


def language_descriptor_errors(descriptor: dict[str, object]) -> list[str]:
    return language_registry_errors(registry_with_descriptor(descriptor))
