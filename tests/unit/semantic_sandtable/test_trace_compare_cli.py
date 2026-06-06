"""CLI comparison for failure-frontier trace pairs."""

from __future__ import annotations

from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main

from .trace_receipt_fixtures import (
    HOT_BLOCKS,
    REPO_ROOT,
    write_failure_frontier_dev_log_root,
)


def test_cli_compares_dev_log_trace_sessions(tmp_path: Path) -> None:
    trace_root = tmp_path / "trace-root"
    write_failure_frontier_dev_log_root(trace_root)

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--compare-traces",
                str(trace_root),
                str(trace_root),
                "--baseline-trace-session-id",
                "baseline",
                "--candidate-trace-session-id",
                "candidate",
                "--trace-language-id",
                "rust",
                "--trace-provider-id",
                "rs-harness",
                "--scenario-id",
                "rust.failure-frontier-trace-cli",
                "--language",
                "rust",
                "--project-name",
                "agent-semantic-protocols",
                "--project-source",
                "fixture",
                "--intent",
                "Compare dev command log sessions.",
            ]
        )

    output = stdout.getvalue()
    assert exit_code == 0
    assert "[failure-frontier] status=pass" in output
    assert "baselineCommands=10 candidateCommands=5" in output
    assert "commandReductionRatio=0.500" in output
    assert "candidateDirectSourceReadCode=4" in output
    assert "coveredHotBlocks=4 expectedHotBlocks=4 missingHotBlocks=0" in output
