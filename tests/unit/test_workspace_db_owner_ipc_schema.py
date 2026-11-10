# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "workspace-db-owner-ipc.v1.schema.json"
SCHEMA = json.loads(SCHEMA_PATH.read_text())
from unit.schema_validation import schema_validator_for

VALIDATOR = schema_validator_for(SCHEMA_PATH)


def response(result: dict[str, object]) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.workspace-db-owner-response.v1",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-test",
        "transportContractDigest": "blake3-256:test-contract",
        "ownerEpoch": 9,
        "requestId": "request-test",
        "result": result,
    }


def request(operation: dict[str, object]) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.workspace-db-owner-request.v1",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-test",
        "transportContractDigest": "blake3-256:test-contract",
        "ownerEpoch": 9,
        "bindingToken": "binding-test",
        "requestId": "request-test",
        "operation": operation,
    }


def test_workspace_db_owner_request_accepts_typed_source_index_lookup() -> None:
    Draft202012Validator.check_schema(SCHEMA)
    VALIDATOR.validate(
        request(
            {
                "kind": "read-source-index",
                "request": {
                    "projectRoot": "/tmp/workspace",
                    "indexedProjectRoot": "/tmp/workspace",
                    "sourceSnapshot": {
                        "schemaId": "asp.source-snapshot.v1",
                        "algorithm": "blake3-256",
                        "rootDigest": "root-test",
                        "sourceKind": "filesystem",
                        "leafCount": 1,
                        "providerDigest": "provider-test",
                    },
                    "query": "resident source index",
                    "languageId": "rust",
                    "limit": 32,
                },
            }
        )
    )


@pytest.mark.parametrize(
    "result",
    [
        {"state": "healthy"},
        {
            "state": "source-index",
            "lookup": {
                "db_path": "/tmp/facts.turso",
                "state": "miss",
                "candidates": [],
                "source_snapshot": None,
                "index_artifact_digest": None,
            },
        },
        {"state": "provider-incremental-owner", "receipt": {}},
        {"state": "provider-tree-sitter-query", "read": {}},
        {"state": "provider-owner-snapshot", "snapshot": {}},
        {"state": "provider-owner-warm", "probe": {}, "snapshot": {}},
        {"state": "provider-tree-sitter-owner", "receipt": {}},
        {"state": "provider-owners", "receipt": {}},
        {"state": "provider-inventory", "receipt": {}},
        {"state": "write-finish", "receipt": {}},
        {"state": "failed", "code": "failed-test", "message": "failure"},
    ],
)
def test_workspace_db_owner_response_accepts_typed_result_payloads(
    result: dict[str, object],
) -> None:
    Draft202012Validator.check_schema(SCHEMA)
    VALIDATOR.validate(response(result))


@pytest.mark.parametrize(
    "state",
    [
        "provider-incremental-owner",
        "source-index",
        "provider-tree-sitter-query",
        "provider-owner-snapshot",
        "provider-owner-warm",
        "provider-tree-sitter-owner",
        "provider-owners",
        "provider-inventory",
        "write-finish",
    ],
)
def test_workspace_db_owner_response_rejects_missing_success_payload(
    state: str,
) -> None:
    with pytest.raises(ValidationError):
        VALIDATOR.validate(response({"state": state}))
