"""Validate dev command log session summaries."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "packages" / "python" / "src"))

# `tools` lives under the repository-local Python source root inserted above.
from tools.dev_command_log_analyzer import load_command_events, render_summary  # noqa: E402


def test_dev_command_log_analyzer_sorts_by_session_ordinal(tmp_path: Path) -> None:
    command_dir = tmp_path / "rust" / "rs-harness" / "commands"
    command_dir.mkdir(parents=True)
    (command_dir / "2026-06-02T10-20-31Z-000002-b.jsonl").write_text(
        json.dumps(_event(2, "search/lexical", "metadata")) + "\n",
        encoding="utf-8",
    )
    (command_dir / "2026-06-02T10-20-30Z-000001-a.jsonl").write_text(
        json.dumps(_event(1, "agent/guide", None)) + "\n",
        encoding="utf-8",
    )
    fallback_dir = tmp_path / "python" / "py-harness" / "commands"
    fallback_dir.mkdir(parents=True)
    (fallback_dir / "2026-06-02T10-20-32Z-000001-c.jsonl").write_text(
        json.dumps(_event(1, "search/lexical", "fallback", session_id="project-x", context="project-fallback"))
        + "\n",
        encoding="utf-8",
    )

    summary = render_summary(load_command_events(tmp_path))

    assert "[dev-log-summary] sessions=2 commands=3 activeContext=2 projectFallback=1" in summary
    session_summary = summary.split('|session id="session-1"', 1)[1]
    assert session_summary.index("method=agent/guide") < session_summary.index(
        "method=search/lexical"
    )
    assert 'id="session-1" commands=2' in summary
    assert "rootHash=0123456789abcdef" in session_summary
    assert "context=project-fallback rootHash=0123456789abcdef" in summary
    assert 'parent="hook-parent-1"' in summary
    assert 'hook="hook-run-1"' in summary
    assert 'query="metadata"' in summary


def _event(
    ordinal: int,
    method: str,
    query: str | None,
    *,
    session_id: str = "session-1",
    context: str = "active-context",
) -> dict[str, object]:
    command: dict[str, object] = {
        "namespace": method.split("/", 1)[0],
        "method": method,
        "pipes": [],
        "querySetCount": 0,
    }
    if query is not None:
        command["query"] = query
    return {
        "schemaId": "agent.semantic-protocols.dev-command-log",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "timestampUtc": f"2026-06-02T10:20:{30 + ordinal:02d}Z",
        "startedAtUtc": f"2026-06-02T10:20:{30 + ordinal:02d}Z",
        "finishedAtUtc": f"2026-06-02T10:20:{30 + ordinal:02d}Z",
        "eventId": f"event-{ordinal}",
        "sessionId": session_id,
        "sessionOrdinal": ordinal,
        "parentEventId": "hook-parent-1",
        "hookRunId": "hook-run-1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "argv": ["rs-harness"],
        "cwd": "/repo",
        "projectRoot": "/repo",
        "projectRootHash": "0123456789abcdef",
        "command": command,
        "result": {
            "exitCode": 0,
            "elapsedMs": 10,
            "stdoutBytes": 0,
            "stderrBytes": 0,
        },
        "fields": {"contextSource": context},
    }
