# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def test_client_protocol_schemas_are_valid_draft_2020_12() -> None:
    for name in (
        "asp-client-frame.schema.json",
        "asp-client-protocol-catalog.schema.json",
        "asp-client-conformance.schema.json",
        "runtime-search-client-timing-witness.v1.schema.json",
    ):
        Draft202012Validator.check_schema(json.loads((SCHEMAS / name).read_text()))


def test_request_frame_admits_only_a_correlated_identity_free_timing_witness() -> None:
    frame = {
        "schemaId": "agent.semantic-protocols.client.frame",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "kind": "request",
        "sessionId": "session-1",
        "projectId": "project-1",
        "workspaceId": "workspace-1",
        "requestId": "request-1",
        "catalogGeneration": "catalog-1",
        "workspaceGeneration": "generation-1",
        "method": "rust.search",
        "params": {"query": "publication"},
        "clientTimingWitness": {
            "schemaId": "agent.semantic-protocols.runtime-search-client-timing-witness",
            "schemaVersion": "1",
            "sessionId": "session-1",
            "requestId": "request-1",
            "phases": [
                {"name": "launcher", "elapsedMicros": 1},
                {"name": "client-frame-encode", "elapsedMicros": 2},
                {"name": "ipc-connect", "elapsedMicros": 3},
            ],
        },
    }
    schema_validator_for(SCHEMAS / "asp-client-frame.schema.json").validate(frame)


def test_language_neutral_conformance_fixture_is_valid() -> None:
    fixture = json.loads(
        (SCHEMAS / "fixtures" / "asp-client-conformance" / "base.json").read_text()
    )
    schema_validator_for(SCHEMAS / "asp-client-conformance.schema.json").validate(fixture)


def test_client_catalog_exposes_codegen_parameter_contract() -> None:
    catalog = {
        "schemaId": "agent.semantic-protocols.client.protocol-catalog",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "catalogGeneration": f"sha256:{'a' * 64}",
        "workspaceGeneration": f"blake3-256:{'b' * 64}",
        "transports": ["http-json", "runtime-ipc"],
        "capabilities": {
            "requestCancellation": True,
            "events": True,
            "streaming": False,
            "traceContext": True,
        },
        "methods": [
            {
                "method": "rust.search",
                "routeId": "rust.search",
                "requestSchema": {
                    "schemaId": "agent.semantic-protocols.client.frame",
                    "schemaVersion": "1",
                },
                "responseSchema": {
                    "schemaId": "agent.semantic-protocols.search-packet",
                    "schemaVersion": "1",
                },
                "errorSchemas": [
                    {
                        "schemaId": "agent.semantic-protocols.route-failure",
                        "schemaVersion": "1",
                    }
                ],
                "parameters": [
                    {
                        "name": "query",
                        "valueType": "string",
                        "cardinality": "required",
                        "source": "request",
                    }
                ],
                "cancellable": True,
                "streaming": False,
            }
        ],
    }
    schema_validator_for(SCHEMAS / "asp-client-protocol-catalog.schema.json").validate(
        catalog
    )
