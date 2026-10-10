# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
TREE_SCHEMA = json.loads(
    (ROOT / "schemas/workspace-path-merkle-tree.v1.schema.json").read_text()
)
DELTA_SCHEMA = json.loads(
    (ROOT / "schemas/workspace-merkle-delta.v1.schema.json").read_text()
)
DIGEST = "blake3-256:" + "1" * 64
OTHER_DIGEST = "blake3-256:" + "2" * 64


def test_workspace_path_merkle_tree_v1_incremental_accepts_a_generation_bound_proof():
    packet = {
        "schemaId": "agent.semantic-protocols.workspace-path-merkle-tree",
        "schemaVersion": "1",
        "algorithm": "workspace-path-radix-merkle-v1",
        "workspaceIdentity": "workspace-test",
        "generationDigest": DIGEST,
        "rootDigest": OTHER_DIGEST,
        "leafCount": 2,
        "nodeCount": 15,
        "nodeTableDigest": DIGEST,
        "proof": {
            "ownerPath": "src/lib.rs",
            "sourceBlobDigest": DIGEST,
            "ownerSubtreeDigest": OTHER_DIGEST,
            "steps": [
                {
                    "depth": 0,
                    "terminalDigest": None,
                    "siblings": [{"edge": 115, "digest": DIGEST}],
                }
            ],
        },
    }
    jsonschema.validate(packet, TREE_SCHEMA)


def test_workspace_merkle_delta_v1_incremental_accepts_base_bound_upsert_and_remove():
    packet = {
        "schemaId": "agent.semantic-protocols.workspace-merkle-delta",
        "schemaVersion": "1",
        "algorithm": "workspace-path-radix-merkle-v1",
        "workspaceIdentity": "workspace-test",
        "mutationId": "mutation-1",
        "baseGenerationDigest": DIGEST,
        "baseRootDigest": OTHER_DIGEST,
        "operations": [
            {
                "kind": "upsert",
                "ownerPath": "src/lib.rs",
                "previousSourceBlobDigest": DIGEST,
                "sourceBlobDigest": OTHER_DIGEST,
            },
            {
                "kind": "remove",
                "ownerPath": "src/old.rs",
                "previousSourceBlobDigest": DIGEST,
            },
        ],
        "successorRootDigest": DIGEST,
        "dirtyPathsDigest": OTHER_DIGEST,
        "touchedLeafCount": 2,
        "writtenNodeCount": 12,
        "reusedNodeCount": 40,
        "fullMerkleRebuilds": 0,
    }
    jsonschema.validate(packet, DELTA_SCHEMA)


@pytest.mark.parametrize(
    "field,value",
    [
        ("baseRootDigest", "1" * 64),
        ("fullMerkleRebuilds", 1),
        ("operations", []),
    ],
)
def test_workspace_merkle_delta_v1_incremental_rejects_non_incremental_or_unbound_packets(
    field, value
):
    packet = {
        "schemaId": "agent.semantic-protocols.workspace-merkle-delta",
        "schemaVersion": "1",
        "algorithm": "workspace-path-radix-merkle-v1",
        "workspaceIdentity": "workspace-test",
        "mutationId": "mutation-1",
        "baseGenerationDigest": DIGEST,
        "baseRootDigest": OTHER_DIGEST,
        "operations": [
            {
                "kind": "upsert",
                "ownerPath": "src/lib.rs",
                "previousSourceBlobDigest": None,
                "sourceBlobDigest": DIGEST,
            }
        ],
        "successorRootDigest": OTHER_DIGEST,
        "dirtyPathsDigest": DIGEST,
        "touchedLeafCount": 1,
        "writtenNodeCount": 8,
        "reusedNodeCount": 0,
        "fullMerkleRebuilds": 0,
    }
    packet[field] = value
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(packet, DELTA_SCHEMA)
