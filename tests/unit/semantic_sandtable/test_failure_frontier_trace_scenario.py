"""Failure-frontier scenario comparison from trace inputs."""

from __future__ import annotations

import json
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario

from .trace_receipt_fixtures import (
    HOT_BLOCKS,
    write_failure_frontier_dev_log_root,
)

_REPO_ROOT = Path(__file__).resolve().parents[3]


def test_real_trigger_trace_replay_scenario_builds_receipts_in_memory() -> None:
    scenario_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-frontier-real-trigger-trace-replay.json"
    )

    result = run_scenario(_REPO_ROOT, scenario_path)

    comparison = result.evidence["failureFrontierComparisonResult"]
    assert isinstance(comparison, dict)
    assert result.status == "pass"
    assert [step.status for step in result.steps] == ["pass"]
    assert comparison["status"] == "pass"
    assert comparison["baseline"]["commandCount"] == 10
    assert comparison["candidate"]["commandCount"] == 5
    assert comparison["candidate"]["directSourceReadCodeCount"] == 4
    assert comparison["delta"]["commandReductionRatio"] == 0.5
    assert comparison["frontier"]["coverageRatio"] == 1.0


def test_failure_loop_memory_replay_scenario_gates_recorded_agent_output() -> None:
    scenario_path = (
        _REPO_ROOT
        / "sandtables"
        / "fixtures"
        / "asp"
        / "failure-loop-memory-replay.json"
    )

    result = run_scenario(_REPO_ROOT, scenario_path)

    assert result.status == "pass"
    assert [step.status for step in result.steps] == ["pass"]
    pipe_flow = result.steps[0].observations["pipeFlow"]
    assert pipe_flow["searchFailureCommands"] == 1
    assert pipe_flow["failureLoopMemoryEntryCount"] == 1
    assert pipe_flow["failureFrontierOutputPrecision"]["hotFacts"] == 1


def test_trace_replay_scenario_filters_dev_log_root_by_session(
    tmp_path: Path,
) -> None:
    trace_root = tmp_path / "trace-root"
    scenario_path = tmp_path / "scenario.json"
    write_failure_frontier_dev_log_root(trace_root)
    _write_session_filter_scenario(scenario_path, trace_root)

    result = run_scenario(_REPO_ROOT, scenario_path)

    comparison = result.evidence["failureFrontierComparisonResult"]
    assert isinstance(comparison, dict)
    assert result.status == "pass"
    assert comparison["baseline"]["commandCount"] == 10
    assert comparison["candidate"]["commandCount"] == 5
    assert comparison["delta"]["commandReductionRatio"] == 0.5


def _write_session_filter_scenario(path: Path, trace_root: Path) -> None:
    path.write_text(
        json.dumps(
            {
                "id": "rust.failure-frontier-session-filter",
                "language": "rust",
                "workdir": ".",
                "evidence": {
                    "source": "recorded-replay",
                    "failureFrontierComparison": {
                        "baselineTracePath": str(trace_root),
                        "candidateTracePath": str(trace_root),
                        "baselineTraceSessionId": "baseline",
                        "candidateTraceSessionId": "candidate",
                        "traceLanguageId": "rust",
                        "traceProviderId": "rs-harness",
                        "projectName": "agent-semantic-protocols",
                        "projectSource": "fixture",
                        "expectedHotBlocks": HOT_BLOCKS,
                    },
                },
                "steps": [{"id": "comparison-recorded", "command": ["true"]}],
            }
        ),
        encoding="utf-8",
    )
