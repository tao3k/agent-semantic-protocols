"""Scenario discovery and loading helpers."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .constants import COVERAGE_POLICY_PATH, DEFAULT_SCENARIO_GLOB
from .models import ScenarioLoadError
from .schemas import validate_scenario_schema


def discover_scenarios(repo_root: Path, scenario_args: list[str]) -> list[Path]:
    if scenario_args:
        paths = [Path(arg).expanduser() for arg in scenario_args]
        return [path if path.is_absolute() else repo_root / path for path in paths]
    matches = sorted(repo_root.glob(DEFAULT_SCENARIO_GLOB))
    return [path for path in matches if discoverable_scenario_path(repo_root, path)]


def discoverable_scenario_path(repo_root: Path, path: Path) -> bool:
    if not path.is_file():
        return False
    try:
        relative = path.relative_to(repo_root)
    except ValueError:
        relative = path
    if relative == COVERAGE_POLICY_PATH:
        return False
    if "receipts" in relative.parts:
        return False
    return not any(part.startswith(".") for part in relative.parts)


def load_scenario(path: Path, repo_root: Path | None = None) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            scenario = json.load(handle)
    except OSError as error:
        raise ScenarioLoadError(f"failed to read scenario: {error}") from error
    except json.JSONDecodeError as error:
        raise ScenarioLoadError(f"failed to parse scenario JSON: {error.msg}") from error
    if repo_root is not None:
        validate_scenario_schema(repo_root, path, scenario)
    return scenario
