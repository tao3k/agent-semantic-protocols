# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
            "basis": "git-common-dir:/workspace/.git",
        },
        "workspace": {
            "digest": "blake3-256:" + "b" * 64,
            "repoDigest": "blake3-256:" + "a" * 64,
            "canonicalRoot": "/workspace",
            "privateGitDir": "/workspace/.git",
        },
        "bindingDigest": "blake3-256:" + "c" * 64,
    }
    jsonschema.validate(document, schema("state-home-project-binding.schema.json"))

    document["workspace"]["digest"] = "workspace-truncated"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(document, schema("state-home-project-binding.schema.json"))


def test_project_binding_v1_binds_the_private_git_directory() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-project-binding",
        "schemaVersion": 1,
        "hostProject": None,
        "repo": {
            "digest": "blake3-256:" + "a" * 64,
            "basis": "git-common-dir:/workspace/.git",
        },
        "workspace": {
            "digest": "blake3-256:" + "b" * 64,
            "repoDigest": "blake3-256:" + "a" * 64,
            "canonicalRoot": "/workspace-linked",
            "privateGitDir": "/workspace/.git/worktrees/linked",
        },
        "bindingDigest": "blake3-256:" + "c" * 64,
    }
    jsonschema.validate(document, schema("state-home-project-binding.schema.json"))

    del document["workspace"]["privateGitDir"]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(document, schema("state-home-project-binding.schema.json"))


def test_retention_plan_requires_typed_disposition_and_byte_accounting() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-retention-plan",
        "schemaVersion": 1,
        "evaluatedAtMs": 200,
        "retainForMs": 100,
        "selection": {"kind": "all"},
        "retainedCount": 1,
        "deletedCount": 0,
        "retainedBytes": 42,
        "deletedBytes": 0,
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

    document["entries"][0]["disposition"]["state"] = "retire"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(document, schema("state-home-retention-plan.schema.json"))


def test_cleanup_receipt_has_one_canonical_workspace_authority() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-cleanup-receipt",
        "schemaVersion": 1,
        "state": "applied",
        "retainedForDays": 1,
        "catalogGeneration": 2,
        "canonicalWorkspacesDeleted": 0,
        "catalogPlan": {
            "schemaId": "agent.semantic-protocols.state-home-retention-plan",
            "schemaVersion": 1,
            "evaluatedAtMs": 200,
            "retainForMs": 100,
            "selection": {"kind": "all"},
            "retainedCount": 0,
            "deletedCount": 0,
            "retainedBytes": 0,
            "deletedBytes": 0,
            "entries": [],
        },
    }
    jsonschema.validate(document, schema("state-home-cleanup-receipt.schema.json"))


def test_state_home_sync_receipt_is_positive_convergence_not_retirement_history() -> None:
    document = {
        "schemaId": "agent.semantic-protocols.state-home-sync-receipt",
        "schemaVersion": 1,
        "state": "converged",
        "stateHome": "/state-home",
        "removedEntries": ["obsolete-root", "trash/contract-convergence"],
        "removedBytes": 42,
    }
    jsonschema.validate(document, schema("state-home-sync-receipt.schema.json"))

    document["state"] = "retired"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(document, schema("state-home-sync-receipt.schema.json"))
