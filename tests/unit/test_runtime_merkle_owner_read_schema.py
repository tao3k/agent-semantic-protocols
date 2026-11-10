# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]


def validator(schema_name: str) -> Draft202012Validator:
    schema = json.loads((ROOT / "schemas" / schema_name).read_text())
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def digest(byte: str) -> str:
    return f"blake3-256:{byte * 64}"


def test_runtime_merkle_owner_read_request_schema_accepts_normalized_owner() -> None:
    validator("runtime-merkle-owner-read-request.v1.schema.json").validate(
        {
            "schemaId": "agent.semantic-protocols.runtime-merkle-owner-read-request",
            "schemaVersion": "1",
            "projectRoot": "/workspace/root",
            "ownerPath": "src/lib.rs",
        }
    )


def test_runtime_merkle_owner_read_receipt_schema_covers_hit_and_miss() -> None:
    receipt_validator = validator(
        "runtime-merkle-owner-read-receipt.v1.schema.json"
    )
    common = {
        "schemaId": "agent.semantic-protocols.runtime-merkle-owner-read-receipt",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-test",
        "projectRoot": "/workspace/root",
        "activeEpoch": 1,
        "generationDigest": digest("1"),
        "rootDigest": digest("2"),
        "ownerPath": "src/lib.rs",
    }
    receipt_validator.validate(
        {
            **common,
            "state": "owner",
            "sourceBlobDigest": digest("3"),
            "ownerSubtreeDigest": digest("4"),
            "inclusionProof": [{"side": "right", "digest": digest("5")}],
        }
    )
    receipt_validator.validate({**common, "state": "owner-missing"})


def test_workspace_memory_generation_segment_v1_requires_merkle_owner_index() -> None:
    schema = json.loads(
        (ROOT / "schemas" / "workspace-memory-generation-segment.v1.schema.json").read_text()
    )
    Draft202012Validator.check_schema(schema)
    kinds = schema["$defs"]["section"]["properties"]["kind"]["enum"]
    assert "merkle-owner-index" in kinds
    assert schema["properties"]["sections"]["minItems"] == 8
    assert schema["properties"]["sections"]["maxItems"] == 8
