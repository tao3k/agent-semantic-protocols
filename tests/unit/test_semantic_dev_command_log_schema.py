"""Validate the semantic dev command log event schema."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA = json.loads(
    (_ROOT / "schemas" / "semantic-dev-command-log.v1.schema.json").read_text()
)


def minimal_event() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.dev-command-log",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "timestampUtc": "2026-06-02T10:20:31Z",
        "startedAtUtc": "2026-06-02T10:20:30Z",
        "finishedAtUtc": "2026-06-02T10:20:31Z",
        "eventId": "rs-harness-1",
        "sessionId": "session-1",
        "sessionOrdinal": 1,
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "argv": ["rs-harness", "search", "fzf", "metadata", "."],
        "cwd": "/repo",
        "projectRoot": "/repo",
        "projectRootHash": "0123456789abcdef",
        "command": {
            "namespace": "search",
            "method": "search/fzf",
            "view": "fzf",
            "query": "metadata",
            "querySetCount": 0,
            "pipes": ["fzf"],
        },
        "result": {
            "exitCode": 0,
            "elapsedMs": 12,
            "stdoutBytes": 0,
            "stderrBytes": 0,
            "status": "success",
        },
    }


def test_semantic_dev_command_log_accepts_minimal_event() -> None:
    Draft202012Validator(_SCHEMA).validate(minimal_event())


def test_semantic_dev_command_log_rejects_stdout_content() -> None:
    event = minimal_event()
    event["stdout"] = "source content"

    errors = list(Draft202012Validator(_SCHEMA).iter_errors(event))

    assert errors
