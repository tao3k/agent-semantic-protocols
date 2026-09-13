# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).parents[2]


def schema(name: str) -> dict:
    return json.loads((ROOT / "schemas" / name).read_text())


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def test_runtime_search_calibration_receipts_are_machine_validated() -> None:
    observation = {
        "schemaId": "agent.semantic-protocols.runtime-search-generation-calibration-receipt",
        "schemaVersion": "1",
        "contentGenerationDigest": digest("a"),
        "sampleIdentity": digest("b"),
        "sampledOwnerCount": 256,
        "strategy": "parallel-segments",
        "workers": 8,
        "memoryBudgetBytes": 120_000_000,
        "buildMicros": 10_000,
        "observedOwnersPerSecond": 25_600,
    }
    jsonschema.Draft202012Validator(
        schema("runtime-search-generation-calibration-receipt.v1.schema.json")
    ).validate(observation)

    decision = {
        "schemaId": "agent.semantic-protocols.runtime-search-generation-calibration-decision-receipt",
        "schemaVersion": "1",
        "state": "ready",
        "contentGenerationDigest": digest("a"),
        "roundCount": 4,
        "observationCount": 19,
        "sampleOwnerLimit": 256,
        "calibrationMicros": 400_000,
        "calibrationMicrosLimit": 1_000_000,
        "strategy": "parallel-segments",
        "workers": 8,
        "memoryBudgetBytes": 120_000_000,
        "sampleIdentity": digest("b"),
    }
    jsonschema.Draft202012Validator(
        schema("runtime-search-generation-calibration-decision-receipt.v1.schema.json")
    ).validate(decision)


def test_runtime_search_calibration_store_uses_structured_exact_identity() -> None:
    store = {
        "schemaId": "agent.semantic-protocols.runtime-search-calibration-store",
        "schemaVersion": "1",
        "entries": [
            {
                "key": {
                    "engineDigest": digest("c"),
                    "effectiveCpu": 8,
                    "processMemoryBudgetBytes": 240_000_000,
                    "workload": {
                        "strategy": 1,
                        "lexicalBytesBucket": 21,
                        "ownerCountBucket": 12,
                        "changedRatioBucket": 8,
                    },
                },
                "decision": {
                    "strategy": "parallel-segments",
                    "workers": 8,
                    "memoryBudgetBytes": 120_000_000,
                    "observedOwnersPerSecond": 7_833,
                    "sampleIdentity": digest("d"),
                },
            }
        ],
    }
    calibration_store_schema = schema("runtime-search-calibration-store.v1.schema.json")
    jsonschema.Draft202012Validator(calibration_store_schema).validate(store)
    invalid = dict(store)
    invalid["entries"] = [dict(store["entries"][0], key="opaque-string-key")]
    errors = list(jsonschema.Draft202012Validator(calibration_store_schema).iter_errors(invalid))
    assert errors
