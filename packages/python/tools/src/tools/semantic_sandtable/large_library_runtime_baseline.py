"""Validate a large-library runtime receipt against its recorded V1 baseline."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import sys
from typing import Any


BASELINE_SCHEMA_ID = "agent.semantic-protocols.large-library-runtime-search-baseline"
BASELINE_SCHEMA_VERSION = "1"


def validate_runtime_baseline(
    baseline: dict[str, Any], receipt: dict[str, Any]
) -> dict[str, Any]:
    """Return a strict, machine-readable baseline comparison report."""
    errors: list[str] = []
    if baseline.get("schemaId") != BASELINE_SCHEMA_ID:
        errors.append("baseline-schema-id")
    if baseline.get("schemaVersion") != BASELINE_SCHEMA_VERSION:
        errors.append("baseline-schema-version")
    if receipt.get("status") != "pass":
        errors.append("runtime-receipt-status")

    budget = dict_value(baseline.get("budget"))
    max_command_elapsed = positive_int(budget.get("maxCommandElapsedMs"))
    coverage = dict_value(baseline.get("commandCoverage"))
    actual_coverage = dict_value(receipt.get("commandCoverage"))
    for key in (
        "registeredSearchMethodCount",
        "runtimeSearchCommandCount",
        "targetSearchCommandCount",
    ):
        if coverage.get(key) != actual_coverage.get(key):
            errors.append(f"coverage-{key}")
    if actual_coverage.get("missingMethods"):
        errors.append("coverage-missing-methods")
    if actual_coverage.get("missingCorpusMethods"):
        errors.append("coverage-missing-corpus-methods")

    expected_deployments = {
        str(entry["language"])
        for entry in list_value(baseline.get("workspaceDeployments"))
        if isinstance(entry.get("language"), str)
    }
    actual_deployments = {
        str(entry["language"]): entry
        for entry in list_value(receipt.get("workspaceDeployments"))
        if isinstance(entry.get("language"), str)
    }
    if expected_deployments != set(actual_deployments):
        errors.append("workspace-deployment-languages")
    if any(entry.get("status") != "pass" for entry in actual_deployments.values()):
        errors.append("workspace-deployment-status")

    expected_scenarios: dict[str, tuple[int | None, int | None]] = {}
    for entry in list_value(baseline.get("scenarios")):
        scenario_id = entry.get("id")
        if not isinstance(scenario_id, str):
            continue
        expected_scenarios[scenario_id] = (
            calibrated_scenario_elapsed(entry),
            positive_int(entry.get("commands")),
        )
    actual_scenarios = scenario_max_elapsed(receipt)
    actual_command_counts = scenario_command_counts(receipt)
    if set(expected_scenarios) != set(actual_scenarios):
        errors.append("scenario-set")
    scenario_budgets: dict[str, int] = {}
    scenario_references: dict[str, int] = {}
    for scenario_id, (baseline_elapsed, expected_commands) in expected_scenarios.items():
        if baseline_elapsed is None or expected_commands is None or max_command_elapsed is None:
            errors.append(f"scenario-baseline-{scenario_id}")
            continue
        scenario_references[scenario_id] = baseline_elapsed
        if actual_command_counts.get(scenario_id) != expected_commands:
            errors.append(f"scenario-command-count-{scenario_id}")
        allowed = min(max_command_elapsed, max(5_000, math.ceil(baseline_elapsed * 1.5)))
        scenario_budgets[scenario_id] = allowed
        actual = actual_scenarios.get(scenario_id)
        if actual is None or actual > allowed:
            errors.append(f"scenario-budget-{scenario_id}")

    validate_corpus_identity(baseline, receipt, errors)

    return {
        "schemaId": BASELINE_SCHEMA_ID,
        "schemaVersion": BASELINE_SCHEMA_VERSION,
        "status": "pass" if not errors else "fail",
        "errors": errors,
        "scenarioBudgetsMs": scenario_budgets,
        "scenarioReferenceMs": scenario_references,
    }


def scenario_max_elapsed(receipt: dict[str, Any]) -> dict[str, int]:
    result: dict[str, int] = {}
    for step in list_value(receipt.get("steps")):
        scenario_id = step.get("scenarioId")
        elapsed = positive_int(step.get("elapsedMs"))
        if not isinstance(scenario_id, str) or elapsed is None:
            continue
        result[scenario_id] = max(result.get(scenario_id, 0), elapsed)
    return result


def scenario_command_counts(receipt: dict[str, Any]) -> dict[str, int]:
    result: dict[str, int] = {}
    for step in list_value(receipt.get("steps")):
        scenario_id = step.get("scenarioId")
        if isinstance(scenario_id, str):
            result[scenario_id] = result.get(scenario_id, 0) + 1
    return result


def calibrated_scenario_elapsed(entry: dict[str, Any]) -> int | None:
    observations = entry.get("observationsMs")
    if not isinstance(observations, list) or not observations:
        return None
    values = [positive_int(value) for value in observations]
    if any(value is None for value in values):
        return None
    reference = positive_int(entry.get("maxElapsedMs"))
    maximum = max(value for value in values if value is not None)
    return reference if reference == maximum else None


def validate_corpus_identity(
    baseline: dict[str, Any], receipt: dict[str, Any], errors: list[str]
) -> None:
    expected = {
        str(entry["scenarioId"]): corpus_identity(entry)
        for entry in list_value(baseline.get("corpora"))
        if isinstance(entry.get("scenarioId"), str)
    }
    actual = {
        str(entry["scenarioId"]): corpus_identity(entry)
        for entry in list_value(receipt.get("corpora"))
        if isinstance(entry.get("scenarioId"), str)
    }
    if set(expected) != set(actual):
        errors.append("corpus-set")
        return
    for scenario_id, identity in expected.items():
        if identity is None or actual.get(scenario_id) != identity:
            errors.append(f"corpus-identity-{scenario_id}")


def corpus_identity(entry: dict[str, Any]) -> tuple[str, str, str, str] | None:
    language = entry.get("language")
    repository = entry.get("repository")
    revision = entry.get("revision")
    directory = entry.get("directory")
    if not isinstance(directory, str) or not directory:
        path = entry.get("path")
        directory = Path(path).name if isinstance(path, str) else None
    if not all(
        isinstance(value, str) and value
        for value in (language, repository, revision, directory)
    ):
        return None
    return language, repository, revision, directory


def dict_value(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def list_value(value: Any) -> list[dict[str, Any]]:
    return [entry for entry in value if isinstance(entry, dict)] if isinstance(value, list) else []


def positive_int(value: Any) -> int | None:
    return value if isinstance(value, int) and value >= 0 else None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    receipt = json.loads(args.receipt.read_text(encoding="utf-8"))
    report = validate_runtime_baseline(dict_value(baseline), dict_value(receipt))
    sys.stdout.write(f"{json.dumps(report, sort_keys=True)}\n")
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
