# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from copy import deepcopy
from pathlib import Path

import jsonschema
import pytest


REPO_ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (REPO_ROOT / "schemas/incremental-search-generation.v1.schema.json").read_text(
        encoding="utf-8"
    )
)
QUERY_DIGEST = "a" * 64
OWNER_DIGEST = "b" * 64
WORKSPACE_DIGEST = "c" * 64
GENERATION_DIGEST = "d" * 64
INVENTORY_DIGEST = "e" * 64


def valid_tree_sitter_packet(*, partial: bool = False) -> dict:
    inventory_state = "known" if partial else "exact"
    count_kind = "known-lower-bound" if partial else "exact"
    processed = 1 if partial else 0
    return {
        "schemaId": "agent.semantic-protocols.incremental-search-generation",
        "schemaVersion": "1",
        "operation": "treesitter-query",
        "languageId": "rust",
        "providerId": "asp-rust",
        "providerWorkspaceRoot": ".",
        "providerWorkspaceIdentityDigest": WORKSPACE_DIGEST,
        "generationBefore": None if partial else generation(),
        "generationAfter": generation(),
        "query": {
            "kind": "treesitter",
            "queryDigest": QUERY_DIGEST,
            "captureNames": ["declaration.name"],
        },
        "inventory": {
            "state": inventory_state,
            "knownOwnerCount": 2 if partial else 1,
            "inventoryDigest": INVENTORY_DIGEST,
        },
        "completeness": {
            "state": "partial" if partial else "complete",
            "indexedOwnerCount": 1,
            "dirtyOwnerCount": 2 if partial else 0,
            "processedOwnerCount": processed,
            "remainingOwnerCount": 1 if partial else 0,
            "remainingCountKind": count_kind,
            "incrementalBudget": 1,
        },
        "ownerAcquisitions": [owner_acquisition("new" if partial else "unchanged")],
        "queryOwnerResults": [
            {
                "ownerPath": "src/lib.rs",
                "ownerContentDigest": OWNER_DIGEST,
                "queryDigest": QUERY_DIGEST,
                "state": "processed" if partial else "cached",
                "captureCount": 1,
                "completeOwnerRefreshCount": processed,
            }
        ],
        "projections": [capture_projection()],
        "counters": counters(processed),
        "continuation": (
            {
                "providerWorkspaceIdentityDigest": WORKSPACE_DIGEST,
                "queryDigest": QUERY_DIGEST,
                "generationRootDigest": GENERATION_DIGEST,
                "inventoryState": inventory_state,
                "remainingCountKind": count_kind,
                "nextOwnerCursor": "src/next.rs",
            }
            if partial
            else None
        ),
    }


@pytest.mark.parametrize("partial", [False, True])
def test_incremental_search_generation_v1_accepts_query_specific_packets(
    partial: bool,
) -> None:
    jsonschema.validate(valid_tree_sitter_packet(partial=partial), SCHEMA)


def test_incremental_search_generation_v1_keeps_owner_items_query_independent() -> None:
    packet = deepcopy(valid_tree_sitter_packet())
    packet["operation"] = "owner-items"
    packet.pop("query")
    packet.pop("inventory")
    packet.pop("queryOwnerResults")
    packet["projections"][0].pop("queryDigest")
    packet["projections"][0].pop("itemSourceByteStart")
    packet["projections"][0].pop("itemSourceByteEnd")
    jsonschema.validate(packet, SCHEMA)


@pytest.mark.parametrize(
    "mutation",
    [
        lambda packet: packet.pop("query"),
        lambda packet: packet["projections"][0].update(signature=""),
        lambda packet: packet["projections"][0].pop("queryDigest"),
        lambda packet: packet["queryOwnerResults"][0].update(
            completeOwnerRefreshCount=2
        ),
        lambda packet: packet.update(continuation=None),
    ],
)
def test_incremental_search_generation_v1_rejects_shape_drift(mutation) -> None:
    packet = deepcopy(valid_tree_sitter_packet(partial=True))
    mutation(packet)
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(packet, SCHEMA)


def generation() -> dict:
    return {
        "rootDigest": GENERATION_DIGEST,
        "leafCount": 1,
        "ownerCount": 1,
    }


def owner_acquisition(decision: str) -> dict:
    return {
        "ownerPath": "src/lib.rs",
        "decision": decision,
        "fingerprint": {
            "fileIdentity": "unix:1:2",
            "sizeBytes": 100,
            "modifiedUnixNanos": 1,
            "changeTimeUnixNanos": 2,
            "contentDigest": OWNER_DIGEST,
        },
    }


def capture_projection() -> dict:
    return {
        "ownerPath": "src/lib.rs",
        "sourceContentDigest": OWNER_DIGEST,
        "queryDigest": QUERY_DIGEST,
        "structuralSelector": "rust://src/lib.rs#item/function/alpha",
        "signature": "pub fn alpha()",
        "itemKind": "function",
        "itemName": "alpha",
        "captureName": "declaration.name",
        "itemSourceByteStart": 0,
        "itemSourceByteEnd": 30,
        "sourceByteStart": 7,
        "sourceByteEnd": 12,
    }


def counters(processed: int) -> dict:
    return {
        "metadataReads": 1,
        "sourceByteReads": processed,
        "sourceBytesRead": processed * 100,
        "providerParses": processed,
        "queryCacheReads": 1,
        "queryCacheWrites": processed,
        "completeOwnerRefreshes": processed,
        "ownerIndexWrites": processed,
        "casWrites": 0,
        "merkleLeafWrites": processed,
        "merklePathNodeWrites": processed,
        "fullSourceWalks": 0,
        "fullCasMaterializations": 0,
        "fullMerkleRebuilds": 0,
        "unrelatedProviderCount": 0,
    }
