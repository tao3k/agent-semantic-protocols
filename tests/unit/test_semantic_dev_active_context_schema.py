"""Validate the semantic dev active-context marker schema."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = json.loads(
    (_ROOT / "schemas" / "semantic-dev-active-context.v1.schema.json").read_text()
)


def minimal_marker() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.dev-active-context",
        "schemaVersion": "1",
        "writtenAtUtc": "2026-06-02T10:20:30Z",
        "ttlSeconds": 1800,
        "projectRoot": "/repo",
        "projectRootHash": "0123456789abcdef",
        "platform": "codex",
        "event": "pre-tool",
        "decision": "deny",
        "sessionId": "session-1",
        "parentEventId": "hook-parent-1",
        "hookRunId": "hook-run-1",
    }


def test_semantic_dev_active_context_accepts_minimal_marker() -> None:
    Draft202012Validator(_SCHEMA).validate(minimal_marker())


def test_semantic_dev_active_context_rejects_uncontracted_payload() -> None:
    marker = minimal_marker()
    marker["payload"] = {"source": "private prompt or command payload"}

    errors = list(Draft202012Validator(_SCHEMA).iter_errors(marker))

    assert errors
