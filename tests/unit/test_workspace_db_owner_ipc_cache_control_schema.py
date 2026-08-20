import json
from pathlib import Path
from typing import Iterator

from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas" / "workspace-db-owner-ipc.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)
REQUEST_VALIDATOR = Draft202012Validator(SCHEMA["$defs"]["cacheControlRequest"])
FIXTURES = ROOT / "schemas" / "fixtures" / "workspace-db-owner-ipc"


def fixture(name: str) -> object:
    return json.loads((FIXTURES / name).read_text())


def error_tree(error: ValidationError) -> Iterator[ValidationError]:
    yield error
    for child in error.context:
        yield from error_tree(child)


def test_cache_control_stale_failure_fixture_is_valid() -> None:
    errors = list(
        VALIDATOR.iter_errors(fixture("valid-cache-control-stale-failure.v1.json"))
    )

    assert errors == []


def test_cache_control_empty_failure_fixture_is_invalid() -> None:
    errors = list(
        VALIDATOR.iter_errors(fixture("invalid-cache-control-empty-failure.v1.json"))
    )

    assert errors
    descendants = [child for error in errors for child in error_tree(error)]
    assert any(
        child.validator == "minLength"
        and list(child.absolute_path)[-1:] == ["failure"]
        for child in descendants
    )


def test_cache_control_owner_delta_request_is_valid() -> None:
    request = {
        "action": "apply-owner-delta",
        "projectRoot": "/workspace",
        "mutationId": "cache:owner-delta:1",
        "changedPaths": ["src/lib.rs"],
        "removedPaths": ["src/legacy.rs"],
        "fallbackPolicy": "full-generation",
    }

    assert list(REQUEST_VALIDATOR.iter_errors(request)) == []


def test_cache_control_owner_delta_rejects_empty_delta() -> None:
    request = {
        "action": "apply-owner-delta",
        "projectRoot": "/workspace",
        "mutationId": "cache:owner-delta:empty",
        "changedPaths": [],
        "removedPaths": [],
        "fallbackPolicy": "fail-closed",
    }

    assert list(REQUEST_VALIDATOR.iter_errors(request))
