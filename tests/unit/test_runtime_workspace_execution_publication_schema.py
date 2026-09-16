import copy
import json
from pathlib import Path

import jsonschema
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"


def load(name: str) -> dict:
    return json.loads((SCHEMAS / name).read_text())


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def runtime_binding() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.runtime-execution-binding",
        "schemaVersion": "2",
        "projectWorkspace": {
            "schemaId": "agent.semantic-protocols.project-workspace-binding",
            "schemaVersion": "1",
            "projectWorkspaceIdentity": "git+https://github.com/tao3k/agent-semantic-protocols.git#workspace/main",
            "workspaceRootPath": ".",
            "portability": "cross-machine",
            "repositoryAliases": [],
        },
        "worktreeInstanceId": "worktree-main",
        "publicationNonce": "publication-1",
        "contentBinding": {
            "schemaId": "asp.content-binding",
            "schemaVersion": "1",
            "runtimeArtifactDigest": digest("1"),
            "workspaceSnapshotDigest": digest("2"),
            "sourceGenerationDigest": digest("3"),
            "sourceIndexDigest": digest("4"),
            "schemaDigest": digest("5"),
            "providerCatalogDigest": digest("6"),
            "authorityStamp": {
                "keyId": "runtime-owner",
                "canonicalDigest": digest("7"),
                "signature": "signature-1",
            },
        },
        "runtimeArtifactDigest": digest("1"),
        "evaluatorPolicyDigest": digest("8"),
        "activeArtifactReceiptDigest": digest("9"),
        "evaluatorAbiDigest": digest("a"),
    }


def content_publication_commit() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.content-publication-commit",
        "schemaVersion": "1",
        "contentBinding": copy.deepcopy(runtime_binding()["contentBinding"]),
        "commitDigest": digest("e"),
        "mutationId": "mutation-e",
        "leaseId": "lease-e",
        "predecessorCommitDigest": None,
    }


def publication() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.runtime-workspace-execution-publication",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-main",
        "generationDigest": digest("b"),
        "sourceRootDigest": digest("c"),
        "contentPublicationCommit": content_publication_commit(),
        "runtimeExecutionBinding": runtime_binding(),
        "runtimeBundleDigest": digest("f"),
        "publicationDigest": digest("d"),
    }


def validator() -> jsonschema.Draft202012Validator:
    return schema_validator_for(
        SCHEMAS / "runtime-workspace-execution-publication.v1.schema.json"
    )


def test_complete_execution_publication_is_schema_valid() -> None:
    validator().validate(publication())


@pytest.mark.parametrize(
    "field",
    [
        "generationDigest",
        "sourceRootDigest",
        "contentPublicationCommit",
        "runtimeExecutionBinding",
        "runtimeBundleDigest",
        "publicationDigest",
    ],
)
def test_execution_publication_rejects_missing_product_fields(field: str) -> None:
    packet = publication()
    packet.pop(field)
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)


def test_execution_publication_rejects_activation_generation_identity() -> None:
    packet = publication()
    packet["activationGeneration"] = 84
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)


def test_execution_publication_rejects_legacy_runtime_binding() -> None:
    packet = publication()
    packet["runtimeExecutionBinding"] = {
        "schemaId": "agent.semantic-protocols.runtime-execution-binding",
        "schemaVersion": "1",
        "projectId": "repo-main",
        "workspaceId": "workspace-main",
    }
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)


def test_execution_publication_rejects_a_digest_only_content_commit_surrogate() -> None:
    packet = publication()
    packet["contentPublicationCommit"] = {"commitDigest": digest("e")}
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(packet)
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
