# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate candidate-bound Runtime Server generation admission receipts."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


REPO_ROOT = Path(__file__).parents[3]
SCHEMA_PATH = (
    REPO_ROOT
    / "schemas"
    / "runtime-server-workspace-generation-admission.schema.json"
)
CANDIDATE_SCHEMA_PATH = (
    REPO_ROOT / "schemas" / "repository-candidate-snapshot.v1.schema.json"
)
DEFINITIONS_SCHEMA_PATH = REPO_ROOT / "schemas" / "runtime-server-definitions.v1.schema.json"
FIXTURE_ROOT = (
    REPO_ROOT
    / "schemas"
    / "fixtures"
    / "runtime-server-workspace-generation-admission"
)
MUTATION_SCHEMA_PATH = (
    REPO_ROOT
    / "schemas"
    / "workspace-generation-mutation-admission-receipt.v1.schema.json"
)
MUTATION_FIXTURE_ROOT = (
    REPO_ROOT
    / "schemas"
    / "fixtures"
    / "workspace-generation-mutation-admission-receipt"
)


def validator() -> Draft202012Validator:
    schema = json.loads(SCHEMA_PATH.read_text())
    candidate_schema = json.loads(CANDIDATE_SCHEMA_PATH.read_text())
    definitions_schema = json.loads(DEFINITIONS_SCHEMA_PATH.read_text())
    Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources(
        [(candidate_schema["$id"], Resource.from_contents(candidate_schema)),
         (definitions_schema["$id"], Resource.from_contents(definitions_schema)),
         ("https://agent-semantic-protocols.dev/schemas/runtime-server-definitions.v1.schema.json", Resource.from_contents(definitions_schema))]
    )
    return Draft202012Validator(schema, registry=registry)


def mutation_validator() -> Draft202012Validator:
    schema = json.loads(MUTATION_SCHEMA_PATH.read_text())
    admission_schema = json.loads(SCHEMA_PATH.read_text())
    candidate_schema = json.loads(CANDIDATE_SCHEMA_PATH.read_text())
    definitions_schema = json.loads(DEFINITIONS_SCHEMA_PATH.read_text())
    Draft202012Validator.check_schema(schema)
    registry = Registry().with_resources(
        [
            (
                admission_schema["$id"],
                Resource.from_contents(admission_schema),
            ),
            (
                candidate_schema["$id"],
                Resource.from_contents(candidate_schema),
            ),
            (definitions_schema["$id"], Resource.from_contents(definitions_schema)),
            ("https://agent-semantic-protocols.dev/schemas/runtime-server-definitions.v1.schema.json", Resource.from_contents(definitions_schema)),
        ]
    )
    return Draft202012Validator(schema, registry=registry)


def test_ready_admission_binds_repository_candidate_generation() -> None:
    fixture = json.loads((FIXTURE_ROOT / "valid-ready.json").read_text())
    validator().validate(fixture)


def test_ready_admission_requires_commit_evidence() -> None:
    fixture = json.loads(
        (FIXTURE_ROOT / "invalid-ready-without-commit.json").read_text()
    )
    assert list(validator().iter_errors(fixture))


def test_mutation_admission_resolves_candidate_bound_receipts() -> None:
    fixture = json.loads(
        (MUTATION_FIXTURE_ROOT / "valid-multi-workspace.json").read_text()
    )
    mutation_validator().validate(fixture)
