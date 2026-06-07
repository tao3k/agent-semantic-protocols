"""Validate sandtable step expectation behavior."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.expectations import validate_step
from tools.semantic_sandtable.models import StepResult


def test_allow_non_zero_exit_suppresses_exit_code_error() -> None:
    result = StepResult(
        scenario_id="rust.live",
        step_id="claude",
        command=["claude"],
        status="pass",
        exit_code=1,
        elapsed_ms=10,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
    )

    validate_step(
        {"expect": {"allowNonZeroExit": True}},
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []


def test_pipe_flow_output_budget_requires_attribution() -> None:
    result = StepResult(
        scenario_id="rust.live",
        step_id="claude",
        command=["claude"],
        status="pass",
        exit_code=0,
        elapsed_ms=10,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
        observations={
            "pipeFlow": {
                "aspCommands": 1,
                "complexPipeFlow": False,
                "missingComplexPipeStages": [],
            }
        },
    )

    validate_step(
        {"expect": {"pipeFlow": {"maxAspCommandOutputBytes": 8000}}},
        result,
        "done",
        "",
        Path("."),
    )

    assert (
        "pipeFlow aspCommandOutputBytes missing for maxAspCommandOutputBytes"
        in result.errors
    )


def test_pipe_flow_precision_gate_accepts_preserved_semantic_evidence() -> None:
    result = StepResult(
        scenario_id="rust.live",
        step_id="claude",
        command=["claude"],
        status="pass",
        exit_code=0,
        elapsed_ms=10,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
        observations={
            "pipeFlow": {
                "aspCommands": 3,
                "complexPipeFlow": True,
                "missingComplexPipeStages": [],
                "searchPipeOutputPrecision": {
                    "fieldFacts": 1,
                    "typeFacts": 1,
                    "collectionFacts": 1,
                    "collectionOfEdges": 1,
                    "s1Selectors": 1,
                    "nextCommands": 1,
                    "exactQueryCoverage": 1,
                    "debugRows": 0,
                },
            }
        },
    )

    validate_step(
        {"expect": {"pipeFlow": {"requireSearchPipePrecision": True}}},
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []


def test_pipe_flow_frontier_context_gates_accept_followed_frontier() -> None:
    result = StepResult(
        scenario_id="rust.live",
        step_id="claude",
        command=["claude"],
        status="pass",
        exit_code=0,
        elapsed_ms=10,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
        observations={
            "pipeFlow": {
                "aspCommands": 3,
                "complexPipeFlow": True,
                "missingComplexPipeStages": [],
                "frontierFollowRate": 0.75,
                "contextPrecision": 1.0,
                "contextUtilization": 0.75,
            }
        },
    )

    validate_step(
        {
            "expect": {
                "pipeFlow": {
                    "minFrontierFollowRate": 0.75,
                    "minContextPrecision": 0.9,
                    "minContextUtilization": 0.7,
                }
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []


def test_pipe_flow_memory_and_failure_precision_gates_accept_preserved_frontier() -> (
    None
):
    result = StepResult(
        scenario_id="rust.live",
        step_id="claude",
        command=["claude"],
        status="pass",
        exit_code=0,
        elapsed_ms=10,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
        observations={
            "pipeFlow": {
                "aspCommands": 3,
                "complexPipeFlow": True,
                "missingComplexPipeStages": [],
                "readLoopMemory": {
                    "entryCount": 1,
                    "entries": [{"selector": "src/lib.rs:1:3"}],
                },
                "failureLoopMemory": {
                    "entryCount": 1,
                    "entries": [{"selector": "src/lib.rs:1:3"}],
                },
                "failureFrontierOutputPrecision": {
                    "failureFacts": 1,
                    "assertFacts": 1,
                    "hotFacts": 1,
                    "frontierActions": 1,
                    "queryProfiles": 1,
                    "omitRows": 1,
                    "avoidRows": 1,
                    "debugRows": 0,
                },
            }
        },
    )

    validate_step(
        {
            "expect": {
                "pipeFlow": {
                    "requireReadLoopMemory": True,
                    "requireFailureFrontierPrecision": True,
                    "requireFailureLoopMemory": True,
                }
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []
