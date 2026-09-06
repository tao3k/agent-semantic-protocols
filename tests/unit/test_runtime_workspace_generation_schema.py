from __future__ import annotations

import json
from pathlib import Path

import jsonschema
import pytest
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "runtime-server-control.v1.schema.json"
EXECUTION_BINDING_SCHEMA_PATH = (
    ROOT / "schemas" / "runtime-provider-execution-binding.v1.schema.json"
)


def workspace_generation_schema() -> dict[str, object]:
    document = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    return document["$defs"]["workspaceGenerationSnapshot"]


def workspace_generation_validator() -> jsonschema.Draft202012Validator:
    execution_binding = json.loads(
        EXECUTION_BINDING_SCHEMA_PATH.read_text(encoding="utf-8")
    )
    registry = Registry().with_resource(
        execution_binding["$id"], Resource.from_contents(execution_binding)
    )
    registry = registry.with_resource(
        "runtime-provider-execution-binding.v1.schema.json",
        Resource.from_contents(execution_binding),
    )
    return jsonschema.Draft202012Validator(
        workspace_generation_schema(), registry=registry
    )


def digest(seed: str) -> str:
    return f"blake3-256:{seed * 64}"


def complete_generation_receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-server-workspace-generation-snapshot.v2",
        "schemaVersion": "2",
        "workspaceIdentity": "workspace-fixture",
        "state": "ready",
        "activeEpoch": 1,
        "generationDigest": digest("1"),
        "rootDepth": [1, 0],
        "sourceKind": "filesystem",
        "leafCount": 1,
        "ownerCount": 1,
        "providerSchemaDigest": digest("2"),
        "runtimeProviderExecutionBinding": {
            "schemaId": "agent.semantic-protocols.runtime-provider-execution-binding",
            "schemaVersion": "1",
            "generation": digest("c"),
            "projectId": "repo-fixture",
            "workspaceId": "workspace-fixture",
            "runtimeBundleDigest": digest("d"),
            "schemaBundleDigest": digest("e"),
            "workspaceClosureDigest": digest("f"),
            "sourceSnapshotDigest": digest("3"),
            "sourceIndexDigest": digest("7"),
        },
        "sourceRootDigest": digest("3"),
        "baseRootDigest": digest("4"),
        "sourceProviderDigest": digest("5"),
        "dirtyPathsDigest": digest("6"),
        "moduleGraphDigest": digest("7"),
        "selectorSetDigest": digest("8"),
        "memoryBackendDigest": digest("9"),
        "workspaceSourceScopeGeneration": digest("a"),
        "durableCommitDigest": digest("b"),
        "mmapSegmentPath": "/runtime/generation-1.mmap",
        "previousEpochReadable": True,
    }


def test_workspace_generation_receipt_is_a_complete_atomic_contract() -> None:
    schema = workspace_generation_schema()
    assert set(schema["required"]) == set(schema["properties"])
    workspace_generation_validator().validate(complete_generation_receipt())


@pytest.mark.parametrize(
    "missing",
    [
        "providerSchemaDigest",
        "runtimeProviderExecutionBinding",
        "sourceRootDigest",
        "moduleGraphDigest",
        "selectorSetDigest",
        "memoryBackendDigest",
        "workspaceSourceScopeGeneration",
        "durableCommitDigest",
    ],
)
def test_workspace_generation_receipt_rejects_torn_publication(missing: str) -> None:
    receipt = complete_generation_receipt()
    del receipt[missing]
    with pytest.raises(jsonschema.ValidationError):
        workspace_generation_validator().validate(receipt)


def test_workspace_generation_receipt_requires_layered_root_depth() -> None:
    receipt = complete_generation_receipt()
    receipt["rootDepth"] = [0, 0]
    with pytest.raises(jsonschema.ValidationError):
        workspace_generation_validator().validate(receipt)
