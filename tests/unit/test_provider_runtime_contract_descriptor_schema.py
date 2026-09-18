# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from .schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "provider-runtime-contract-descriptor.schema.json"
CLIENT_SERVER_SCHEMA_PATHS = (
    ROOT / "schemas" / "asp-client-server-bootstrap.schema.json",
    ROOT / "schemas" / "asp-client-server-descriptor.schema.json",
)


def validator() -> Draft202012Validator:
    return schema_validator_for(SCHEMA_PATH)


def contract(transport: str) -> dict[str, object]:
    value: dict[str, object] = {
        "transport": transport,
        "clientBinding": "schema-driven",
        "operations": [
            {
                "operation": "projection-batch-stdin",
                "requestSchema": {
                    "schemaId": "provider-language-projection-batch-request",
                    "schemaVersion": "1",
                },
                "responseSchema": {
                    "schemaId": "provider-language-projection-batch-response",
                    "schemaVersion": "1",
                },
            }
        ],
    }
    if transport == "http-json":
        value["aspClientServer"] = {
            "schemaId": "agent.semantic-protocols.asp-client-server-descriptor",
            "schemaVersion": "1",
            "transport": "http-json",
            "command": ["serve"],
            "healthPath": "/health",
            "requestPath": "/v1/provider-runtime",
            "shutdownPath": "/shutdown",
            "warmupPolicy": "before-ready",
        }
    return value


def test_runtime_transport_is_http_json_only() -> None:
    schema_validator = validator()

    assert not schema_validator.is_valid(contract("runtime-ipc"))
    schema_validator.validate(contract("http-json"))
    assert not schema_validator.is_valid(contract("in-process"))


def test_versioned_and_legacy_runtime_transport_names_are_rejected() -> None:
    schema_validator = validator()

    for transport in ("unsupported-transport",):
        errors = list(schema_validator.iter_errors(contract(transport)))
        assert errors, f"legacy transport namespace must be rejected: {transport}"


def test_legacy_string_schema_references_are_rejected() -> None:
    value = contract("http-json")
    operation = value["operations"][0]
    operation.pop("requestSchema")
    operation.pop("responseSchema")
    operation["requestSchemaId"] = "provider-language-projection-batch-request"
    operation["responseSchemaId"] = "provider-language-projection-batch-response"

    assert not validator().is_valid(value)


def test_runtime_contract_schema_remains_v1_owned() -> None:
    schema = json.loads(SCHEMA_PATH.read_text())

    assert SCHEMA_PATH.name == "provider-runtime-contract-descriptor.schema.json"
    assert schema["$id"].endswith("provider-runtime-contract-descriptor.schema.json")


def test_language_client_server_http_namespace_is_unversioned() -> None:
    for schema_path in CLIENT_SERVER_SCHEMA_PATHS:
        schema = json.loads(schema_path.read_text())
        Draft202012Validator.check_schema(schema)
        assert schema_path.name.endswith(".schema.json")
        assert ".v1.schema.json" not in schema_path.name
        assert ".v1.schema.json" not in schema["$id"]
        assert schema["properties"]["schemaVersion"]["const"] == "1"
        assert schema["properties"]["transport"]["const"] == "http-json"
