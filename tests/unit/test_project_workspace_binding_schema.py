# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Durable project-workspace identity and local worktree separation."""

from copy import deepcopy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/project-workspace-binding.v1.schema.json").read_text()
)
VALID = json.loads(
    (
        ROOT
        / "schemas/fixtures/project-workspace-binding/valid-cross-machine.v1.json"
    ).read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def test_manifest_declared_git_workspace_is_cross_machine_stable():
    VALIDATOR.validate(VALID)


@pytest.mark.parametrize("workspace_root", ["/tmp/repo", "../repo", "src/../docs"])
def test_workspace_root_is_a_repository_relative_containment_boundary(workspace_root):
    packet = deepcopy(VALID)
    packet["workspaceRootPath"] = workspace_root
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


@pytest.mark.parametrize(
    "identity",
    [
        "workspace-23cc5ba784c605ae",
        "git+https://github.com/tao3k/agent-semantic-protocols.git#branch/main",
        "git+https://github.com/tao3k/agent-semantic-protocols.git#worktree/feature",
    ],
)
def test_runtime_ids_branches_and_worktrees_cannot_impersonate_a_workspace(identity):
    packet = deepcopy(VALID)
    packet["projectWorkspaceIdentity"] = identity
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_file_locator_cannot_claim_cross_machine_portability():
    packet = deepcopy(VALID)
    packet["projectWorkspaceIdentity"] = (
        "git+file:///tmp/agent-semantic-protocols.git#workspace/root"
    )
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)


def test_local_project_identity_is_explicitly_local_only():
    packet = deepcopy(VALID)
    packet.update(
        {
            "projectWorkspaceIdentity": (
                "git+file:///tmp/agent-semantic-protocols.git#workspace/root"
            ),
            "portability": "local-only",
        }
    )
    VALIDATOR.validate(packet)


@pytest.mark.parametrize("field", ["checkoutRootPath", "branch", "worktreeInstanceId"])
def test_host_local_worktree_state_is_not_part_of_the_durable_binding(field):
    packet = deepcopy(VALID)
    packet[field] = "host-local"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(packet)
