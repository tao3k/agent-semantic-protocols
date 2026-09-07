# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
import importlib.util
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[2]
PROJECT_RESOLUTION_TEST = Path(__file__).with_name(
    "test_repository_project_resolution_schemas.py"
)
PROJECT_RESOLUTION_SPEC = importlib.util.spec_from_file_location(
    "repository_project_resolution_schemas", PROJECT_RESOLUTION_TEST
)
assert (
    PROJECT_RESOLUTION_SPEC is not None and PROJECT_RESOLUTION_SPEC.loader is not None
)
PROJECT_RESOLUTION_MODULE = importlib.util.module_from_spec(PROJECT_RESOLUTION_SPEC)
PROJECT_RESOLUTION_SPEC.loader.exec_module(PROJECT_RESOLUTION_MODULE)
git_candidates = PROJECT_RESOLUTION_MODULE.git_candidates


def load_schema(name: str) -> dict:
    return json.loads((ROOT / "schemas" / name).read_text())


def validator(schema: dict, *dependencies: dict) -> Draft202012Validator:
    registry = Registry()
    for dependency in dependencies:
        registry = registry.with_resource(
            dependency["$id"], Resource.from_contents(dependency)
        )
    return Draft202012Validator(schema, registry=registry)


def test_project_resolution_request_accepts_repository_candidate_snapshot() -> None:
    request_schema = load_schema("provider-project-resolution-request.v1.schema.json")
    candidate_schema = load_schema("repository-candidate-snapshot.v1.schema.json")
    request = {
        "schemaId": "agent.semantic-protocols.provider-project-resolution-request",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "workspaceRoot": "/workspace",
        "repositoryCandidates": git_candidates(),
    }

    validator(request_schema, candidate_schema).validate(request)


def test_project_resolution_failure_is_typed_and_actionable() -> None:
    response_schema = load_schema("provider-project-resolution-response.v1.schema.json")
    response = {
        "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "state": "failed",
        "failure": {
            "reasonKind": "project-entry-missing",
            "message": "Cargo.toml is not a repository candidate",
            "nextAction": "refresh-repository-candidates-or-select-project-entry",
        },
    }

    validator(response_schema).validate(response)


def test_project_resolution_failure_rejects_missing_next_action() -> None:
    response_schema = load_schema("provider-project-resolution-response.v1.schema.json")
    response = {
        "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "asp-rust",
        "state": "failed",
        "failure": {
            "reasonKind": "project-entry-missing",
            "message": "Cargo.toml is not a repository candidate",
        },
    }

    errors = list(validator(response_schema).iter_errors(response))
    assert errors
