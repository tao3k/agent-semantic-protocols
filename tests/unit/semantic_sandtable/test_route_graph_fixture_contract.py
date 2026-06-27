from __future__ import annotations

import json
import unittest
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, RefResolver


_REPO_ROOT = Path(__file__).resolve().parents[3]
_SCHEMA_DIR = _REPO_ROOT / "schemas"
_FIXTURE = (
    _REPO_ROOT
    / "tests"
    / "fixtures"
    / "semantic_sandtable"
    / "asp-route-known-owner-skips-prime.v1.json"
)


def _load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def _scenario_validator() -> Draft202012Validator:
    schema = _load_json(_SCHEMA_DIR / "semantic-sandtable-scenario.v1.schema.json")
    route_trace_schema = _load_json(_SCHEMA_DIR / "semantic-route-verification-trace.v1.schema.json")
    resolver = RefResolver.from_schema(
        schema,
        store={
            route_trace_schema["$id"]: route_trace_schema,
            "semantic-route-verification-trace.v1.schema.json": route_trace_schema,
        },
    )
    return Draft202012Validator(schema, resolver=resolver)


def _absolute_string_values(value: Any, path: str = "$") -> list[str]:
    if isinstance(value, dict):
        leaks: list[str] = []
        for key, child in value.items():
            leaks.extend(_absolute_string_values(child, f"{path}.{key}"))
        return leaks
    if isinstance(value, list):
        leaks = []
        for index, child in enumerate(value):
            leaks.extend(_absolute_string_values(child, f"{path}[{index}]"))
        return leaks
    if isinstance(value, str) and value.startswith("/"):
        return [f"{path}={value}"]
    return []


class SemanticSandtableRouteGraphFixtureContractTests(unittest.TestCase):
    def test_known_owner_route_fixture_is_schema_valid(self) -> None:
        scenario = _load_json(_FIXTURE)
        errors = [error.message for error in _scenario_validator().iter_errors(scenario)]
        self.assertEqual([], errors)

    def test_known_owner_route_fixture_has_no_absolute_paths(self) -> None:
        scenario = _load_json(_FIXTURE)
        self.assertEqual([], _absolute_string_values(scenario))


if __name__ == "__main__":
    unittest.main()
