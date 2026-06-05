"""Stdout JSON expectation validation."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .models import StepResult
from .utils import resolve_path


def validate_stdout_json(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
    repo_root: Path,
) -> None:
    json_expect = _stdout_json_expectation(expect, result)
    if json_expect is None:
        return

    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        result.errors.append(f"stdout JSON parse failed: {error.msg}")
        return

    _validate_stdout_json_schema(json_expect, result, repo_root, payload)
    _validate_stdout_json_equals(json_expect["equals"], result, payload)
    _validate_stdout_json_contains(json_expect["contains"], result, payload)
    _validate_stdout_json_array_contains(
        json_expect["arrayContains"],
        result,
        payload,
    )


def _stdout_json_expectation(
    expect: dict[str, Any],
    result: StepResult,
) -> dict[str, Any] | None:
    json_expect = {
        "equals": _dict_expectation(expect, result, "stdoutJsonEquals"),
        "contains": _dict_expectation(expect, result, "stdoutJsonContains"),
        "arrayContains": _dict_expectation(
            expect,
            result,
            "stdoutJsonArrayContains",
        ),
        "schemaPath": expect.get("stdoutJsonSchema"),
        "schemasAt": _dict_expectation(expect, result, "stdoutJsonSchemaAt"),
    }
    if json_expect["schemaPath"] is not None and not isinstance(
        json_expect["schemaPath"], str
    ):
        result.errors.append("expect.stdoutJsonSchema must be a string")
        json_expect["schemaPath"] = None
    if (
        not json_expect["equals"]
        and not json_expect["contains"]
        and not json_expect["arrayContains"]
        and json_expect["schemaPath"] is None
        and not json_expect["schemasAt"]
    ):
        return None
    return json_expect


def _dict_expectation(
    expect: dict[str, Any],
    result: StepResult,
    key: str,
) -> dict[str, Any]:
    value = expect.get(key, {})
    if isinstance(value, dict):
        return value
    result.errors.append(f"expect.{key} must be an object")
    return {}


def _validate_stdout_json_schema(
    json_expect: dict[str, Any],
    result: StepResult,
    repo_root: Path,
    payload: Any,
) -> None:
    schema_path = json_expect["schemaPath"]
    if schema_path is not None:
        _validate_json_value_against_schema(result, repo_root, payload, "$", schema_path)
    for path, nested_schema_path in json_expect["schemasAt"].items():
        if not isinstance(path, str) or not isinstance(nested_schema_path, str):
            result.errors.append("stdoutJsonSchemaAt entries must be string to string")
            continue
        actual, found = _json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
            continue
        _validate_json_value_against_schema(
            result,
            repo_root,
            actual,
            path,
            nested_schema_path,
        )


def _validate_stdout_json_equals(
    equals: dict[str, Any],
    result: StepResult,
    payload: Any,
) -> None:
    for path, expected in equals.items():
        if not isinstance(path, str):
            result.errors.append("stdoutJsonEquals paths must be strings")
            continue
        actual, found = _json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
        elif actual != expected:
            result.errors.append(
                f"stdout JSON path {path!r}={actual!r} expected={expected!r}"
            )


def _validate_stdout_json_contains(
    contains: dict[str, Any],
    result: StepResult,
    payload: Any,
) -> None:
    for path, needle in contains.items():
        if not isinstance(path, str) or not isinstance(needle, str):
            result.errors.append("stdoutJsonContains entries must be string to string")
            continue
        actual, found = _json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
            continue
        actual_text = (
            actual if isinstance(actual, str) else json.dumps(actual, sort_keys=True)
        )
        if needle not in actual_text:
            result.errors.append(
                f"stdout JSON path {path!r} missing substring {needle!r}"
            )


def _validate_stdout_json_array_contains(
    array_contains: dict[str, Any],
    result: StepResult,
    payload: Any,
) -> None:
    for path, expected in array_contains.items():
        if not isinstance(path, str):
            result.errors.append("stdoutJsonArrayContains paths must be strings")
            continue
        actual, found = _json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
            continue
        if not isinstance(actual, list):
            result.errors.append(f"stdout JSON path {path!r} is not an array")
            continue
        if not any(_json_value_matches(item, expected) for item in actual):
            result.errors.append(
                "stdout JSON array path "
                f"{path!r} missing {json.dumps(expected, sort_keys=True)!r}"
            )


def _json_value_matches(actual: Any, expected: Any) -> bool:
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            return False
        for key, expected_value in expected.items():
            if key not in actual:
                return False
            if not _json_value_matches(actual[key], expected_value):
                return False
        return True
    return actual == expected


def _validate_json_value_against_schema(
    result: StepResult,
    repo_root: Path,
    value: Any,
    value_path: str,
    schema_path_text: str,
) -> None:
    schema_path = resolve_path(repo_root, schema_path_text)
    if schema_path is None or not schema_path.exists():
        result.errors.append(f"stdout JSON schema not found {schema_path_text!r}")
        return
    try:
        from jsonschema import Draft202012Validator
        from referencing import Registry, Resource
    except ImportError:
        result.errors.append("stdout JSON schema validation requires jsonschema")
        return
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        result.errors.append(f"stdout JSON schema load failed {schema_path_text!r}: {error}")
        return
    local_schemas = []
    try:
        for local_schema_path in sorted(schema_path.parent.glob("*.schema.json")):
            with local_schema_path.open("r", encoding="utf-8") as handle:
                local_schemas.append(json.load(handle))
    except (OSError, json.JSONDecodeError) as error:
        result.errors.append(f"stdout JSON schema registry load failed: {error}")
        return
    registry = Registry().with_resources(
        (local_schema["$id"], Resource.from_contents(local_schema))
        for local_schema in local_schemas
        if "$id" in local_schema
    )
    errors = sorted(
        Draft202012Validator(schema, registry=registry).iter_errors(value),
        key=lambda error: list(error.path),
    )
    for error in errors[:3]:
        location = ".".join(str(part) for part in error.path) or "$"
        result.errors.append(
            f"stdout JSON schema {schema_path_text!r} failed at {value_path}.{location}: {error.message}"
        )
    if len(errors) > 3:
        result.errors.append(
            f"stdout JSON schema {schema_path_text!r} has {len(errors) - 3} more failures"
        )


def _json_path(payload: Any, path: str) -> tuple[Any, bool]:
    current = payload
    for part in path.split("."):
        if isinstance(current, dict) and part in current:
            current = current[part]
            continue
        if isinstance(current, list) and part.isdigit():
            index = int(part)
            if index < len(current):
                current = current[index]
                continue
        return None, False
    return current, True
