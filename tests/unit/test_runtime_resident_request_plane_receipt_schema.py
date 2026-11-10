# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-resident-request-plane-receipt.v1.schema.json").read_text()
)


def receipt(*, state: str, temperature: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-resident-request-plane-receipt",
        "schemaVersion": "1",
        "operation": "search",
        "requestTemperature": temperature,
        "state": state,
        "generationDigest": f"blake3-256:{'a' * 64}" if state == "ready" else None,
        "elapsedMicros": 999,
        "generationLookupCount": 1,
        "generationWaitCount": 0,
        "generationBuildCount": 0,
        "filesystemReadCount": 0,
        "databaseReadCount": 0,
        "providerProcessCount": 0,
        "parserInvocationCount": 0,
        "secondaryRuntimeRpcCount": 0,
        "socketDiscoveryCount": 0,
        "terminalWaitCount": 0,
    }


@pytest.mark.parametrize("state", ["ready", "query-not-ready"])
@pytest.mark.parametrize("temperature", ["cold", "warm"])
def test_cold_and_warm_resident_request_receipts_are_sub_millisecond(
    state: str, temperature: str
) -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(
        receipt(state=state, temperature=temperature)
    )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("elapsedMicros", 1000),
        ("generationWaitCount", 1),
        ("generationBuildCount", 1),
        ("filesystemReadCount", 1),
        ("databaseReadCount", 1),
        ("providerProcessCount", 1),
        ("parserInvocationCount", 1),
        ("secondaryRuntimeRpcCount", 1),
        ("socketDiscoveryCount", 1),
        ("terminalWaitCount", 1),
    ],
)
def test_request_plane_rejects_latency_or_detached_generation_work(
    field: str, value: int
) -> None:
    invalid = receipt(state="ready", temperature="warm")
    invalid[field] = value
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(invalid)
