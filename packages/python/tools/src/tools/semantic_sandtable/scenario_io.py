"""Scenario discovery and loading helpers."""

from __future__ import annotations

import json
import string
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
    if relative.parts[:2] == ("sandtables", "fixtures"):
        return False
    return not any(part.startswith(".") for part in relative.parts)


def load_scenario(path: Path, repo_root: Path | None = None) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            scenario = json.load(handle)
    except OSError as error:
        raise ScenarioLoadError(f"failed to read scenario: {error}") from error
    except json.JSONDecodeError as error:
        raise ScenarioLoadError(
            f"failed to parse scenario JSON: {error.msg}"
        ) from error
    validate_no_absolute_local_paths(path, scenario)
    if repo_root is not None:
        validate_scenario_schema(repo_root, path, scenario)
    return scenario


def validate_no_absolute_local_paths(path: Path, scenario: Any) -> None:
    violations = [
        f"{pointer}: {value!r}"
        for pointer, value in _iter_json_strings(scenario)
        if _contains_absolute_local_path(value)
    ]
    if not violations:
        return
    preview = "\n".join(violations[:8])
    suffix = "" if len(violations) <= 8 else f"\n... and {len(violations) - 8} more"
    raise ScenarioLoadError(
        "sandtable scenarios must be GitHub-portable; absolute local paths are "
        f"not allowed in {path}:\n{preview}{suffix}"
    )


def _iter_json_strings(value: Any, pointer: str = "$") -> list[tuple[str, str]]:
    if isinstance(value, str):
        return [(pointer, value)]
    if isinstance(value, list):
        results: list[tuple[str, str]] = []
        for index, item in enumerate(value):
            results.extend(_iter_json_strings(item, f"{pointer}/{index}"))
        return results
    if isinstance(value, dict):
        results = []
        for key, item in value.items():
            escaped = str(key).replace("~", "~0").replace("/", "~1")
            results.extend(_iter_json_strings(item, f"{pointer}/{escaped}"))
        return results
    return []


def _contains_absolute_local_path(value: str) -> bool:
    stripped = value.strip()
    if stripped.startswith(("file://", "/", "\\\\")):
        return True
    if _contains_windows_absolute_path(stripped):
        return True
    return any(
        marker in value
        for marker in ("/Users/", "/home/", "/private/", "/tmp/", "/var/folders/")
    )


def _contains_windows_absolute_path(value: str) -> bool:
    for index in range(len(value) - 2):
        if (
            value[index] in string.ascii_letters
            and value[index + 1] == ":"
            and value[index + 2] in ("/", "\\")
            and (index == 0 or value[index - 1] in " \t\r\n\"'=([{")
        ):
            return True
    return False
