# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from pathlib import Path

import json
import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_NAME = "asp-client-server-lifecycle-receipt.schema.json"


def validator() -> Draft202012Validator:
    schema = json.loads((ROOT / "schemas" / SCHEMA_NAME).read_text())
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def lifecycle_receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.asp-client-server-lifecycle-receipt",
        "schemaVersion": "1",
        "operationId": "lifecycle-1",
        "extensionId": "rust",
        "languageId": "rust",
        "providerId": "asp-rust",
        "workspaceIdentity": "workspace-1",
        "generationDigest": "blake3-256:generation",
        "rootDigest": "blake3-256:root",
        "providerCatalogDigest": "blake3-256:catalog",
        "transport": "http-json",
        "eventKind": "ready",
        "terminalStatus": "completed",
        "state": "ready",
        "attempt": 1,
        "publicationEpoch": 1,
        "artifactDigest": "blake3-256:artifact",
        "registrationDigest": "blake3-256:registration",
        "runtimeContractDigest": "blake3-256:contract",
        "leaseCount": 1,
        "elapsedMicros": 500,
        "endpoint": "http://127.0.0.1:3000",
        "operations": ["owner-items", "exact-source", "search"],
        "error": None,
    }


def test_lifecycle_receipt_binds_otel_and_agent_authority() -> None:
    validator().validate(lifecycle_receipt())


def test_repository_ready_lifecycle_fixture_matches_shared_contract() -> None:
    fixture = json.loads(
        (
            ROOT
            / "schemas"
            / "fixtures"
            / "asp-client-server-lifecycle.ready.v1.json"
        ).read_text()
    )
    validator().validate(fixture)


@pytest.mark.parametrize(
    "missing",
    [
        "operationId",
        "workspaceIdentity",
        "generationDigest",
        "rootDigest",
        "providerCatalogDigest",
        "terminalStatus",
        "leaseCount",
        "elapsedMicros",
    ],
)
def test_lifecycle_receipt_rejects_missing_authority_witness(missing: str) -> None:
    payload = lifecycle_receipt()
    del payload[missing]
    with pytest.raises(ValidationError):
        validator().validate(payload)
