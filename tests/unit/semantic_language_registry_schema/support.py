# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Helpers for semantic language registry schema tests."""

from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from tests.unit.schema_validator_support import local_schema_validator


_PROTOCOL_REPO_ROOT = Path(__file__).resolve().parents[3]


def registry_with_descriptor(
    descriptor: dict[str, object],
    *,
    schemas: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    descriptor = dict(descriptor)
    descriptor.setdefault(
        "invocation",
        {"argv": ["asp-rust", str(descriptor["command"])]},
    )
    return {
        "registryId": "agent.semantic-protocols.semantic-language-registry",
        "registryVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languages": [
            {
                "languageId": "rust",
                "providerId": "asp-rust",
                "binary": "asp-rust",
                "namespace": "agent.semantic-protocols.rust",
                "methods": [descriptor["method"]],
                "methodDescriptors": [descriptor],
                "schemas": [] if schemas is None else schemas,
                "queryPackDescriptor": {
                    "descriptorId": "asp-rust.query-pack",
                    "descriptorVersion": "1",
                    "languageId": "rust",
                    "recipes": [],
                },
            }
        ],
    }


def language_registry_schema_validator() -> Draft202012Validator:
    schema_path = (
        _PROTOCOL_REPO_ROOT / "schemas" / "semantic-language-registry.v1.schema.json"
    )
    return local_schema_validator(
        schema_path,
        _PROTOCOL_REPO_ROOT
        / "schemas"
        / "provider-query-pack-descriptor.schema.json",
    )


def language_registry_errors(registry: dict[str, Any]) -> list[str]:
    return [
        error.message
        for error in language_registry_schema_validator().iter_errors(registry)
    ]


def language_descriptor_errors(descriptor: dict[str, object]) -> list[str]:
    return language_registry_errors(registry_with_descriptor(descriptor))
