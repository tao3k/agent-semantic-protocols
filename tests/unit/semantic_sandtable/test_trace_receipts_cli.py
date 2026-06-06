"""CLI integration for trace-built sandtable receipts."""

from __future__ import annotations

import json
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main

from .trace_receipt_fixtures import (
    FRESHNESS_BLOCK,
    HOT_BLOCKS,
    REPLAY_BLOCK,
    REPO_ROOT,
    TEST_BLOCK,
    WRITEBACK_BLOCK,
    trace_event,
    window_selector,
)


def test_cli_builds_receipts_that_feed_failure_frontier_compare(
    tmp_path: Path,
) -> None:
    baseline_trace = tmp_path / "baseline.jsonl"
    candidate_trace = tmp_path / "candidate.jsonl"
    baseline_receipt = tmp_path / "baseline.json"
    candidate_receipt = tmp_path / "candidate.json"
    _write_baseline_trace(baseline_trace)
    _write_candidate_trace(candidate_trace)

    assert (
        _build_receipt_cli(
            baseline_trace,
            baseline_receipt,
            scenario_id="rust.failure-frontier-baseline-from-trace",
        )
        == 0
    )
    assert (
        _build_receipt_cli(
            candidate_trace,
            candidate_receipt,
            scenario_id="rust.failure-frontier-candidate-from-trace",
        )
        == 0
    )

    output, exit_code = _compare_receipt_cli(baseline_receipt, candidate_receipt)

    assert exit_code == 0
    assert "[failure-frontier] status=pass" in output
    assert "baselineCommands=10 candidateCommands=5" in output
    assert "commandReductionRatio=0.500" in output
    assert "candidateDirectSourceReadCode=4" in output
    assert "coveredHotBlocks=4 expectedHotBlocks=4 missingHotBlocks=0" in output


def _write_baseline_trace(path: Path) -> None:
    path.write_text(
        "\n".join(
            json.dumps(trace_event(f"window-{index}", window_selector(index)))
            for index in range(1, 11)
        )
        + "\n",
        encoding="utf-8",
    )


def _write_candidate_trace(path: Path) -> None:
    path.write_text(
        "\n".join(
            [
                json.dumps(_frontier_event()),
                json.dumps(trace_event("test", TEST_BLOCK)),
                json.dumps(trace_event("writeback", WRITEBACK_BLOCK)),
                json.dumps(trace_event("replay", REPLAY_BLOCK)),
                json.dumps(trace_event("freshness", FRESHNESS_BLOCK)),
            ]
        )
        + "\n",
        encoding="utf-8",
    )


def _frontier_event() -> dict[str, object]:
    return {
        "id": "failure-frontier",
        "kind": "check",
        "argv": ["asp", "rust", "check", "changed", "--view", "seeds", "."],
        "next": HOT_BLOCKS,
        "metrics": {"elapsedMs": 5, "stdoutBytes": 180, "stderrBytes": 0},
    }


def _build_receipt_cli(
    trace_path: Path,
    output_path: Path,
    *,
    scenario_id: str,
) -> int:
    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--build-receipt-from-trace",
                str(trace_path),
                "--output",
                str(output_path),
                "--scenario-id",
                scenario_id,
                "--language",
                "rust",
                "--project-name",
                "agent-semantic-protocols",
                "--project-source",
                "fixture",
                "--intent",
                "Build receipt from trace fixture.",
            ]
        )
    assert "[receipt] receipts=1 pass=1 fail=0" in stdout.getvalue()
    return exit_code


def _compare_receipt_cli(
    baseline_receipt: Path, candidate_receipt: Path
) -> tuple[str, int]:
    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(REPO_ROOT),
                "--compare-receipts",
                str(baseline_receipt),
                str(candidate_receipt),
                *[
                    item
                    for block in HOT_BLOCKS
                    for item in ("--expected-hot-block", block)
                ],
            ]
        )
    return stdout.getvalue(), exit_code
