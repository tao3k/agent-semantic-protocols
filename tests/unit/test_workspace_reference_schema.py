# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
import pytest


SCHEMA = json.loads(
    (
        Path(__file__).parents[2]
        / "schemas"
        / "workspace-reference.v1.schema.json"
    ).read_text()
)


def test_workspace_reference_accepts_project_and_workspace_identity_pair() -> None:
    jsonschema.validate(
        {
            "projectId": "repo-5b62c3d2cf8c7296",
            "workspaceId": "workspace-5b62c3d2cf8c7296",
        },
        SCHEMA,
    )


@pytest.mark.parametrize(
    "value",
    [
        {},
        {"projectId": "repo-5b62c3d2cf8c7296"},
        {"workspaceId": "workspace-5b62c3d2cf8c7296"},
        {"projectId": "", "workspaceId": "workspace-5b62c3d2cf8c7296"},
        {"projectId": "repo-5b62c3d2cf8c7296", "workspaceId": ""},
        {
            "projectId": "repo-5b62c3d2cf8c7296",
            "workspaceId": "workspace-5b62c3d2cf8c7296",
            "workspaceRoot": "/checkout/project",
        },
    ],
)
def test_workspace_reference_rejects_partial_empty_or_path_bearing_values(value) -> None:
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(value, SCHEMA)
