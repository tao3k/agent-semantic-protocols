# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

import jsonschema
import pytest
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
RECEIPT_SCHEMA = json.loads(
    (SCHEMAS / "resident-query-performance-receipt.v1.schema.json").read_text()
)
WORKSPACE_SCHEMA = json.loads(
    (SCHEMAS / "workspace-reference.v1.schema.json").read_text()
)
VALIDATOR = jsonschema.Draft202012Validator(
    RECEIPT_SCHEMA,
    registry=Registry().with_resource(
        WORKSPACE_SCHEMA["$id"],
        Resource.from_contents(WORKSPACE_SCHEMA),
    ),
)


def valid_receipt() -> dict:
    return {
        "schemaId": "asp.resident-query-performance-receipt",
        "schemaVersion": "1",
        "workspace": {
            "kind": "workspace-id",
            "workspaceId": "workspace-23cc5ba784c605ae",
        },
        "generationRoot": "blake3-256:" + ("a" * 64),
        "rootDepth": [1, 0],
        "phase": "cold-selector-cache-miss",
        "elapsedMicros": 999,
        "budgetMicros": 1000,
        "concurrency": {
            "workspaceCount": 4,
            "sessionCount": 16,
            "requestCount": 64,
        },
        "io": {
            "fsReads": 0,
            "dbOpens": 0,
            "manifestReads": 0,
            "processSpawns": 0,
            "lockProbes": 0,
            "schemaBootstraps": 0,
            "workspaceCanonicalizations": 0,
        },
    }


def test_resident_query_receipt_accepts_zero_io_sub_millisecond_query() -> None:
    VALIDATOR.validate(valid_receipt())


def test_resident_query_receipt_rejects_query_time_io() -> None:
    receipt = copy.deepcopy(valid_receipt())
    receipt["io"]["fsReads"] = 1
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(receipt)


def test_resident_query_receipt_rejects_query_over_budget() -> None:
    receipt = copy.deepcopy(valid_receipt())
    receipt["elapsedMicros"] = 1001
    with pytest.raises(jsonschema.ValidationError):
        VALIDATOR.validate(receipt)
