import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]


def schema(name: str) -> object:
    return json.loads((ROOT / "schemas" / name).read_text())


def test_project_binding_keeps_host_repo_and_workspace_axes_distinct() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-project-binding",
        "schemaVersion": 1,
        "hostProject": {
            "platform": "codex",
            "projectId": "local-093afd45fd0bf21aef12ffbe8c6ff1db",
            "projectKind": "local",
            "hostId": "local",
        },
        "repo": {
            "digest": "blake3-256:" + "a" * 64,
            "basis": "git-remote:example/repo",
        },
        "workspace": {
            "digest": "blake3-256:" + "b" * 64,
            "repoDigest": "blake3-256:" + "a" * 64,
            "canonicalRoot": "/workspace",
        },
        "bindingDigest": "blake3-256:" + "c" * 64,
    }
    jsonschema.validate(document, schema("state-home-project-binding.schema.json"))

    document["workspace"]["digest"] = "workspace-truncated"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(document, schema("state-home-project-binding.schema.json"))


def test_retention_plan_requires_typed_disposition_and_byte_accounting() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-retention-plan",
        "schemaVersion": 1,
        "evaluatedAtMs": 200,
        "retainForMs": 100,
        "retainedCount": 1,
        "retiredCount": 0,
        "retainedBytes": 42,
        "retiredBytes": 0,
        "entries": [
            {
                "object": {
                    "objectId": "workspace:abc",
                    "kind": "workspace",
                    "lastObservedAtMs": 150,
                    "byteCount": 42,
                },
                "disposition": {"state": "keep", "reason": "active-lease"},
            }
        ],
    }
    jsonschema.validate(document, schema("state-home-retention-plan.schema.json"))

    document["entries"][0]["disposition"]["state"] = "delete"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(document, schema("state-home-retention-plan.schema.json"))


def test_cleanup_receipt_joins_workspace_and_project_retirement() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-cleanup-receipt",
        "schemaVersion": 1,
        "state": "applied",
        "retainedForDays": 1,
        "catalogGeneration": 2,
        "catalogPlan": {
            "schemaId": "agent.semantic-protocols.state-home-retention-plan",
            "schemaVersion": 1,
            "evaluatedAtMs": 200,
            "retainForMs": 100,
            "retainedCount": 0,
            "retiredCount": 0,
            "retainedBytes": 0,
            "retiredBytes": 0,
            "entries": [],
        },
        "temporaryWorkspaceReport": {"retiredCount": 2},
        "projectRegistryReport": {"removedCount": 84},
    }
    jsonschema.validate(document, schema("state-home-cleanup-receipt.schema.json"))
