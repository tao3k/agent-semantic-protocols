# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema
import pytest
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[2]
LEAF_SCHEMA_PATH = ROOT / "schemas/agent-session-lifecycle-projection.v1.schema.json"
CONTROL_PLANE_SCHEMA_PATH = ROOT / "schemas/multi-agent-lifecycle-projection.v1.schema.json"
FIXTURES = ROOT / "schemas/fixtures/multi-agent-lifecycle-projection"


def load_json(path: Path) -> object:
    return json.loads(path.read_text())


def validator() -> jsonschema.Draft202012Validator:
    leaf_schema = load_json(LEAF_SCHEMA_PATH)
    control_plane_schema = load_json(CONTROL_PLANE_SCHEMA_PATH)
    registry = Registry().with_resources(
        [
            (
                "https://agent-semantic-protocols.dev/schemas/agent-session-lifecycle-projection.v1.schema.json",
                Resource.from_contents(leaf_schema),
            ),
            (
                "https://agent-semantic-protocols.dev/schemas/multi-agent-lifecycle-projection.v1.schema.json",
                Resource.from_contents(control_plane_schema),
            ),
        ]
    )
    return jsonschema.Draft202012Validator(control_plane_schema, registry=registry)


def test_codex_control_plane_fixture_matches_schema() -> None:
    validator().validate(load_json(FIXTURES / "valid-codex-control-plane.v1.json"))


def test_delivered_delegation_requires_codex_receipt() -> None:
    with pytest.raises(jsonschema.ValidationError):
        validator().validate(
            load_json(FIXTURES / "invalid-delivered-without-receipt.v1.json")
        )
