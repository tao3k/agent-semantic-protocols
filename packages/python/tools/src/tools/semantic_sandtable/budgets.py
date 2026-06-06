"""Warning budget helpers for sandtable results."""

from __future__ import annotations

from typing import Any

from .models import ScenarioResult, StepResult
from .utils import optional_int


def warn_if_over(
    result: StepResult,
    name: str,
    actual: int,
    threshold_name: str,
    threshold: Any,
) -> None:
    limit = optional_int(threshold)
    if limit is not None and actual > limit:
        result.warnings.append(f"{name}={actual} exceeds {threshold_name}={limit}")


def warn_scenario_if_over(
    result: ScenarioResult,
    name: str,
    actual: int,
    threshold_name: str,
    threshold: Any,
) -> None:
    limit = optional_int(threshold)
    if limit is not None and actual > limit:
        result.warnings.append(f"{name}={actual} exceeds {threshold_name}={limit}")
