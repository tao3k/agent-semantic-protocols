# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
SCHEMA_ROOT = REPOSITORY_ROOT / "schemas"
FIXTURE_ROOT = REPOSITORY_ROOT / "tests" / "fixtures" / "semantic_polyglot_search"


CASES = (
    (
        "semantic-polyglot-search-document.v1.schema.json",
        "document.valid.json",
        True,
        None,
    ),
    (
        "semantic-polyglot-search-document.v1.schema.json",
        "document.mutation.invalid.json",
        False,
        ("sections", "gql", "clauses", 0),
    ),
    (
        "semantic-search-relation-batch.v1.schema.json",
        "relation-batch.valid.json",
        True,
        None,
    ),
    (
        "semantic-search-relation-batch.v1.schema.json",
        "relation-batch.abi-drift.invalid.json",
        False,
        ("abiVersion",),
    ),
    (
        "semantic-progressive-search-turn.v1.schema.json",
        "turn.valid.json",
        True,
        None,
    ),
    (
        "semantic-progressive-search-turn.v1.schema.json",
        "turn.selection-over-budget.invalid.json",
        False,
        ("selectedCandidateIds",),
    ),
    (
        "semantic-search-replacement-certificate.v1.schema.json",
        "replacement.valid.json",
        True,
        None,
    ),
    (
        "semantic-search-replacement-certificate.v1.schema.json",
        "replacement.unsound.invalid.json",
        False,
        ("safety", "unsoundClosureCount"),
    ),
)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    ("schema_name", "fixture_name", "expected_valid", "expected_error_path"),
    CASES,
)
def test_semantic_polyglot_search_fixture(
    schema_name: str,
    fixture_name: str,
    expected_valid: bool,
    expected_error_path: tuple[str | int, ...] | None,
) -> None:
    schema = load_json(SCHEMA_ROOT / schema_name)
    fixture = load_json(FIXTURE_ROOT / fixture_name)
    Draft202012Validator.check_schema(schema)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(fixture),
        key=lambda error: tuple(str(part) for part in error.absolute_path),
    )

    assert (not errors) is expected_valid
    if expected_error_path is not None:
        assert any(tuple(error.absolute_path) == expected_error_path for error in errors)
