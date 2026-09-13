# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "schemas"
    / "semantic-ready-search-ablation-receipt.v1.schema.json"
)


def _receipt() -> dict:
    digest = "blake3-256:" + "a" * 64
    return {
        "schemaId": "agent.semantic-protocols.semantic-ready-search-ablation-receipt",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-fixture",
        "sourceRootDigest": digest,
        "generationDigest": digest,
        "schemaCatalogDigest": digest,
        "algorithmDigest": digest,
        "planDigest": digest,
        "languageId": "rust",
        "operation": "search",
        "transport": "grpc",
        "cacheMode": "warm-read",
        "budget": {
            "exactEntries": 1,
            "lexicalPostings": 4096,
            "graphNodes": 256,
            "graphEdges": 1024,
            "outputBytes": 65536,
        },
        "work": {
            "exactEntries": 0,
            "lexicalPostings": 128,
            "lexicalBlocksSkipped": 12,
            "graphNodes": 8,
            "graphEdges": 16,
            "graphFrontierPeak": 4,
            "outputBytes": 2048,
        },
        "effects": {
            "providerProcessCount": 0,
            "providerRpcCount": 0,
            "workspaceScanCount": 0,
            "durableDbReadCount": 0,
            "durableDbWriteCount": 0,
            "generationMutationCount": 0,
            "endpointDiscoveryCount": 0,
            "clientRetryCount": 0,
        },
        "timing": {
            "serviceCoreNs": 530000,
            "frameCodecNs": 100000,
            "generationLeaseNs": 70000,
            "planNs": 40000,
            "residentReadNs": 280000,
            "mergeEncodeNs": 140000,
            "transportNs": 100000,
            "totalNs": 730000,
        },
        "terminal": {"count": 1, "kind": "ready", "reasonKind": None},
        "resultDigest": digest,
    }


def _validator() -> Draft202012Validator:
    return Draft202012Validator(json.loads(SCHEMA_PATH.read_text()))


def test_ready_search_ablation_receipt_accepts_bounded_resident_grpc() -> None:
    _validator().validate(_receipt())


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("providerRpcCount", 1),
        ("workspaceScanCount", 1),
        ("durableDbReadCount", 1),
        ("generationMutationCount", 1),
        ("clientRetryCount", 1),
    ],
)
def test_ready_search_ablation_receipt_rejects_legacy_effects(
    field: str, value: int
) -> None:
    receipt = _receipt()
    receipt["effects"][field] = value
    with pytest.raises(ValidationError):
        _validator().validate(receipt)


def test_ready_search_ablation_receipt_rejects_one_millisecond() -> None:
    receipt = _receipt()
    receipt["timing"]["totalNs"] = 1_000_000
    with pytest.raises(ValidationError):
        _validator().validate(receipt)


def test_ready_search_ablation_receipt_requires_exactly_one_terminal() -> None:
    receipt = _receipt()
    receipt["terminal"]["count"] = 2
    with pytest.raises(ValidationError):
        _validator().validate(receipt)
