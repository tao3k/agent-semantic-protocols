# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from pathlib import Path

import json
import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]


def validator(name: str) -> Draft202012Validator:
    schema = json.loads((ROOT / "schemas" / name).read_text())
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def request() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.asp-client-server-request",
        "schemaVersion": "1",
        "operationId": "operation-1",
        "extensionId": "rust",
        "languageId": "rust",
        "providerId": "asp-rust",
        "workspaceIdentity": "workspace-1",
        "generationDigest": "blake3-256:generation",
        "rootDigest": "blake3-256:root",
        "providerCatalogDigest": "blake3-256:catalog",
        "transport": "http-json",
        "operation": "owner-items",
        "projectionKind": "owner-items",
        "overlayIdentity": None,
        "budgetMicros": 1000,
        "payloadDigest": "blake3-256:payload",
        "payload": {"ownerPath": "src/lib.rs"},
    }


def response(terminal_status: str) -> dict[str, object]:
    receipt: dict[str, object] = {
        "schemaId": "agent.semantic-protocols.asp-client-server-response",
        "schemaVersion": "1",
        "operationId": "operation-1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "workspaceIdentity": "workspace-1",
        "generationDigest": "blake3-256:generation",
        "rootDigest": "blake3-256:root",
        "providerCatalogDigest": "blake3-256:catalog",
        "transport": "http-json",
        "operation": "owner-items",
        "projectionKind": "owner-items",
        "requestPayloadDigest": "blake3-256:payload",
        "terminalStatus": terminal_status,
        "cacheState": "hit",
        "overlayIdentity": None,
        "elapsedMicros": 400,
        "resultDigest": "blake3-256:result",
    }
    if terminal_status == "completed":
        receipt["result"] = {"items": []}
    else:
        receipt["error"] = {
            "reasonKind": "operation-cancelled",
            "message": "operation was cancelled",
        }
    return receipt


def test_agent_operation_request_is_generation_bound_and_http_first() -> None:
    validator("asp-client-server-request.v1.schema.json").validate(request())


@pytest.mark.parametrize("transport", ["unsupported-transport"])
def test_versioned_transport_aliases_are_rejected(transport: str) -> None:
    payload = request()
    payload["transport"] = transport
    with pytest.raises(ValidationError):
        validator("asp-client-server-request.v1.schema.json").validate(payload)


def test_request_rejects_missing_generation_authority() -> None:
    payload = request()
    del payload["generationDigest"]
    with pytest.raises(ValidationError):
        validator("asp-client-server-request.v1.schema.json").validate(payload)


@pytest.mark.parametrize(
    "legacy_provider_id",
    ["asp-rust", "asp-python", "asp-typescript", "orgize", "asp+rust"],
)
def test_request_rejects_implementation_named_provider_ids(
    legacy_provider_id: str,
) -> None:
    payload = request()
    payload["providerId"] = legacy_provider_id
    with pytest.raises(ValidationError):
        validator("asp-client-server-request.v1.schema.json").validate(payload)


def test_request_rejects_provider_id_from_a_different_language() -> None:
    payload = request()
    payload["providerId"] = "asp-python"
    with pytest.raises(ValidationError):
        validator("asp-client-server-request.v1.schema.json").validate(payload)


@pytest.mark.parametrize("terminal_status", ["completed", "cancelled", "failed"])
def test_every_operation_has_one_typed_terminal_response(terminal_status: str) -> None:
    validator("asp-client-server-response.v1.schema.json").validate(
        response(terminal_status)
    )


def test_completed_and_cancelled_responses_are_not_interchangeable() -> None:
    payload = response("cancelled")
    del payload["error"]
    payload["result"] = {"items": []}
    with pytest.raises(ValidationError):
        validator("asp-client-server-response.v1.schema.json").validate(payload)


def test_response_rejects_missing_request_cache_identity() -> None:
    payload = response("completed")
    del payload["requestPayloadDigest"]
    with pytest.raises(ValidationError):
        validator("asp-client-server-response.v1.schema.json").validate(payload)


@pytest.mark.parametrize(
    ("schema_name", "fixture_name"),
    [
        (
            "asp-client-server-request.v1.schema.json",
            "asp-client-server-request.owner-items.v1.json",
        ),
        (
            "asp-client-server-response.v1.schema.json",
            "asp-client-server-response.completed.v1.json",
        ),
        (
            "asp-client-server-response.v1.schema.json",
            "asp-client-server-response.cancelled.v1.json",
        ),
    ],
)
def test_repository_operation_fixtures_match_shared_contract(
    schema_name: str,
    fixture_name: str,
) -> None:
    fixture = json.loads((ROOT / "schemas" / "fixtures" / fixture_name).read_text())
    validator(schema_name).validate(fixture)


def test_terminal_fixtures_preserve_request_authority() -> None:
    fixture_root = ROOT / "schemas" / "fixtures"
    request_fixture = json.loads(
        (fixture_root / "asp-client-server-request.owner-items.v1.json").read_text()
    )
    responses = [
        json.loads(
            (fixture_root / "asp-client-server-response.completed.v1.json").read_text()
        ),
        json.loads(
            (fixture_root / "asp-client-server-response.cancelled.v1.json").read_text()
        ),
    ]
    authority_fields = (
        "operationId",
        "languageId",
        "providerId",
        "workspaceIdentity",
        "generationDigest",
        "rootDigest",
        "providerCatalogDigest",
        "transport",
        "operation",
        "projectionKind",
        "overlayIdentity",
    )
    for response_fixture in responses:
        assert {
            field: response_fixture[field] for field in authority_fields
        } == {field: request_fixture[field] for field in authority_fields}
        assert response_fixture["requestPayloadDigest"] == request_fixture["payloadDigest"]
    assert {receipt["terminalStatus"] for receipt in responses} == {
        "completed",
        "cancelled",
    }
