# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shared agent-status admission for Codex collaboration schemas."""

from __future__ import annotations

from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


SCHEMA_ROOT = Path(__file__).resolve().parents[2] / "schemas"


@pytest.mark.parametrize(
    ("schema_name", "payload"),
    [
        (
            "codex-collaboration-interrupt-result.v1.schema.json",
            {"previous_status": {"errored": "worker failed"}},
        ),
        (
            "codex-collaboration-live-agents.v1.schema.json",
            {
                "agents": [
                    {"agent_name": "/root/explore", "agent_status": "running"},
                    {
                        "agent_name": "/root/test",
                        "agent_status": {"completed": "green"},
                    },
                ]
            },
        ),
    ],
)
def test_collaboration_schemas_resolve_shared_agent_status(
    schema_name: str, payload: dict[str, object]
) -> None:
    validator = schema_validator_for(SCHEMA_ROOT / schema_name)
    Draft202012Validator.check_schema(validator.schema)
    validator.validate(payload)


@pytest.mark.parametrize(
    ("schema_name", "payload"),
    [
        (
            "codex-collaboration-interrupt-result.v1.schema.json",
            {"previous_status": "unknown"},
        ),
        (
            "codex-collaboration-live-agents.v1.schema.json",
            {"agents": [{"agent_name": "/root/test", "agent_status": "unknown"}]},
        ),
    ],
)
def test_collaboration_schemas_reject_unknown_agent_status(
    schema_name: str, payload: dict[str, object]
) -> None:
    validator = schema_validator_for(SCHEMA_ROOT / schema_name)
    with pytest.raises(ValidationError):
        validator.validate(payload)
