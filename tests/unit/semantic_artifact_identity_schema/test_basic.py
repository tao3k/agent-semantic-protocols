import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[3]
SCHEMA_PATH = ROOT / "schemas" / "semantic-artifact-identity.v1.schema.json"
EDGE_SCHEMA_PATH = ROOT / "schemas" / "semantic-artifact-edge.v1.schema.json"
REPAIR_CHAIN_SCHEMA_PATH = (
    ROOT / "schemas" / "semantic-artifact-repair-chain-frame.v1.schema.json"
)


def load_schema():
    return json.loads(SCHEMA_PATH.read_text())


def load_repair_chain_schema():
    return json.loads(REPAIR_CHAIN_SCHEMA_PATH.read_text())


def load_edge_schema():
    return json.loads(EDGE_SCHEMA_PATH.read_text())


def artifact_hash(value=None):
    return {
        "algorithm": "blake3",
        "value": value or "a" * 64,
    }


def test_artifact_identity_schema_accepts_root_reference():
    packet = {
        "schemaId": "semantic-artifact-identity",
        "schemaVersion": "1",
        "hashAlgorithm": "blake3",
        "roots": [
            {
                "repoId": "repo_123",
                "workspaceId": "workspace_456",
                "scopeId": "default",
                "generation": "g1",
                "rootKind": "sourceSnapshot",
                "rootHash": artifact_hash("b" * 64),
                "nodeHash": artifact_hash("c" * 64),
                "producerHash": artifact_hash("d" * 64),
                "schemaHash": artifact_hash("e" * 64),
                "contentHash": artifact_hash("f" * 64),
            }
        ],
    }

    jsonschema.validate(packet, load_schema())


def test_artifact_identity_schema_accepts_repair_chain_root_kinds():
    roots = []
    for index, root_kind in enumerate(
        ["howFromFrame", "howFixFrame", "changeSet", "proofReceipt", "graphDiff"]
    ):
        roots.append(
            {
                "repoId": "repo_123",
                "workspaceId": "workspace_456",
                "scopeId": "default",
                "generation": f"g{index}",
                "rootKind": root_kind,
                "rootHash": artifact_hash(f"{index + 1}" * 64),
                "nodeHash": artifact_hash(f"{index + 2}" * 64),
            }
        )
    packet = {
        "schemaId": "semantic-artifact-identity",
        "schemaVersion": "1",
        "hashAlgorithm": "blake3",
        "roots": roots,
    }

    jsonschema.validate(packet, load_schema())


def test_repair_chain_frame_schema_accepts_how_fix_with_how_from_parent():
    how_from_root = {
        "repoId": "repo_123",
        "workspaceId": "workspace_456",
        "scopeId": "default",
        "generation": "how-from-1",
        "rootKind": "howFromFrame",
        "rootHash": artifact_hash("a" * 64),
        "nodeHash": artifact_hash("b" * 64),
    }
    how_fix_root = {
        "repoId": "repo_123",
        "workspaceId": "workspace_456",
        "scopeId": "default",
        "generation": "how-fix-1",
        "rootKind": "howFixFrame",
        "rootHash": artifact_hash("c" * 64),
        "nodeHash": artifact_hash("d" * 64),
        "contentHash": artifact_hash("e" * 64),
    }
    frame = {
        "frameKind": "howFixFrame",
        "root": how_fix_root,
        "contentHash": artifact_hash("e" * 64),
        "parents": [
            {
                "role": "howFrom",
                "ordinal": 0,
                "root": how_from_root,
            }
        ],
    }

    jsonschema.validate(frame, load_repair_chain_schema())


def test_artifact_edge_schema_accepts_proof_to_change_set_edge():
    proof_root = {
        "repoId": "repo_123",
        "workspaceId": "workspace_456",
        "scopeId": "default",
        "generation": "proof-1",
        "rootKind": "proofReceipt",
        "rootHash": artifact_hash("a" * 64),
        "nodeHash": artifact_hash("b" * 64),
    }
    change_set_root = {
        "repoId": "repo_123",
        "workspaceId": "workspace_456",
        "scopeId": "default",
        "generation": "change-set-1",
        "rootKind": "changeSet",
        "rootHash": artifact_hash("c" * 64),
        "nodeHash": artifact_hash("d" * 64),
    }
    edge = {
        "schemaId": "semantic-artifact-edge",
        "schemaVersion": "1",
        "edgeHash": artifact_hash("e" * 64),
        "role": "changeSet",
        "ordinal": 0,
        "parent": proof_root,
        "child": change_set_root,
    }

    jsonschema.validate(edge, load_edge_schema())


def test_artifact_identity_schema_rejects_short_hash():
    packet = {
        "schemaId": "semantic-artifact-identity",
        "schemaVersion": "1",
        "hashAlgorithm": "blake3",
        "roots": [
            {
                "repoId": "repo_123",
                "workspaceId": "workspace_456",
                "scopeId": "default",
                "generation": "g1",
                "rootKind": "sourceSnapshot",
                "rootHash": artifact_hash("abc"),
                "nodeHash": artifact_hash("c" * 64),
            }
        ],
    }

    try:
        jsonschema.validate(packet, load_schema())
    except jsonschema.ValidationError:
        return
    raise AssertionError("short artifact hash should fail schema validation")
