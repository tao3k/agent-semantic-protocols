# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).parents[2]
SCHEMA = json.loads(
    (
        ROOT / "schemas/python-generation-graph-performance-receipt.v1.schema.json"
    ).read_text()
)


def test_python_generation_graph_performance_receipt_is_machine_validated() -> None:
    distribution = {
        "samples": 128,
        "p50Nanos": 10_000_000,
        "p95Nanos": 200_000_000,
        "p99Nanos": 400_000_000,
        "maxNanos": 450_000_000,
    }
    receipt = {
        "schemaId": "agent.semantic-protocols.python-generation-graph-performance-receipt",
        "schemaVersion": "1",
        "owners": 4096,
        "cold": {**distribution, "samples": 8},
        "warm": distribution,
        "concurrent": {
            **distribution,
            "samples": 32,
            "queries": 32,
            "wallNanos": 800_000_000,
        },
        "artifactDigest": f"blake3-256:{'a' * 64}",
    }
    validator = schema_validator_for(
        ROOT / "schemas/python-generation-graph-performance-receipt.v1.schema.json"
    )
    jsonschema.Draft202012Validator.check_schema(validator.schema)
    validator.validate(receipt)


def test_python_generation_graph_performance_receipt_requires_real_sample_sets() -> (
    None
):
    assert (
        SCHEMA["properties"]["cold"]["allOf"][1]["properties"]["samples"]["minimum"]
        == 8
    )
    assert (
        SCHEMA["properties"]["warm"]["allOf"][1]["properties"]["samples"]["minimum"]
        == 128
    )
