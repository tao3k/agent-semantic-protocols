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


@pytest.mark.parametrize(
    "value",
    [
        {
            "kind": "checkout-root",
            "workspaceRoot": "/checkout/project",
        },
        {
            "kind": "workspace-id",
            "workspaceId": "workspace-5b62c3d2cf8c7296",
        },
    ],
)
def test_workspace_reference_accepts_exactly_one_typed_variant(value):
    jsonschema.validate(value, SCHEMA)


@pytest.mark.parametrize(
    "value",
    [
        {},
        {"kind": "checkout-root", "workspaceRoot": ""},
        {"kind": "checkout-root", "workspaceRoot": "relative/project"},
        {"kind": "workspace-id", "workspaceId": ""},
        {"kind": "workspace-id", "workspaceId": "/checkout/project"},
        {
            "kind": "checkout-root",
            "workspaceRoot": "/checkout/project",
            "workspaceId": "workspace-5b62c3d2cf8c7296",
        },
        {
            "kind": "workspace-id",
            "workspaceId": "workspace-5b62c3d2cf8c7296",
            "workspaceRoot": "/checkout/project",
        },
    ],
)
def test_workspace_reference_rejects_empty_ambiguous_or_reinterpreted_text(value):
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(value, SCHEMA)
