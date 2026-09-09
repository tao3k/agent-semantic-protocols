# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = ROOT / "schemas"
SCHEMA_NAMES = (
    "semantic-search-definitions.v1.schema.json",
    "source-snapshot-evidence.v1.schema.json",
    "workspace-generation-root-evidence.v1.schema.json",
    "search-generation-graph-request.v1.schema.json",
    "search-generation-graph-receipt.v1.schema.json",
)


def load_schema(name: str) -> dict[str, object]:
    return json.loads((SCHEMA_ROOT / name).read_text())


def schema_registry() -> Registry:
    resources = []
    for name in SCHEMA_NAMES:
        schema = load_schema(name)
        Draft202012Validator.check_schema(schema)
        resources.append((schema["$id"], Resource.from_contents(schema)))
    return Registry().with_resources(resources)


def request() -> dict[str, object]:
    digest = "a" * 64
    return {
        "schemaId": "agent.semantic-protocols.search-generation-graph-request",
        "schemaVersion": "1",
        "contentGenerationDigest": f"blake3-256:{'e' * 64}",
        "identity": {
            "projectId": "project-test",
            "workspaceId": "workspace-test",
            "sourceRootDigest": f"blake3-256:{digest}",
            "providerDigest": f"blake3-256:{'b' * 64}",
            "schemaDigest": f"blake3-256:{'c' * 64}",
            "generationCandidateDigest": f"blake3-256:{'d' * 64}",
        },
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": digest,
            "sourceKind": "filesystem",
            "leafCount": 1,
            "providerDigest": "b" * 64,
        },
        "workspaceGeneration": {
            "rootDigest": digest,
            "rootDepth": 1,
            "leafCount": 1,
            "ownerCount": 1,
        },
        "ownerPaths": ["src/lib.rs"],
        "relations": [],
    }


def test_generation_graph_request_resolves_shared_identity_evidence() -> None:
    Draft202012Validator(
        load_schema("search-generation-graph-request.v1.schema.json"),
        registry=schema_registry(),
    ).validate(request())


@pytest.mark.parametrize("field", ["sourceSnapshot", "workspaceGeneration"])
def test_generation_graph_request_rejects_partial_identity_evidence(field: str) -> None:
    payload = request()
    payload[field] = {"rootDigest": "a" * 64}
    with pytest.raises(ValidationError):
        Draft202012Validator(
            load_schema("search-generation-graph-request.v1.schema.json"),
            registry=schema_registry(),
        ).validate(payload)


def test_generation_graph_request_rejects_empty_owner_frontier() -> None:
    payload = request()
    payload["ownerPaths"] = []
    with pytest.raises(ValidationError):
        Draft202012Validator(
            load_schema("search-generation-graph-request.v1.schema.json"),
            registry=schema_registry(),
        ).validate(payload)


def test_generation_graph_receipt_rejects_legacy_or_empty_frontier() -> None:
    payload = {
        "schemaId": "agent.semantic-protocols.search-generation-graph-receipt",
        "schemaVersion": "1",
        "identity": request()["identity"],
        "contentGenerationDigest": request()["contentGenerationDigest"],
        "entryOwnerIds": ["src/lib.rs"],
        "entryNodeIds": ["owner:src/lib.rs"],
        "candidateOwnerIds": ["src/lib.rs"],
        "artifactDigest": f"blake3-256:{'e' * 64}",
        "complete": True,
    }
    validator = Draft202012Validator(
        load_schema("search-generation-graph-receipt.v1.schema.json"),
        registry=schema_registry(),
    )
    validator.validate(payload)
    empty = copy.deepcopy(payload)
    empty["entryNodeIds"] = []
    with pytest.raises(ValidationError):
        validator.validate(empty)
    legacy = copy.deepcopy(payload)
    legacy["seedIds"] = legacy.pop("entryNodeIds")
    with pytest.raises(ValidationError):
        validator.validate(legacy)

    partial_identity = copy.deepcopy(payload)
    partial_identity["identity"].pop("providerDigest")
    with pytest.raises(ValidationError):
        validator.validate(partial_identity)
