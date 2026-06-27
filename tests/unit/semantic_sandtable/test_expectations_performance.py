"""Validate sandtable performance expectation gates."""

from __future__ import annotations

import json
from pathlib import Path

from tools.semantic_sandtable.expectations import validate_step
from tools.semantic_sandtable.models import StepResult
from tools.semantic_sandtable.scenario_runner import run_scenario


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


def test_max_cold_start_elapsed_ms_is_a_hard_step_gate() -> None:
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
        {"expect": {"maxColdStartElapsedMs": 100}},
        result,
        "done",
        "",
        Path("."),
    )

    assert "elapsedMs=101 exceeds maxColdStartElapsedMs=100" in result.errors


def test_cold_start_retry_requires_final_warm_budget(tmp_path: Path) -> None:
    scenario_path = tmp_path / "cold-start.json"
    scenario_path.write_text(
        json.dumps(
            {
                "id": "cold.start",
                "language": "python",
                "workdir": ".",
                "steps": [
                    {
                        "id": "probe",
                        "command": [
                            "sh",
                            "-c",
                            (
                                "if [ ! -f cold.marker ]; then "
                                ": > cold.marker; sleep 0.25; "
                                "fi; printf ok"
                            ),
                        ],
                        "expect": {
                            "stdoutContains": ["ok"],
                            "maxElapsedMs": 150,
                            "maxColdStartElapsedMs": 1000,
                        },
                    }
                ],
            }
        ),
        encoding="utf-8",
    )

    result = run_scenario(tmp_path, scenario_path)

    assert result.status == "pass"
    assert len(result.steps) == 1
    assert result.steps[0].errors == []
    assert result.steps[0].observations["coldStartRetry"]["maxElapsedMs"] == 150
