"""Compatibility exports for semantic sandtable CLI and runners."""

from __future__ import annotations

from .cli import semantic_sandtable_main
from .coverage import coverage_report
from .scenario_io import discover_scenarios
from .scenario_runner import run_scenario
from .step_runner import run_step

__all__ = [
    "coverage_report",
    "discover_scenarios",
    "semantic_sandtable_main",
    "run_scenario",
    "run_step",
]
