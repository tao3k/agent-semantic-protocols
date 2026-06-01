"""Shared report formatting helpers."""

from __future__ import annotations

import json
import re

from .models import ScenarioResult


def scenario_totals(result: ScenarioResult) -> dict[str, int]:
    return {
        "commands": len(result.steps),
        "elapsedMs": sum(step.elapsed_ms for step in result.steps),
        "stdoutBytes": sum(step.stdout_bytes for step in result.steps),
        "stderrBytes": sum(step.stderr_bytes for step in result.steps),
    }


def quote_value(value: str) -> str:
    if re.fullmatch(r"[A-Za-z0-9_.:/=-]+", value):
        return value
    return json.dumps(value)
