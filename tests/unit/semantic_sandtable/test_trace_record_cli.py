"""CLI recording for command trace events."""

from __future__ import annotations

import json
import sys
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main

from .trace_receipt_fixtures import REPO_ROOT


def test_cli_records_command_event(tmp_path: Path) -> None:
    trace_root = tmp_path / "trace-root"

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--record-trace-root",
                str(trace_root),
                "--record-session-id",
                "record-session",
                "--trace-language-id",
                "python",
                "--trace-provider-id",
                "python",
                "--record-command",
                sys.executable,
                "-c",
                "print('hello trace')",
            ]
        )

    output = stdout.getvalue()
    assert exit_code == 0
    assert "[trace-record]" in output
    assert "session=record-session" in output
    assert "stdoutBytes=12" in output

    session_output = _list_sessions(trace_root)
    assert "[trace-sessions] sessions=1 commands=1 files=1" in session_output
    assert (
        "|session id=record-session commands=1 languages=python providers=python"
        in (session_output)
    )
    assert "stdoutBytes=12" in session_output


def test_cli_records_failure_frontier_next_from_stdout(tmp_path: Path) -> None:
    trace_root = tmp_path / "trace-root"

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--record-trace-root",
                str(trace_root),
                "--record-session-id",
                "record-session",
                "--trace-language-id",
                "rust",
                "--trace-provider-id",
                "rs-harness",
                "--record-command",
                sys.executable,
                "-c",
                "print('|hotBlock selector=src/lib.rs:1-14 source=finding rule=RUST-PROJ-R003 line=2')",
            ]
        )

    assert exit_code == 0
    command_event = _recorded_event(trace_root)
    assert command_event["next"] == ["src/lib.rs:1-14"]
    assert command_event["result"]["stdoutPath"].startswith("outputs/")
    assert (trace_root / command_event["result"]["stdoutPath"]).is_file()


def _list_sessions(trace_root: Path) -> str:
    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--list-trace-sessions",
                str(trace_root),
                "--trace-language-id",
                "python",
                "--trace-provider-id",
                "python",
            ]
        )
    assert exit_code == 0
    return stdout.getvalue()


def _recorded_event(trace_root: Path) -> dict[str, object]:
    paths = sorted((trace_root / "rust" / "rs-harness" / "commands").glob("*.jsonl"))
    assert len(paths) == 1
    return json.loads(paths[0].read_text(encoding="utf-8"))
