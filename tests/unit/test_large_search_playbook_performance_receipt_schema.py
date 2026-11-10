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
        ROOT / "schemas/large-search-playbook-performance-receipt.v1.schema.json"
    ).read_text()
)


def test_large_search_playbook_performance_receipt_is_machine_validated() -> None:
    distribution = {
        "samples": 128,
        "p50Nanos": 90_000,
        "p95Nanos": 400_000,
        "p99Nanos": 1_100_000,
        "maxNanos": 12_000_000,
    }
    request_distribution = {
        "samples": 128,
        "p50Nanos": 90_000,
        "p95Nanos": 400_000,
        "p99Nanos": 700_000,
        "maxNanos": 900_000,
    }
    receipt = {
        "schemaId": "agent.semantic-protocols.large-search-playbook-performance-receipt",
        "schemaVersion": "1",
        "owners": 4096,
        "contentGeneration": {
            **distribution,
            "samples": 32,
            "stageP95Nanos": {
                "fdInventory": 4_000_000,
                "repositoryAdmission": 6_000_000,
                "sourceByteAcquisition": 10_000_000,
                "nativeSyntax": 40_000_000,
                "rustGraph": 320_000_000,
                "publication": 20_000_000,
            },
        },
        "coldRgQuery": {
            **request_distribution,
            "samples": 32,
            "fdProcessCount": 0,
            "rgProcessCount": 0,
            "tantivyBuildCount": 0,
            "contentVerificationP95Nanos": 8_000_000,
            "nativeSyntaxVerificationP95Nanos": 5_000_000,
        },
        "bootstrapAccelerator": {
            **distribution,
            "samples": 32,
            "reusedShardCount": 0,
            "rebuiltShardCount": 4096,
            "shardPlanP95Nanos": 20_000_000,
            "tantivyCommitP95Nanos": 830_000_000,
            "equivalenceValidationP95Nanos": 15_000_000,
            "pointerPublicationP95Nanos": 4_000_000,
        },
        "deltaAccelerator": {
            **distribution,
            "samples": 32,
            "changedOwnerCount": 1,
            "reusedShardCount": 4095,
            "rebuiltShardCount": 1,
            "retiredShardCount": 1,
            "unchangedOwnerRescanCount": 0,
        },
        "firstTantivyOpen": {
            **distribution,
            "samples": 32,
            "filesystemReadCount": 32,
            "fdProcessCount": 0,
            "rgProcessCount": 0,
            "tantivyBuildCount": 0,
        },
        "warm": {**request_distribution, "samples": 512},
        "concurrent": {
            "queries": 32,
            "p99Nanos": 700_000,
            "maxNanos": 1_200_000,
            "wallNanos": 9_000_000,
        },
        "fusedCache": {
            "hitCount": 1,
            "missCount": 0,
            "entryCount": 64,
            "valueBytes": 16_384,
            "capacity": 1_024,
            "shardCount": 64,
            "externalWorkCount": 0,
        },
        "pythonGraph": {
            "state": "executed",
            "generationDigest": f"blake3-256:{'d' * 64}",
            "receiptValidationNanos": 2_500_000,
        },
        "requestTimeExternalWork": {
            "databaseReadCount": 0,
            "filesystemReadCount": 0,
            "providerProcessCount": 0,
            "socketOperationCount": 0,
            "schedulerTaskCount": 0,
        },
    }
    validator = schema_validator_for(
        ROOT / "schemas/large-search-playbook-performance-receipt.v1.schema.json"
    )
    jsonschema.Draft202012Validator.check_schema(validator.schema)
    validator.validate(receipt)


def test_large_search_playbook_performance_receipt_rejects_single_sample() -> None:
    for phase in (
        "contentGeneration",
        "coldRgQuery",
        "bootstrapAccelerator",
        "deltaAccelerator",
        "firstTantivyOpen",
    ):
        assert SCHEMA["properties"][phase]["allOf"][1]["properties"]["samples"]["minimum"] == 32
    assert (
        SCHEMA["properties"]["warm"]["allOf"][1]["properties"]["samples"]["minimum"]
        == 512
    )
