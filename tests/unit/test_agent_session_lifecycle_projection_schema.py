# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/agent-session-lifecycle-projection.v1.schema.json").read_text()
)
FIXTURES = ROOT / "schemas/fixtures/agent-session-lifecycle-projection"


def load_fixture(name: str) -> object:
    return json.loads((FIXTURES / name).read_text())


def test_ready_projection_matches_schema() -> None:
    jsonschema.validate(load_fixture("valid-ready.v1.json"), SCHEMA)


def test_unobserved_projection_matches_schema() -> None:
    jsonschema.validate(load_fixture("valid-unobserved.v1.json"), SCHEMA)


def test_present_path_cannot_select_spawn_agent() -> None:
    with __import__("pytest").raises(jsonschema.ValidationError):
        jsonschema.validate(
            load_fixture("invalid-present-path-spawn-action.v1.json"), SCHEMA
        )


def test_observed_session_requires_generation() -> None:
    with __import__("pytest").raises(jsonschema.ValidationError):
        jsonschema.validate(
            load_fixture("invalid-observed-session-without-generation.v1.json"), SCHEMA
        )
