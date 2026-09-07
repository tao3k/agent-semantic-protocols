# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = ROOT / "schemas"


def load_schema(name: str) -> dict:
    return json.loads((SCHEMA_ROOT / name).read_text())


CONTROL = load_schema("workspace-project-resolution-control.v1.schema.json")
RECEIPT = load_schema("workspace-project-resolution-receipt.v1.schema.json")
CONTROL_VALIDATOR = Draft202012Validator(CONTROL)
RECEIPT_VALIDATOR = Draft202012Validator(RECEIPT)


def workspace_identity() -> dict:
    return {
        "repositoryIdentity": "repo-1",
        "worktreeIdentity": "worktree-1",
        "workspaceRootDigest": "blake3:workspace",
    }


def control(operation: dict) -> dict:
    return {
        "schemaId": "agent.semantic-protocols.workspace-project-resolution-control",
        "schemaVersion": "1",
        "workspaceIdentity": workspace_identity(),
        "operation": operation,
    }


def test_many_sessions_attach_to_one_workspace_identity() -> None:
    for session in ("session-a", "session-b"):
        CONTROL_VALIDATOR.validate(
            control({"kind": "attach-session", "sessionIdentity": session})
        )


def test_refresh_is_driven_only_by_candidate_and_package_manager_inputs() -> None:
    request = control(
        {
            "kind": "refresh-inputs",
            "candidateGeneration": "git-index:42",
            "projectEntries": [
                {
                    "providerId": "rust",
                    "path": "Cargo.toml",
                    "contentDigest": "blake3:manifest",
                }
            ],
            "packageManagerInputs": [
                {
                    "path": "Cargo.lock",
                    "contentDigest": "blake3:lock",
                }
            ],
        }
    )
    CONTROL_VALIDATOR.validate(request)
    serialized = json.dumps(request)
    assert "defaultSourceRoots" not in serialized
    assert "defaultIgnoredPathPrefixes" not in serialized


@pytest.mark.parametrize(
    "forbidden",
    ("defaultSourceRoots", "defaultIgnoredPathPrefixes", "sourceRoots"),
)
def test_refresh_rejects_caller_supplied_scope_defaults(forbidden: str) -> None:
    operation = {
        "kind": "refresh-inputs",
        "candidateGeneration": "git-index:42",
        "projectEntries": [],
        "packageManagerInputs": [],
        forbidden: [],
    }
    assert list(CONTROL_VALIDATOR.iter_errors(control(operation)))


def test_serving_receipt_publishes_one_atomic_generation() -> None:
    RECEIPT_VALIDATOR.validate(
        {
            "schemaId": "agent.semantic-protocols.workspace-project-resolution-receipt",
            "schemaVersion": "1",
            "workspaceIdentity": workspace_identity(),
            "state": "generation-serving",
            "attachedSessionCount": 2,
            "generation": {
                "generationId": 7,
                "candidateGeneration": "git-index:42",
                "packageGraphDigest": "blake3:package-graph",
                "resolvedSourceScopeDigest": "blake3:scope",
                "projectResolutionArtifact": "project-resolution/generation-7.json",
                "resolvedSourceScopeArtifact": "resolved-source-scope/generation-7.json",
            },
        }
    )


def test_failed_receipt_is_typed_and_actionable() -> None:
    RECEIPT_VALIDATOR.validate(
        {
            "schemaId": "agent.semantic-protocols.workspace-project-resolution-receipt",
            "schemaVersion": "1",
            "workspaceIdentity": workspace_identity(),
            "state": "failed",
            "attachedSessionCount": 1,
            "failure": {
                "reasonKind": "project-entry-parse-failed",
                "next": "repair the changed package-manager entry and retry refresh-inputs",
            },
        }
    )
