from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from tools.semantic_sandtable.route_verification import build_route_verification_trace
from tools.semantic_sandtable.route_verification_patterns import (
    MONITOR_PATTERN_SET_VERSION,
    ROUTE_MONITOR_PATTERNS,
    feedback_reason_for_risk,
)


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_DIR = REPO_ROOT / "schemas"
PATTERN_SET_FIXTURE = (
    REPO_ROOT
    / "tests"
    / "fixtures"
    / "semantic_sandtable"
    / "route-monitor-pattern-set.v1.json"
)


def test_route_monitor_pattern_set_fixture_matches_runtime() -> None:
    fixture = _load_json(PATTERN_SET_FIXTURE)
    _validator("semantic-route-monitor-pattern-set.v1.schema.json").validate(fixture)

    runtime_patterns = [dict(pattern) for pattern in ROUTE_MONITOR_PATTERNS]
    assert fixture["monitorPatternSetVersion"] == MONITOR_PATTERN_SET_VERSION
    assert fixture["patterns"] == runtime_patterns


def test_route_monitor_pattern_set_drives_trace_version_and_feedback() -> None:
    trace = build_route_verification_trace(
        [_prime_command()],
        {
            "expectedEvidenceAnchors": ["owner-path"],
            "allowedFirstRoutes": ["owner-items"],
            "forbiddenRiskFlags": ["unnecessary-prime"],
        },
    )

    assert trace["monitorPatternSetVersion"] == MONITOR_PATTERN_SET_VERSION
    assert feedback_reason_for_risk("unnecessary-prime") == "inefficiency"
    assert any(signal["reason"] == "inefficiency" for signal in trace["feedbackSignals"])


def _prime_command() -> dict[str, Any]:
    return {
        "id": "search-prime",
        "kind": "search",
        "argv": ["asp", "rust", "search", "prime", "--workspace", "."],
        "metrics": {"elapsedMs": 1, "stdoutBytes": 10, "stderrBytes": 0},
    }


def _validator(name: str) -> Draft202012Validator:
    return Draft202012Validator(_load_schema(name), registry=_schema_registry())


def _load_schema(name: str) -> dict[str, Any]:
    return _load_json(SCHEMA_DIR / name)


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _schema_registry() -> Registry:
    resources = []
    for schema_path in SCHEMA_DIR.glob("*.schema.json"):
        schema = _load_json(schema_path)
        schema_id = schema.get("$id")
        if schema_id:
            resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)
