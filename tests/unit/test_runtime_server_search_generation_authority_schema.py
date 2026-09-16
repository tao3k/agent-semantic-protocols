# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = ROOT / "schemas"
SCHEMA_NAMES = (
    "semantic-search-definitions.v1.schema.json",
    "runtime-server-definitions.v1.schema.json",
    "source-snapshot-evidence.v1.schema.json",
    "workspace-generation-root-evidence.v1.schema.json",
    "runtime-provider-execution-binding.v1.schema.json",
    "content-search-generation-receipt.v1.schema.json",
    "runtime-server-search-generation-authority.v1.schema.json",
    "runtime-server-search-generation-authority-open-receipt.v1.schema.json",
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


def authority() -> dict[str, object]:
    digest = "a" * 64
    wire_digest = f"blake3-256:{digest}"
    identity = {
        "projectId": "repo-schema-fixture",
        "workspaceId": "workspace-schema-fixture",
        "sourceRootDigest": wire_digest,
        "providerDigest": wire_digest,
        "schemaDigest": wire_digest,
        "generationCandidateDigest": wire_digest,
    }
    def stage(name: str) -> dict[str, object]:
        return {
            "stage": name,
            "identity": identity,
            "artifactDigest": wire_digest,
            "workerId": f"fixture-{name}",
            "complete": True,
        }
    return {
        "schemaId": "agent.semantic-protocols.runtime-server-search-generation-authority",
        "schemaVersion": "1",
        "projectId": "repo-schema-fixture",
        "workspaceId": "workspace-schema-fixture",
        "activeEpoch": 1,
        "generationDigest": wire_digest,
        "ownerMerkleRootDigest": f"blake3-256:{'b' * 64}",
        "searchProjectionManifestDigest": f"blake3-256:{'c' * 64}",
        "searchProjectionAnalyzerDigest": f"blake3-256:{'d' * 64}",
        "providerSchemaDigest": f"blake3-256:{'e' * 64}",
        "runtimeProviderExecutionBinding": {
            "schemaId": "agent.semantic-protocols.runtime-provider-execution-binding",
            "schemaVersion": "1",
            "generation": wire_digest,
            "projectId": "repo-schema-fixture",
            "workspaceId": "workspace-schema-fixture",
            "runtimeBundleDigest": wire_digest,
            "schemaBundleDigest": wire_digest,
            "workspaceClosureDigest": wire_digest,
            "sourceSnapshotDigest": wire_digest,
            "sourceIndexDigest": wire_digest,
        },
        "contentSearchGeneration": {
            "schemaId": "agent.semantic-protocols.content-search-generation-receipt",
            "schemaVersion": "1",
            "acquisition": stage("source-byte-acquisition"),
            "contentGenerationDigest": wire_digest,
        },
        "sourceSnapshot": {
            "schemaId": "asp.source-snapshot.v1",
            "algorithm": "blake3-merkle-v1",
            "rootDigest": digest,
            "sourceKind": "filesystem",
            "leafCount": 1,
            "providerDigest": digest,
        },
        "projectResolutions": [],
        "workspaceGeneration": {
            "rootDigest": digest,
            "rootDepth": 1,
            "leafCount": 1,
            "ownerCount": 1,
        },
    }


def test_compact_authority_and_pointer_open_receipt_resolve_all_refs() -> None:
    registry = schema_registry()
    authority_schema = load_schema(
        "runtime-server-search-generation-authority.v1.schema.json"
    )
    open_receipt_schema = load_schema(
        "runtime-server-search-generation-authority-open-receipt.v1.schema.json"
    )
    packet = authority()

    Draft202012Validator(authority_schema, registry=registry).validate(packet)
    Draft202012Validator(open_receipt_schema, registry=registry).validate(
        {
            "schemaId": "agent.semantic-protocols.runtime-server-search-generation-authority-open-receipt",
            "schemaVersion": "1",
            "authority": packet,
            "generationPointerPath": "/runtime/workspaces/scope/active-generation.pointer",
        }
    )


def test_authority_rejects_complete_workspace_snapshot_payload() -> None:
    packet = authority()
    packet["workspaceSnapshot"] = {"leaves": {"src/lib.rs": "source bytes"}}
    validator = Draft202012Validator(
        load_schema("runtime-server-search-generation-authority.v1.schema.json"),
        registry=schema_registry(),
    )

    with pytest.raises(ValidationError):
        validator.validate(packet)


def test_authority_rejects_project_root_as_identity() -> None:
    packet = authority()
    packet["projectRoot"] = "/workspace"
    validator = Draft202012Validator(
        load_schema("runtime-server-search-generation-authority.v1.schema.json"),
        registry=schema_registry(),
    )

    with pytest.raises(ValidationError):
        validator.validate(packet)


def test_content_generation_rejects_workspace_without_project_identity() -> None:
    packet = authority()
    del packet["contentSearchGeneration"]["acquisition"]["identity"]["projectId"]
    validator = Draft202012Validator(
        load_schema("runtime-server-search-generation-authority.v1.schema.json"),
        registry=schema_registry(),
    )

    with pytest.raises(ValidationError):
        validator.validate(packet)
