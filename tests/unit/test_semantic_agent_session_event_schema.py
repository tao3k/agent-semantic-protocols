"""Validate semantic agent session event schema."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = json.loads(
    (_ROOT / "schemas" / "semantic-agent-session-event.v1.schema.json").read_text()
)


def test_agent_session_event_accepts_command_result_artifact_ref() -> None:
    event = {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-event",
        "schemaVersion": "1",
        "eventId": "session-0003-command-result",
        "sessionId": "session-1",
        "ordinal": 3,
        "timestampUtc": "2026-06-14T12:00:00Z",
        "kind": "command.result",
        "source": "claude-sdk",
        "toolUseId": "toolu_1",
        "commandId": "command-toolu_1",
        "artifactRefs": [
            {
                "kind": "stdout",
                "path": "outputs/command-toolu_1.stdout",
                "sha256": "0" * 64,
                "bytes": 14,
                "lines": 1,
            }
        ],
        "fields": {
            "command": "asp rust search prime --workspace . --view seeds",
            "argv": ["asp", "rust", "search", "prime", "--view", "seeds", "."],
            "stdoutBytes": 14,
            "denied": False,
        },
    }

    Draft202012Validator(_SCHEMA).validate(event)


def test_agent_session_event_rejects_hidden_reasoning_field() -> None:
    event = {
        "schemaId": "agent.semantic-protocols.semantic-agent-session-event",
        "schemaVersion": "1",
        "eventId": "session-0001-hidden",
        "sessionId": "session-1",
        "ordinal": 1,
        "timestampUtc": "2026-06-14T12:00:00Z",
        "kind": "assistant.visible-message",
        "source": "claude-sdk",
        "hiddenReasoning": "private chain of thought",
    }

    errors = list(Draft202012Validator(_SCHEMA).iter_errors(event))

    assert errors
