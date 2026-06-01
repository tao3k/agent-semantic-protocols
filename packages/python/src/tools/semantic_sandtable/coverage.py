"""Coverage audit helpers for semantic sandtable scenarios."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .constants import SCENARIO_SCHEMA_PATH
from .models import (
    CoveragePolicyLoadError,
    CoverageReport,
    CoverageSurface,
    LargeLibraryTarget,
    ScenarioLoadError,
)
from .scenario_io import load_scenario
from .schemas import validate_coverage_policy_schema
from .utils import dict_value, list_value, require_str, string_list


def coverage_report(
    repo_root: Path,
    scenario_paths: list[Path],
    policy_path: Path | None = None,
) -> CoverageReport:
    surfaces: dict[str, CoverageSurface] = {}
    large_library_targets: dict[str, dict[str, LargeLibraryTarget]] = {}
    languages: set[str] = set()
    scenario_count = 0
    errors: list[str] = []
    expected_surfaces = coverage_surfaces_from_schema(repo_root)
    language_expected_surfaces = _load_language_coverage_policy(
        repo_root,
        policy_path,
        expected_surfaces,
        errors,
    )
    scenario_count = sum(
        1
        for path in scenario_paths
        if _add_scenario_coverage(
            repo_root,
            path,
            surfaces,
            large_library_targets,
            languages,
            errors,
        )
    )
    return CoverageReport(
        scenario_count=scenario_count,
        language_ids=languages,
        expected_surfaces=expected_surfaces,
        surfaces=surfaces,
        policy_path=_display_policy_path(repo_root, policy_path),
        language_expected_surfaces=language_expected_surfaces,
        large_library_targets=large_library_targets,
        errors=errors,
    )


def _load_language_coverage_policy(
    repo_root: Path,
    policy_path: Path | None,
    expected_surfaces: list[str],
    errors: list[str],
) -> dict[str, list[str]]:
    if policy_path is None or not policy_path.exists():
        return {}
    try:
        return load_coverage_policy(repo_root, policy_path, expected_surfaces)
    except CoveragePolicyLoadError as error:
        errors.append(str(error))
        return {}


def _add_scenario_coverage(
    repo_root: Path,
    path: Path,
    surfaces: dict[str, CoverageSurface],
    large_library_targets: dict[str, dict[str, LargeLibraryTarget]],
    languages: set[str],
    errors: list[str],
) -> bool:
    try:
        scenario = load_scenario(path, repo_root)
    except ScenarioLoadError as error:
        errors.append(str(error))
        return False
    scenario_id = require_str(scenario, "id", path.stem)
    language = require_str(scenario, "language", "unknown")
    languages.add(language)
    _add_scenario_surfaces(surfaces, scenario, scenario_id, language)
    _add_step_surfaces(surfaces, scenario, scenario_id, language)
    _add_large_library_target(
        large_library_targets,
        scenario,
        scenario_id,
        language,
    )
    return True


def _add_scenario_surfaces(
    surfaces: dict[str, CoverageSurface],
    scenario: dict[str, Any],
    scenario_id: str,
    language: str,
) -> None:
    for surface in string_list(scenario.get("coverage", [])):
        add_coverage_surface(
            surfaces,
            surface,
            scenario_id=scenario_id,
            language=language,
        )


def _add_step_surfaces(
    surfaces: dict[str, CoverageSurface],
    scenario: dict[str, Any],
    scenario_id: str,
    language: str,
) -> None:
    for index, step in enumerate(scenario.get("steps", []), start=1):
        if isinstance(step, dict):
            _add_step_surface_entries(surfaces, step, scenario_id, language, index)


def _add_step_surface_entries(
    surfaces: dict[str, CoverageSurface],
    step: dict[str, Any],
    scenario_id: str,
    language: str,
    index: int,
) -> None:
    step_id = require_str(step, "id", f"step-{index}")
    for surface in string_list(step.get("coverage", [])):
        add_coverage_surface(
            surfaces,
            surface,
            scenario_id=scenario_id,
            language=language,
            step_id=f"{scenario_id}:{step_id}",
        )


def _display_policy_path(repo_root: Path, policy_path: Path | None) -> Path | None:
    if policy_path is None or not policy_path.exists():
        return None
    try:
        return policy_path.relative_to(repo_root)
    except ValueError:
        return policy_path


def add_coverage_surface(
    surfaces: dict[str, CoverageSurface],
    surface: str,
    *,
    scenario_id: str,
    language: str,
    step_id: str | None = None,
) -> None:
    entry = surfaces.setdefault(surface, CoverageSurface(name=surface))
    entry.scenario_ids.add(scenario_id)
    entry.languages.add(language)
    if step_id is not None:
        entry.step_ids.add(step_id)


def _add_large_library_target(
    large_library_targets: dict[str, dict[str, LargeLibraryTarget]],
    scenario: dict[str, Any],
    scenario_id: str,
    language: str,
) -> None:
    evidence = dict_value(scenario.get("evidence"))
    if evidence.get("fixtureTier") != "large-library":
        return
    target_library = dict_value(evidence.get("targetLibrary"))
    package = target_library.get("package")
    name = target_library.get("name")
    if not isinstance(package, str) or not isinstance(name, str):
        return
    language_targets = large_library_targets.setdefault(language, {})
    target = language_targets.setdefault(
        package,
        LargeLibraryTarget(language=language, package=package, name=name),
    )
    target.scenario_ids.add(scenario_id)
    for intent_case in list_value(evidence.get("intentCases")):
        case = dict_value(intent_case)
        intent_kind = case.get("intentKind")
        if isinstance(intent_kind, str):
            target.intent_kinds.add(intent_kind)


def coverage_surfaces_from_schema(repo_root: Path) -> list[str]:
    schema_path = repo_root / SCENARIO_SCHEMA_PATH
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError):
        return []
    values = (
        schema.get("$defs", {})
        .get("coverageList", {})
        .get("items", {})
        .get("enum", [])
    )
    return [item for item in values if isinstance(item, str)]


def load_coverage_policy(
    repo_root: Path,
    path: Path,
    expected_surfaces: list[str],
) -> dict[str, list[str]]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            policy = json.load(handle)
    except OSError as error:
        raise CoveragePolicyLoadError(
            f"failed to read coverage policy: {error}"
        ) from error
    except json.JSONDecodeError as error:
        raise CoveragePolicyLoadError(
            f"failed to parse coverage policy JSON: {error.msg}"
        ) from error

    validate_coverage_policy_schema(repo_root, path, policy)
    if not isinstance(policy, dict):
        raise CoveragePolicyLoadError("coverage policy must be an object")

    expected_set = set(expected_surfaces)
    languages = policy.get("languages", [])
    if not isinstance(languages, list):
        raise CoveragePolicyLoadError("coverage policy languages must be an array")
    result: dict[str, list[str]] = {}
    for index, entry in enumerate(languages):
        if not isinstance(entry, dict):
            raise CoveragePolicyLoadError(
                f"coverage policy languages.{index} must be an object"
            )
        language = entry.get("languageId")
        if not isinstance(language, str):
            raise CoveragePolicyLoadError(
                f"coverage policy languages.{index}.languageId must be a string"
            )
        required = string_list(entry.get("requiredCoverage", []))
        unknown = [surface for surface in required if surface not in expected_set]
        if unknown:
            raise CoveragePolicyLoadError(
                f"coverage policy languageId {language} has unknown surfaces: "
                f"{','.join(unknown)}"
            )
        result[language] = required
    return result
