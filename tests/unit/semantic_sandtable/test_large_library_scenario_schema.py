"""Scenario schema coverage for large-library evidence."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[3]


def test_large_library_evidence_requires_complete_matrix_metadata() -> None:
    assert _validation_errors(_large_library_scenario()) == []


def test_large_library_evidence_requires_coverage_and_repository() -> None:
    scenario = _large_library_scenario()
    scenario.pop("coverage")
    target = _target_library(scenario)
    target.pop("repository")

    errors = _validation_errors(scenario)

    assert "'coverage' is a required property" in errors
    assert "'repository' is a required property" in errors


def test_large_library_evidence_rejects_unknown_workdir_kind() -> None:
    scenario = _large_library_scenario()
    _target_library(scenario)["workdirKind"] = "unknown"

    errors = _validation_errors(scenario)

    assert "'unknown' is not one of ['checkout', 'registry']" in errors


def _target_library(scenario: dict[str, object]) -> dict[str, object]:
    evidence = scenario["evidence"]
    assert isinstance(evidence, dict)
    target = evidence["targetLibrary"]
    assert isinstance(target, dict)
    return target


def _validation_errors(scenario: dict[str, object]) -> list[str]:
    schema = _load_json(
        _REPO_ROOT / "schemas" / "semantic-sandtable-scenario.v1.schema.json"
    )
    validator = Draft202012Validator(schema)
    return [error.message for error in validator.iter_errors(scenario)]


def _load_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _large_library_scenario() -> dict[str, object]:
    return {
        "id": "python.demo-large-library",
        "language": "python",
        "coverage": ["large-library"],
        "workdir": ".",
        "evidence": {
            "source": "handwritten",
            "fixtureTier": "large-library",
            "targetLibrary": {
                "language": "python",
                "name": "demo",
                "package": "demo",
                "repository": "example/demo",
                "workdirKind": "checkout",
            },
            "intentCases": [
                _intent_case("feature-implementation", "feature", "Feature"),
                _intent_case("api-usage", "api", "Api"),
                _intent_case(
                    "implementation-principle",
                    "principle",
                    "Principle",
                ),
            ],
        },
        "steps": [
            {
                "id": "intent-query-set",
                "command": [
                    "py-harness",
                    "search",
                    "lexical",
                    "--query-set",
                    "Feature",
                    "--query-set",
                    "Api",
                    "--query-set",
                    "Principle",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ],
            }
        ],
    }


def _intent_case(intent_kind: str, intent: str, query_term: str) -> dict[str, object]:
    return {
        "intentKind": intent_kind,
        "intent": intent,
        "stepIds": ["intent-query-set"],
        "queryTerms": [query_term],
    }
