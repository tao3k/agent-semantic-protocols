# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
import pytest


SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "schemas"
    / "workspace-db-owner-ipc.v1.schema.json"
)


def endpoint() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.workspace-db-owner-endpoint.v1",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-test",
        "transportContractDigest": "blake3-256:test-contract",
        "ownerEpoch": 1,
        "ownerPid": 701,
        "runtimeBinaryPath": "/runtime/bin/asp",
        "runtimeBinaryDigest": "blake3-256:test-runtime",
        "bindingToken": "binding-test",
        "dataEndpoint": {
            "transport": "loopback-tcp",
            "address": "127.0.0.1",
            "port": 4317,
        },
    }


def endpoint_validator() -> jsonschema.Draft202012Validator:
    schema = json.loads(SCHEMA_PATH.read_text())
    endpoint_schema = dict(schema["$defs"]["endpoint"])
    endpoint_schema["$defs"] = schema["$defs"]
    return jsonschema.Draft202012Validator(endpoint_schema)


def test_workspace_db_owner_endpoint_requires_transport_contract_identity() -> None:
    validator = endpoint_validator()
    validator.validate(endpoint())

    missing_contract = endpoint()
    del missing_contract["transportContractDigest"]
    with pytest.raises(jsonschema.ValidationError):
        validator.validate(missing_contract)

    for field in ("ownerPid", "runtimeBinaryPath", "runtimeBinaryDigest"):
        missing_runtime_identity = endpoint()
        del missing_runtime_identity[field]
        with pytest.raises(jsonschema.ValidationError):
            validator.validate(missing_runtime_identity)
