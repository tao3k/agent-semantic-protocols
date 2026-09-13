# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path
from typing import Iterator

from jsonschema.exceptions import ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "workspace-db-owner-ipc.v1.schema.json"
from unit.schema_validation import schema_validator_for

VALIDATOR = schema_validator_for(SCHEMA_PATH)
FIXTURES = ROOT / "schemas" / "fixtures" / "workspace-db-owner-ipc"


def fixture(name: str) -> object:
    return json.loads((FIXTURES / name).read_text())


def owner_request(cache_control_request: dict[str, object]) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.workspace-db-owner-request.v1",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-test",
        "transportContractDigest": "blake3-256:test-contract",
        "ownerEpoch": 9,
        "bindingToken": "binding-test",
        "requestId": "request-test",
        "operation": {"kind": "cache-control", "request": cache_control_request},
    }


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

    assert list(VALIDATOR.iter_errors(owner_request(request))) == []


def test_cache_control_owner_delta_rejects_empty_delta() -> None:
    request = {
        "action": "apply-owner-delta",
        "projectRoot": "/workspace",
        "mutationId": "cache:owner-delta:empty",
        "changedPaths": [],
        "removedPaths": [],
        "fallbackPolicy": "fail-closed",
    }

    assert list(VALIDATOR.iter_errors(owner_request(request)))
