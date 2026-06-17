"""Validate timeout evidence retention for sandtable step processes."""

from __future__ import annotations

import json
import sys
from pathlib import Path

from tools.semantic_sandtable.models import StepResult
from tools.semantic_sandtable.step_process import run_step_process


def test_timeout_keeps_partial_stdout_observations() -> None:
    payload = {
        "type": "AssistantMessage",
        "content": [
            {
                "name": "Bash",
                "input": {"command": "asp rust search prime --workspace . --view seeds"},
            }
        ],
    }
    script = (
        "import sys, time; "
        f"print({json.dumps(json.dumps(payload))}); "
        "sys.stdout.flush(); "
        "time.sleep(1)"
    )

    result = run_step_process(
        [sys.executable, "-c", script],
        Path("."),
        {},
        None,
        0.1,
        "rust.timeout",
        "agent",
    )

    assert isinstance(result, StepResult)
    assert result.status == "fail"
    assert result.stdout_lines == 1
    assert result.observations["pipeFlow"]["aspCommands"] == 1
