"""Trace receipt filtering for dev-command-log roots."""

from __future__ import annotations

import json
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.trace_receipt_events import TraceCommandFilter
from tools.semantic_sandtable.trace_receipts import (
    TraceReceiptConfig,
    build_receipt_from_trace_path,
)

from .trace_receipt_fixtures import REPO_ROOT


def test_build_receipt_filters_dev_log_directory_by_session(tmp_path: Path) -> None:
    trace_root = _write_dev_log_root(tmp_path)

    receipt = build_receipt_from_trace_path(
        trace_root,
        config=_config(),
        filters=TraceCommandFilter(session_id="session-a"),
    )

    assert receipt["summary"]["commandCount"] == 1
    assert receipt["summary"]["jsonSearches"] == 1
    command = receipt["commands"][0]
    assert command["id"] == "event-a"
    assert command["kind"] == "search"
    assert command["argv"] == [
        "py-harness",
        "search",
        "prime",
        "--view",
        "seeds",
        "--json",
    ]
    assert command["metrics"] == {
        "elapsedMs": 11,
        "stdoutBytes": 101,
        "stderrBytes": 0,
    }


def test_cli_build_receipt_filters_dev_log_root(tmp_path: Path) -> None:
    trace_root = _write_dev_log_root(tmp_path)
    output_path = tmp_path / "receipt.json"

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--build-receipt-from-trace",
                str(trace_root),
                "--output",
                str(output_path),
                "--scenario-id",
                "python.dev-log-session",
                "--language",
                "python",
                "--project-name",
                "agent-semantic-protocols",
                "--project-source",
                "fixture",
                "--intent",
                "Build receipt from a dev command log root.",
                "--trace-session-id",
                "session-b",
                "--trace-language-id",
                "python",
                "--trace-provider-id",
                "py-harness",
            ]
        )

    receipt = json.loads(output_path.read_text(encoding="utf-8"))
    assert exit_code == 0
    assert "[receipt] receipts=1 pass=1 fail=0" in stdout.getvalue()
    assert "commands=1" in stdout.getvalue()
    assert "jsonSearches=1" in stdout.getvalue()
    assert receipt["commands"][0]["id"] == "event-b"
    assert receipt["summary"]["stdoutBytes"] == 202


def _write_dev_log_root(tmp_path: Path) -> Path:
    command_dir = tmp_path / "semantic_protocol" / "python" / "py-harness" / "commands"
    command_dir.mkdir(parents=True)
    (command_dir / "commands.jsonl").write_text(
        "\n".join(
            [
                json.dumps(_dev_log_event("event-a", "session-a", 11, 101)),
                json.dumps(_dev_log_event("event-b", "session-b", 22, 202)),
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    (tmp_path / "hooks.jsonl").write_text(
        json.dumps(_dev_log_event("ignored", "session-b", 33, 303)) + "\n",
        encoding="utf-8",
    )
    return tmp_path


def _dev_log_event(
    event_id: str,
    session_id: str,
    elapsed_ms: int,
    stdout_bytes: int,
) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.dev-command-log",
        "schemaVersion": "1",
        "eventId": event_id,
        "sessionId": session_id,
        "languageId": "python",
        "providerId": "py-harness",
        "argv": ["py-harness", "search", "prime", "--view", "seeds", "--json"],
        "command": {"method": "search/prime"},
        "result": {
            "exitCode": 0,
            "elapsedMs": elapsed_ms,
            "stdoutBytes": stdout_bytes,
            "stderrBytes": 0,
        },
    }


def _config() -> TraceReceiptConfig:
    return TraceReceiptConfig(
        scenario_id="python.dev-log-session",
        language="python",
        project_name="agent-semantic-protocols",
        project_source="fixture",
        intent="Build receipt from a dev command log root.",
    )
