"""Scenario schema coverage for failure-frontier comparison evidence."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[3]


def test_failure_frontier_receipt_comparison_evidence_is_valid() -> None:
    assert _validation_errors(_scenario("failure-frontier-real-trigger-replay")) == []


def test_failure_frontier_trace_comparison_evidence_is_valid() -> None:
    assert _validation_errors(_scenario("failure-frontier-real-trigger-trace-replay")) == []


def test_failure_frontier_comparison_requires_receipt_or_trace_pair() -> None:
    scenario = _scenario("failure-frontier-real-trigger-trace-replay")
    comparison = _failure_frontier_comparison(scenario)
    comparison.pop("candidateTracePath")

    errors = _validation_errors(scenario)

    assert any("is not valid under any of the given schemas" in error for error in errors)


def test_failure_frontier_comparison_rejects_unknown_threshold() -> None:
    scenario = _scenario("failure-frontier-real-trigger-replay")
    thresholds = _failure_frontier_comparison(scenario)["thresholds"]
    assert isinstance(thresholds, dict)
    thresholds["maxRawSourceWindows"] = 4

    errors = _validation_errors(scenario)

    assert any("Additional properties are not allowed" in error for error in errors)


def _scenario(name: str) -> dict[str, object]:
    return _load_json(_REPO_ROOT / "sandtables" / "fixtures" / "asp" / f"{name}.json")


def _failure_frontier_comparison(
    scenario: dict[str, object],
) -> dict[str, object]:
    evidence = scenario["evidence"]
    assert isinstance(evidence, dict)
    comparison = evidence["failureFrontierComparison"]
    assert isinstance(comparison, dict)
    return comparison


def _validation_errors(scenario: dict[str, object]) -> list[str]:
    schema = _load_json(
        _REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
    )
    validator = Draft202012Validator(schema)
    return [error.message for error in validator.iter_errors(scenario)]


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))
