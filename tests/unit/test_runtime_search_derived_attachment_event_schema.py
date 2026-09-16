# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-search-derived-attachment-event.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def event(state: str = "ready") -> dict:
    ready = state == "ready"
    failed = state == "failed"
    return {
        "schemaId": "agent.semantic-protocols.runtime-search-derived-attachment-event",
        "schemaVersion": "1",
        "sequence": 3,
        "projectId": "project-a",
        "workspaceId": "workspace-a",
        "generationToken": 2,
        "contentGenerationDigest": "blake3-256:" + "a" * 64,
        "attachment": "tantivy",
        "state": state,
        "buildMicros": 750 if ready else None,
        "finalizeMicros": 40 if ready else None,
        "reasonKind": "build-failed" if failed else None,
    }


def test_ready_attachment_event_is_exact_generation_bound() -> None:
    VALIDATOR.validate(event())


@pytest.mark.parametrize("state", ["queued", "building"])
def test_nonterminal_attachment_event_cannot_claim_build_timing(state: str) -> None:
    value = event(state)
    value["buildMicros"] = 1
    with pytest.raises(ValidationError):
        VALIDATOR.validate(value)


def test_failed_attachment_event_requires_typed_reason() -> None:
    value = event("failed")
    value["reasonKind"] = None
    with pytest.raises(ValidationError):
        VALIDATOR.validate(value)


def test_attachment_event_rejects_unknown_state_and_generation_shape() -> None:
    value = event()
    value["state"] = "complete"
    value["contentGenerationDigest"] = "generation-latest"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(value)


def test_attachment_event_requires_nonzero_generation_token() -> None:
    value = event()
    value["generationToken"] = 0
    with pytest.raises(ValidationError):
        VALIDATOR.validate(value)
