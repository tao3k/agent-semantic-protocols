"""Validate sandtable performance expectation gates."""

from __future__ import annotations

from pathlib import Path

from tools.semantic_sandtable.expectations import validate_step
from tools.semantic_sandtable.models import StepResult


def test_max_elapsed_ms_is_a_hard_step_gate() -> None:
    result = StepResult(
        scenario_id="org.recall",
        step_id="recall",
        command=["asp", "org", "recall", "plans"],
        status="pass",
        exit_code=0,
        elapsed_ms=101,
        stdout_lines=1,
        stderr_lines=0,
        stdout_bytes=4,
        stderr_bytes=0,
    )

    validate_step(
        {"expect": {"maxElapsedMs": 100}},
        result,
        "done",
        "",
        Path("."),
    )

    assert "elapsedMs=101 exceeds maxElapsedMs=100" in result.errors
