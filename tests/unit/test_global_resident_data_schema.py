# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def load_schema(name: str) -> dict:
    return json.loads((SCHEMAS / name).read_text())


def validator() -> Draft202012Validator:
    workspace_reference = load_schema("workspace-reference.v1.schema.json")
    registry = Registry().with_resource(
        workspace_reference["$id"],
        Resource.from_contents(workspace_reference),
    )
    return Draft202012Validator(
        load_schema("global-resident-data.v1.schema.json"),
        registry=registry,
    )


def request(surface: dict) -> dict:
    return {
        "schemaId": "agent.semantic-protocols.global-resident-data-request.v1",
        "schemaVersion": "1",
        "requestId": "request-1",
        "transportContractDigest": f"blake3-256:{'a' * 64}",
        "ownerEpoch": 1,
        "bindingToken": "binding-1",
        "workspaceIdentity": {
            "projectId": "repo-project1",
            "workspaceId": "workspace-project1",
        },
        "operation": "query",
        "surface": surface,
        "payload": {},
    }


def test_programming_language_and_document_surfaces_are_distinct() -> None:
    schema = validator()
    schema.validate(
        request(
            {
                "kind": "programming-language",
                "languageId": "rust",
                "providerId": "asp-rust",
            }
        )
    )
    schema.validate(
        request(
            {
                "kind": "document",
                "documentFormat": "org",
                "gitScopeDigest": "git-scope-1",
            }
        )
    )


def test_document_surface_cannot_manufacture_language_activation() -> None:
    document = request(
        {
            "kind": "document",
            "documentFormat": "md",
            "gitScopeDigest": "git-scope-1",
            "languageId": "markdown",
        }
    )
    with pytest.raises(ValidationError):
        validator().validate(document)
