"""Failure-frontier receipt comparison tests."""

from __future__ import annotations

import json
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from tools.semantic_sandtable.cli import semantic_sandtable_main as main
from tools.semantic_sandtable.scenario_runner import run_scenario


_REPO_ROOT = Path(__file__).resolve().parents[3]
_TEST_BLOCK = (
    "crates/agent-semantic-client/tests/unit/cache_cli/writeback/search.rs:40-72"
)
_WRITEBACK_BLOCK = "crates/agent-semantic-client/src/cache_cli/writeback.rs:220-260"
_REPLAY_BLOCK = "crates/agent-semantic-client/src/cache_replay/artifact.rs:88-132"
_FRESHNESS_BLOCK = "crates/agent-semantic-client/src/cache_cli/probe.rs:140-205"
_HOT_BLOCKS = [
    _TEST_BLOCK,
    _WRITEBACK_BLOCK,
    _REPLAY_BLOCK,
    _FRESHNESS_BLOCK,
]


def test_compare_receipts_accepts_failure_frontier_round_reduction() -> None:
    baseline_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-baseline-receipt.json"
    )
    candidate_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-candidate-receipt.json"
    )

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(_REPO_ROOT),
                "--compare-receipts",
                str(baseline_path),
                str(candidate_path),
                *[
                    item
                    for block in _HOT_BLOCKS
                    for item in ("--expected-hot-block", block)
                ],
            ]
        )

    output = stdout.getvalue()
    assert exit_code == 0
    assert "[failure-frontier] status=pass" in output
    assert "baselineCommands=10 candidateCommands=5" in output
    assert "commandReductionRatio=0.500" in output
    assert "candidateDirectSourceReadCode=4" in output
    assert "candidateDuplicateSelectors=0" in output
    assert "coveredHotBlocks=4 expectedHotBlocks=4 missingHotBlocks=0" in output


def test_compare_receipts_rejects_window_scan_candidate(tmp_path: Path) -> None:
    baseline_path = tmp_path / "baseline.json"
    candidate_path = tmp_path / "candidate.json"
    baseline_path.write_text(
        json.dumps(
            _receipt(
                "rust.cache-replay-baseline",
                [_frontier_check("check", _HOT_BLOCKS)]
                + [
                    _direct_read(
                        f"window-{index}",
                        f"src/cache.rs:{index * 10}-{index * 10 + 20}",
                    )
                    for index in range(1, 11)
                ],
                stdout_bytes=1_000,
            )
        ),
        encoding="utf-8",
    )
    candidate_path.write_text(
        json.dumps(
            _receipt(
                "rust.cache-replay-window-scan",
                [
                    _direct_read("test-a", _TEST_BLOCK),
                    _direct_read(
                        "writeback-a",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:1-140",
                    ),
                    _direct_read(
                        "writeback-b",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:140-260",
                    ),
                    _direct_read(
                        "writeback-c",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:560-680",
                    ),
                    _direct_read(
                        "writeback-d",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:560-680",
                    ),
                    _direct_read(
                        "artifact-a",
                        "crates/agent-semantic-client/src/cache_replay/artifact.rs:1-120",
                    ),
                ],
                stdout_bytes=900,
            )
        ),
        encoding="utf-8",
    )

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(_REPO_ROOT),
                "--compare-receipts",
                str(baseline_path),
                str(candidate_path),
                *[
                    item
                    for block in _HOT_BLOCKS
                    for item in ("--expected-hot-block", block)
                ],
            ]
        )

    output = stdout.getvalue()
    assert exit_code == 1
    assert "[failure-frontier] status=fail" in output
    assert "candidateDirectSourceReadCode=6" in output
    assert "candidateDuplicateSelectors=1" in output
    assert "candidateSameFileWindowFanout=3" in output
    assert "|failure directSourceReadCode=6>4" in output
    assert "|failure duplicateSelectors=1>0" in output
    assert "|failure sameFileWindowFanout=3>0" in output
    assert "|failure missingHotBlocks=3" in output


def test_compare_receipts_rejects_declared_but_unread_frontier(tmp_path: Path) -> None:
    baseline_path = tmp_path / "baseline.json"
    candidate_path = tmp_path / "candidate.json"
    baseline_path.write_text(
        json.dumps(
            _receipt(
                "rust.cache-replay-baseline",
                [
                    _direct_read(f"window-{index}", f"src/cache.rs:{index}-{index + 1}")
                    for index in range(4)
                ],
                stdout_bytes=200,
            )
        ),
        encoding="utf-8",
    )
    candidate_path.write_text(
        json.dumps(
            _receipt(
                "rust.cache-replay-declared-only",
                [_frontier_check("check", [_TEST_BLOCK])],
                stdout_bytes=100,
            )
        ),
        encoding="utf-8",
    )

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(_REPO_ROOT),
                "--compare-receipts",
                str(baseline_path),
                str(candidate_path),
            ]
        )

    output = stdout.getvalue()
    assert exit_code == 1
    assert "coveredHotBlocks=0 expectedHotBlocks=1 missingHotBlocks=1" in output
    assert "|failure missingHotBlocks=1" in output


def test_compare_receipts_json_reports_custom_thresholds() -> None:
    baseline_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-baseline-receipt.json"
    )
    candidate_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-candidate-receipt.json"
    )

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(
            [
                "--repo-root",
                str(_REPO_ROOT),
                "--json",
                "--compare-receipts",
                str(baseline_path),
                str(candidate_path),
                "--min-command-reduction",
                "0.4",
                "--max-direct-source-read-code",
                "4",
                "--max-duplicate-selectors",
                "0",
                "--max-same-file-window-fanout",
                "0",
                *[
                    item
                    for block in _HOT_BLOCKS
                    for item in ("--expected-hot-block", block)
                ],
            ]
        )

    payload = json.loads(stdout.getvalue())
    assert exit_code == 0
    assert payload["schemaId"] == (
        "agent.semantic-protocols.semantic-sandtable-failure-frontier-comparison"
    )
    assert payload["status"] == "pass"
    assert payload["thresholds"] == {
        "minCommandReduction": 0.4,
        "maxDirectSourceReadCode": 4,
        "maxDuplicateSelectors": 0,
        "maxSameFileWindowFanout": 0,
    }
    assert payload["candidate"]["directSourceReadCodeCount"] == 4
    assert payload["candidate"]["sameFileWindowFanout"] == 0
    assert payload["frontier"]["coverageRatio"] == 1.0


def test_real_trigger_replay_scenario_runs_comparison_gate() -> None:
    scenario_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-real-trigger-replay.json"
    )

    result = run_scenario(_REPO_ROOT, scenario_path)

    comparison = result.evidence["failureFrontierComparisonResult"]
    assert isinstance(comparison, dict)
    assert result.status == "pass"
    assert [step.status for step in result.steps] == ["pass"]
    assert comparison["status"] == "pass"
    assert comparison["delta"]["commandReductionRatio"] == 0.5
    assert comparison["frontier"]["coverageRatio"] == 1.0


def test_real_trigger_replay_scenario_report_prints_comparison() -> None:
    scenario_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-real-trigger-replay.json"
    )

    stdout = StringIO()
    with redirect_stdout(stdout):
        exit_code = main(["--repo-root", str(_REPO_ROOT), str(scenario_path)])

    output = stdout.getvalue()
    assert exit_code == 0
    assert "|failureFrontier status=pass" in output
    assert "baselineCommands=10 candidateCommands=5" in output
    assert "commandReductionRatio=0.500" in output
    assert "directSourceReadCode=4 duplicateSelectors=0" in output
    assert "sameFileWindowFanout=0 missingHotBlocks=0" in output


def test_real_trigger_replay_scenario_fails_on_window_scan_candidate(
    tmp_path: Path,
) -> None:
    candidate_path = tmp_path / "candidate.json"
    scenario_path = tmp_path / "scenario.json"
    candidate_path.write_text(
        json.dumps(
            _receipt(
                "rust.cache-replay-window-scan",
                [
                    _direct_read("test-a", _TEST_BLOCK),
                    _direct_read(
                        "writeback-a",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:1-140",
                    ),
                    _direct_read(
                        "writeback-b",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:140-260",
                    ),
                    _direct_read(
                        "writeback-c",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:560-680",
                    ),
                    _direct_read(
                        "writeback-d",
                        "crates/agent-semantic-client/src/cache_cli/writeback.rs:560-680",
                    ),
                    _direct_read(
                        "artifact-a",
                        "crates/agent-semantic-client/src/cache_replay/artifact.rs:1-120",
                    ),
                ],
                stdout_bytes=900,
            )
        ),
        encoding="utf-8",
    )
    scenario_path.write_text(
        json.dumps(
            {
                "id": "rust.failure-frontier-window-scan",
                "language": "rust",
                "workdir": ".",
                "evidence": {
                    "source": "recorded-replay",
                    "failureFrontierComparison": {
                        "baselineReceiptPath": (
                            "sandtables/fixtures/asp/"
                            "failure-frontier-baseline-receipt.json"
                        ),
                        "candidateReceiptPath": str(candidate_path),
                        "expectedHotBlocks": _HOT_BLOCKS,
                    },
                },
                "steps": [
                    {
                        "id": "comparison-recorded",
                        "command": ["true"],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    result = run_scenario(_REPO_ROOT, scenario_path)

    comparison = result.evidence["failureFrontierComparisonResult"]
    assert isinstance(comparison, dict)
    assert result.status == "fail"
    assert result.steps == []
    assert comparison["status"] == "fail"
    assert result.errors == [
        "failure-frontier comparison failed: "
        "commandReductionRatio=0.400<0.500, directSourceReadCode=6>4, "
        "duplicateSelectors=1>0, sameFileWindowFanout=3>0, missingHotBlocks=3"
    ]


def _receipt(
    scenario_id: str,
    commands: list[dict[str, object]],
    *,
    stdout_bytes: int,
) -> dict[str, object]:
    for command in commands:
        metrics = command["metrics"]
        assert isinstance(metrics, dict)
        metrics.setdefault("stdoutBytes", stdout_bytes)
    return {
        "schemaId": "agent.semantic-protocols.semantic-sandtable-receipt",
        "schemaVersion": "1",
        "scenarioId": scenario_id,
        "language": "rust",
        "project": {"name": "agent-semantic-protocols", "source": "fixture"},
        "intent": "Compare baseline source-window scan with failure-frontier flow.",
        "editBoundary": "before-edit",
        "commands": commands,
        "summary": {
            "commandCount": len(commands),
            "stdoutBytes": sum(
                _metric_int(command, "stdoutBytes") for command in commands
            ),
            "stderrBytes": 0,
            "elapsedMs": sum(_metric_int(command, "elapsedMs") for command in commands),
        },
    }


def _frontier_check(command_id: str, next_items: list[str]) -> dict[str, object]:
    return {
        "id": command_id,
        "kind": "check",
        "argv": ["asp", "rust", "check", "changed", "--view", "seeds", "."],
        "outputMode": "compact",
        "next": next_items,
        "metrics": {"elapsedMs": 5, "stdoutBytes": 180, "stderrBytes": 0},
    }


def _direct_read(
    command_id: str,
    selector: str,
    *,
    stdout_bytes: int = 700,
) -> dict[str, object]:
    return {
        "id": command_id,
        "kind": "other",
        "argv": [
            "asp",
            "rust",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            selector,
            "--code",
            ".",
        ],
        "outputMode": "compact",
        "metrics": {
            "elapsedMs": 3,
            "stdoutBytes": stdout_bytes,
            "stderrBytes": 0,
        },
    }


def _metric_int(command: dict[str, object], field: str) -> int:
    metrics = command["metrics"]
    assert isinstance(metrics, dict)
    value = metrics[field]
    assert isinstance(value, int)
    return value
