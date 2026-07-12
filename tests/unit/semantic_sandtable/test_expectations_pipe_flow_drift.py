"""Validate pipe-flow drift expectation diagnostics."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.expectations import validate_step
from tools.semantic_sandtable.models import StepResult


def test_pipe_flow_expectation_reports_budget_and_stage_drift() -> None:
    result = _pipe_flow_drift_result()

    validate_step(
        {
            "expect": {
                "allowNonZeroExit": True,
                "pipeFlow": _strict_pipe_flow_expectation(),
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    _assert_pipe_flow_drift_errors(result.errors)


def _pipe_flow_drift_result() -> StepResult:
    return StepResult(
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
        observations={"pipeFlow": _drifting_pipe_flow_observation()},
    )


def _drifting_pipe_flow_observation() -> dict[str, object]:
    return {
        "aspCommands": 9,
        "searchCommands": 5,
        "queryCommands": 1,
        "guideCommands": 1,
        "repeatedCommands": 1,
        "searchPipeCommands": 2,
        "searchPrimeCommands": 2,
        "searchFailureCommands": 2,
        "searchLexicalCommands": 0,
        "searchReasoningCommands": 1,
        "querySelectorCommands": 0,
        "readLoopDirectCodeCommands": 3,
        "readLoopDuplicateSelectors": 1,
        "readLoopAdjacentRangeWindows": 1,
        "readLoopSameOwnerScans": 2,
        "readLoopMemorySuppressibleReads": 3,
        "aspCommandOutputBytes": 9001,
        "frontierFollowRate": 0.25,
        "contextPrecision": 0.5,
        "contextUtilization": 0.2,
        "searchPipeOutputPrecision": {
            "fieldFacts": 0,
            "typeFacts": 1,
            "collectionFacts": 1,
            "collectionOfEdges": 1,
            "s1Selectors": 1,
            "nextCommands": 1,
            "exactQueryCoverage": 1,
            "debugRows": 1,
        },
        "failureFrontierOutputPrecision": {
            "failureFacts": 0,
            "assertFacts": 1,
            "hotFacts": 1,
            "frontierActions": 1,
            "queryProfiles": 1,
            "omitRows": 1,
            "avoidRows": 1,
            "debugRows": 1,
        },
        "complexPipeFlow": False,
        "missingComplexPipeStages": ["query-selector"],
    }


def _strict_pipe_flow_expectation() -> dict[str, object]:
    return {
        "maxAspCommands": 8,
        "maxSearchCommands": 4,
        "maxGuideCommands": 0,
        "maxRepeatedCommands": 0,
        "maxSearchPipeCommands": 1,
        "maxSearchPrimeCommands": 1,
        "maxSearchFailureCommands": 1,
        "maxReadLoopDirectCodeCommands": 2,
        "maxReadLoopDuplicateSelectors": 0,
        "maxReadLoopAdjacentRangeWindows": 0,
        "maxReadLoopSameOwnerScans": 0,
        "maxReadLoopMemorySuppressibleReads": 0,
        "maxAspCommandOutputBytes": 8000,
        "minQuerySelectorCommands": 1,
        "minFrontierFollowRate": 0.75,
        "minContextPrecision": 0.75,
        "minContextUtilization": 0.5,
        "requireComplexPipeFlow": True,
        "requireTokenCost": True,
        "requireSearchPipePrecision": True,
        "requireReadLoopMemory": True,
        "requireFailureFrontierPrecision": True,
        "requireFailureLoopMemory": True,
        "requiredStages": ["search-pipe", "query-selector"],
        "forbiddenStages": [
            "repeated-prime",
            "repeated-commands",
            "read-loop-risk",
            "read-loop-memory-risk",
        ],
    }


def _assert_pipe_flow_drift_errors(errors: list[str]) -> None:
    assert "pipeFlow aspCommands=9 exceeds maxAspCommands=8" in errors
    assert "pipeFlow searchCommands=5 exceeds maxSearchCommands=4" in errors
    assert "pipeFlow guideCommands=1 exceeds maxGuideCommands=0" in errors
    assert "pipeFlow repeatedCommands=1 exceeds maxRepeatedCommands=0" in errors
    assert "pipeFlow searchPipeCommands=2 exceeds maxSearchPipeCommands=1" in errors
    assert "pipeFlow searchPrimeCommands=2 exceeds maxSearchPrimeCommands=1" in errors
    assert (
        "pipeFlow searchFailureCommands=2 exceeds maxSearchFailureCommands=1" in errors
    )
    assert (
        "pipeFlow readLoopDirectCodeCommands=3 exceeds maxReadLoopDirectCodeCommands=2"
        in errors
    )
    assert (
        "pipeFlow readLoopDuplicateSelectors=1 exceeds maxReadLoopDuplicateSelectors=0"
        in errors
    )
    assert (
        "pipeFlow readLoopAdjacentRangeWindows=1 exceeds "
        "maxReadLoopAdjacentRangeWindows=0"
    ) in errors
    assert (
        "pipeFlow readLoopSameOwnerScans=2 exceeds maxReadLoopSameOwnerScans=0"
        in errors
    )
    assert (
        "pipeFlow readLoopMemorySuppressibleReads=3 exceeds "
        "maxReadLoopMemorySuppressibleReads=0"
    ) in errors
    assert (
        "pipeFlow aspCommandOutputBytes=9001 exceeds maxAspCommandOutputBytes=8000"
        in errors
    )
    assert (
        "pipeFlow frontierFollowRate=0.2500 below minFrontierFollowRate=0.7500"
        in errors
    )
    assert (
        "pipeFlow contextPrecision=0.5000 below minContextPrecision=0.7500"
        in errors
    )
    assert (
        "pipeFlow contextUtilization=0.2000 below minContextUtilization=0.5000"
        in errors
    )
    assert "pipeFlow searchPipeOutputPrecision fieldFacts=0 below 1" in errors
    assert "pipeFlow searchPipeOutputPrecision debugRows=1 expected=0" in errors
    assert "pipeFlow readLoopMemory missing" in errors
    assert "pipeFlow failureFrontierOutputPrecision failureFacts=0 below 1" in errors
    assert "pipeFlow failureFrontierOutputPrecision debugRows=1 expected=0" in errors
    assert "pipeFlow failureLoopMemory missing" in errors
    assert "pipeFlow querySelectorCommands=0 below minQuerySelectorCommands=1" in errors
    assert "pipeFlow complex=false missing=['query-selector']" in errors
    assert "tokenCost missing from agent observations" in errors
    assert "pipeFlow missing required stage 'query-selector'" in errors
    assert "pipeFlow contains forbidden stage 'repeated-prime'" in errors
    assert "pipeFlow contains forbidden stage 'repeated-commands'" in errors
    assert "pipeFlow contains forbidden stage 'read-loop-risk'" in errors
    assert "pipeFlow contains forbidden stage 'read-loop-memory-risk'" in errors
