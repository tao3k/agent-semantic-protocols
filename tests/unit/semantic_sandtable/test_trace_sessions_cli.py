"""CLI listing for trace sessions."""

from __future__ import annotations

import json
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main

from .trace_receipt_fixtures import REPO_ROOT, write_failure_frontier_dev_log_root


def test_cli_lists_trace_sessions(tmp_path: Path) -> None:
    trace_root = tmp_path / "trace-root"
    write_failure_frontier_dev_log_root(trace_root)

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--list-trace-sessions",
                str(trace_root),
                "--trace-language-id",
                "rust",
                "--trace-provider-id",
                "rs-harness",
            ]
        )

    output = stdout.getvalue()
    assert exit_code == 0
    assert "[trace-sessions] sessions=2 commands=15 files=1" in output
    assert (
        "|session id=baseline commands=10 languages=rust providers=rs-harness" in output
    )
    assert (
        "|session id=candidate commands=5 languages=rust providers=rs-harness" in output
    )
    assert "stdoutBytes=7000" in output
    assert "stdoutBytes=660" in output


def test_cli_lists_trace_sessions_as_json(tmp_path: Path) -> None:
    trace_root = tmp_path / "trace-root"
    write_failure_frontier_dev_log_root(trace_root)

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--json",
                "--list-trace-sessions",
                str(trace_root),
                "--trace-language-id",
                "rust",
                "--trace-provider-id",
                "rs-harness",
            ]
        )

    report = json.loads(stdout.getvalue())
    assert exit_code == 0
    assert report["schemaId"] == (
        "agent.semantic-protocols.semantic-sandtable-trace-sessions"
    )
    assert report["summary"] == {
        "sessionCount": 2,
        "commandCount": 15,
        "traceFileCount": 1,
    }
    assert {session["sessionId"] for session in report["sessions"]} == {
        "baseline",
        "candidate",
    }
