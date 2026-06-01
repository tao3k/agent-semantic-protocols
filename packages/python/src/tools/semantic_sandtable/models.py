"""Result and error models for semantic sandtable execution."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .constants import (
    LARGE_LIBRARY_INTENT_KINDS,
    LARGE_LIBRARY_MIN_TARGETS_PER_LANGUAGE,
)


class ScenarioLoadError(Exception):
    """A scenario file is not valid enough to execute."""


class CoveragePolicyLoadError(Exception):
    """The coverage policy cannot be used for audit reporting."""


class ReceiptLoadError(Exception):
    """A real-trigger receipt cannot be validated."""


@dataclass
class StepResult:
    scenario_id: str
    step_id: str
    command: list[str]
    status: str
    exit_code: int | None
    elapsed_ms: int
    stdout_lines: int
    stderr_lines: int
    stdout_bytes: int
    stderr_bytes: int
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)


@dataclass
class ScenarioResult:
    scenario_id: str
    language: str
    path: Path
    status: str
    workdir: Path | None
    coverage: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    evidence: dict[str, Any] = field(default_factory=dict)
    workdir_spec: Any = None
    steps: list[StepResult] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    skip_reason: str | None = None


@dataclass
class RuntimeAuditFinding:
    kind: str
    severity: str
    scenario_id: str
    message: str
    action: str
    step_id: str | None = None


@dataclass
class CoverageSurface:
    name: str
    scenario_ids: set[str] = field(default_factory=set)
    languages: set[str] = field(default_factory=set)
    step_ids: set[str] = field(default_factory=set)


@dataclass
class LargeLibraryTarget:
    language: str
    package: str
    name: str
    scenario_ids: set[str] = field(default_factory=set)
    intent_kinds: set[str] = field(default_factory=set)


@dataclass
class CoverageReport:
    scenario_count: int
    language_ids: set[str]
    expected_surfaces: list[str]
    surfaces: dict[str, CoverageSurface]
    policy_path: Path | None = None
    language_expected_surfaces: dict[str, list[str]] = field(default_factory=dict)
    large_library_targets: dict[str, dict[str, LargeLibraryTarget]] = field(
        default_factory=dict
    )
    errors: list[str] = field(default_factory=list)

    @property
    def missing(self) -> list[str]:
        return [
            surface
            for surface in self.expected_surfaces
            if surface not in self.surfaces
        ]

    @property
    def language_missing(self) -> dict[str, list[str]]:
        missing: dict[str, list[str]] = {}
        for language, expected in self.language_expected_surfaces.items():
            covered = self.covered_surfaces_for_language(language)
            language_missing = [surface for surface in expected if surface not in covered]
            if language_missing:
                missing[language] = language_missing
        return missing

    @property
    def large_library_missing(self) -> dict[str, list[str]]:
        missing: dict[str, list[str]] = {}
        required_intents = set(LARGE_LIBRARY_INTENT_KINDS)
        for language, expected in self.language_expected_surfaces.items():
            if "large-library" not in expected:
                continue
            targets = self.large_library_targets.get(language, {})
            language_missing: list[str] = []
            if len(targets) < LARGE_LIBRARY_MIN_TARGETS_PER_LANGUAGE:
                language_missing.append(
                    "libraries="
                    f"{len(targets)}/{LARGE_LIBRARY_MIN_TARGETS_PER_LANGUAGE}"
                )
            for package, target in sorted(targets.items()):
                missing_intents = sorted(required_intents - target.intent_kinds)
                if missing_intents:
                    language_missing.append(
                        f"{package}:intents={','.join(missing_intents)}"
                    )
            if language_missing:
                missing[language] = language_missing
        return missing

    def covered_surfaces_for_language(self, language: str) -> set[str]:
        return {
            name
            for name, surface in self.surfaces.items()
            if language in surface.languages
        }


@dataclass
class ReceiptResult:
    path: Path
    status: str
    scenario_id: str = "unknown"
    language: str = "unknown"
    command_count: int = 0
    stdout_bytes: int = 0
    stderr_bytes: int = 0
    elapsed_ms: int = 0
    json_searches: int = 0
    compact_searches: int = 0
    token_cost: dict[str, Any] = field(default_factory=dict)
    command_token_costs: list[dict[str, Any]] = field(default_factory=list)
    query_set_opportunities: list[dict[str, Any]] = field(default_factory=list)
    findings: list[dict[str, Any]] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)


def has_warnings(result: ScenarioResult) -> bool:
    return bool(result.warnings or any(step.warnings for step in result.steps))
