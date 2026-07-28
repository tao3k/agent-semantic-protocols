import json
from pathlib import Path

import jsonschema
import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = REPO_ROOT / "schemas"


def load_schema(name: str) -> dict:
    return json.loads((SCHEMA_ROOT / name).read_text(encoding="utf-8"))


def valid_request() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-request",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "workspaceIdentity": "workspace-1",
        "providerWorkspaceIdentityDigest": "a" * 64,
        "ownerPath": "src/lib.rs",
        "sourceFingerprint": {
            "fileIdentity": "file-1",
            "sizeBytes": 18,
            "modifiedUnixNanos": 1,
            "changeTimeUnixNanos": 2,
            "contentDigest": "b" * 64,
        },
        "sourceEncoding": "base64",
        "sourceBytesBase64": "cHViIGZuIGFscGhhKCkge30K",
        "query": {"text": "alpha", "itemMode": "items"},
        "transport": "stdin-json",
    }


def valid_response() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.provider-native-owner-search-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "requestedOwnerPath": "src/lib.rs",
        "requestedQuery": "alpha",
        "sourceContentDigest": "b" * 64,
        "parsedOwnerCount": 1,
        "projectionCompleteness": "complete-owner",
        "projections": [
            {
                "structuralSelector": "rust://src/lib.rs#item/function/alpha",
                "signature": "pub fn alpha()",
                "itemKind": "function",
                "itemName": "alpha",
                "captureName": "declaration.name",
                "sourceByteStart": 0,
                "sourceByteEnd": 17,
            }
        ],
    }


def test_provider_native_owner_search_v1_accepts_complete_echo_contract() -> None:
    jsonschema.validate(
        valid_request(),
        load_schema("provider-native-owner-search-request.v1.schema.json"),
    )
    jsonschema.validate(
        valid_response(),
        load_schema("provider-native-owner-search-response.v1.schema.json"),
    )


@pytest.mark.parametrize(
    ("packet", "schema_name"),
    [
        (
            {
                **valid_request(),
                "sourceFingerprint": {
                    **valid_request()["sourceFingerprint"],
                    "contentDigest": "not-a-digest",
                },
            },
            "provider-native-owner-search-request.v1.schema.json",
        ),
        (
            {key: value for key, value in valid_response().items() if key != "requestedOwnerPath"},
            "provider-native-owner-search-response.v1.schema.json",
        ),
        (
            {**valid_response(), "parsedOwnerCount": 2},
            "provider-native-owner-search-response.v1.schema.json",
        ),
        (
            {**valid_response(), "projectionCompleteness": "query-filtered"},
            "provider-native-owner-search-response.v1.schema.json",
        ),
    ],
)
def test_provider_native_owner_search_v1_rejects_contract_drift(
    packet: dict, schema_name: str
) -> None:
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(packet, load_schema(schema_name))
