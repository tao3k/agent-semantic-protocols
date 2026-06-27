from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


REPO_ROOT = Path(__file__).resolve().parents[3]
SCHEMA_DIR = REPO_ROOT / "schemas"
FIXTURE_DIR = REPO_ROOT / "tests" / "fixtures" / "semantic_sandtable"
FEEDBACK_FIXTURE = FIXTURE_DIR / "route-user-feedback-dataset.v1.json"
PATTERN_FIXTURE = FIXTURE_DIR / "route-monitor-pattern-set.v1.json"
HOST_ABSOLUTE_PATH_RE = re.compile(
    r"(^|\s)(/(Users|home|private|Volumes)/|[A-Za-z]:[\\/])"
)


def test_route_user_feedback_dataset_fixture_is_schema_valid() -> None:
    feedback = _load_json(FEEDBACK_FIXTURE)
    _validator("semantic-route-user-feedback-dataset.v1.schema.json").validate(
        feedback
    )

    assert feedback["datasetVersion"] == "2026-06-27"
    assert {entry["feedbackReason"] for entry in feedback["entries"]} >= {
        "inefficiency",
        "overaction",
        "communication",
    }


def test_user_feedback_entries_are_linked_to_monitor_patterns() -> None:
    feedback = _load_json(FEEDBACK_FIXTURE)
    patterns = _load_json(PATTERN_FIXTURE)

    feedback_ids = {entry["id"] for entry in feedback["entries"]}
    pattern_ids = {pattern["id"] for pattern in patterns["patterns"]}
    pattern_feedback_refs = {
        ref.removeprefix("route-feedback:")
        for pattern in patterns["patterns"]
        for ref in pattern.get("evidenceRefs", [])
        if ref.startswith("route-feedback:")
    }

    assert pattern_feedback_refs <= feedback_ids
    for entry in feedback["entries"]:
        assert set(entry["patternIds"]) <= pattern_ids
    assert {"avoid-prime-when-owner-known", "line-range-is-display-hint"} <= (
        pattern_feedback_refs
    )


def test_route_user_feedback_dataset_has_no_host_absolute_paths() -> None:
    feedback = _load_json(FEEDBACK_FIXTURE)

    assert list(_host_absolute_path_leaks(feedback)) == []


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


def _host_absolute_path_leaks(value: Any, path: str = "$") -> list[str]:
    if isinstance(value, dict):
        leaks: list[str] = []
        for key, child in value.items():
            leaks.extend(_host_absolute_path_leaks(child, f"{path}.{key}"))
        return leaks
    if isinstance(value, list):
        leaks = []
        for index, child in enumerate(value):
            leaks.extend(_host_absolute_path_leaks(child, f"{path}[{index}]"))
        return leaks
    if isinstance(value, str) and HOST_ABSOLUTE_PATH_RE.search(value):
        return [path]
    return []
