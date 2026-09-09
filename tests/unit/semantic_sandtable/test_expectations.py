# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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


def test_agent_answer_expectation_requires_final_answer_after_tools() -> None:
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
            "finalAnswer": {
                "present": False,
                "afterLastToolUse": False,
                "textBytes": 0,
                "textPreview": "",
            }
        },
    )

    validate_step(
        {"expect": {"agentAnswer": {"required": True, "afterLastToolUse": True}}},
        result,
        "done",
        "",
        Path("."),
    )

    assert "agentAnswer missing explicit final assistant answer" in result.errors
    assert "agentAnswer was not after the last tool use" in result.errors


def test_agent_answer_expectation_accepts_explicit_answer_text() -> None:
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
            "finalAnswer": {
                "present": True,
                "afterLastToolUse": True,
                "textBytes": 120,
                "textPreview": "Vec fields are collection fields, not scalar fields.",
            }
        },
    )

    validate_step(
        {
            "expect": {
                "agentAnswer": {
                    "required": True,
                    "afterLastToolUse": True,
                    "minTextBytes": 80,
                    "contains": ["Vec", "collection", "scalar"],
                }
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []


def test_command_flow_output_budget_requires_attribution() -> None:
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
            "commandFlow": {
                "aspCommands": 1,
            }
        },
    )

    validate_step(
        {"expect": {"commandFlow": {"maxAspCommandOutputBytes": 8000}}},
        result,
        "done",
        "",
        Path("."),
    )

    assert (
        "commandFlow aspCommandOutputBytes missing for maxAspCommandOutputBytes"
        in result.errors
    )


def test_command_flow_frontier_context_gates_accept_followed_frontier() -> None:
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
            "commandFlow": {
                "aspCommands": 3,
                "frontierFollowRate": 0.75,
                "contextPrecision": 1.0,
                "contextUtilization": 0.75,
            }
        },
    )

    validate_step(
        {
            "expect": {
                "commandFlow": {
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


def test_command_flow_memory_gate_accepts_preserved_read_memory() -> None:
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
            "commandFlow": {
                "aspCommands": 3,
                "readLoopMemory": {
                    "entryCount": 1,
                    "entries": [{"selector": "src/lib.rs:1:3"}],
                },
            }
        },
    )

    validate_step(
        {
            "expect": {
                "commandFlow": {
                    "requireReadLoopMemory": True,
                }
            }
        },
        result,
        "done",
        "",
        Path("."),
    )

    assert result.errors == []
