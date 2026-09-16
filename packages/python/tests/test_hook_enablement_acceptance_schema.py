# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate the fail-closed Hook enablement Ready receipt."""

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


REPO_ROOT = Path(__file__).parents[3]


def schema_and_fixture() -> tuple[dict, dict]:
    schema = json.loads(
        (REPO_ROOT / "schemas/hook-enablement-acceptance.schema.json").read_text()
    )
    fixture = json.loads(
        (
            REPO_ROOT
            / "schemas/fixtures/hook-enablement-acceptance/ready.json"
        ).read_text()
    )
    return schema, fixture


def test_ready_hook_enablement_receipt_is_valid() -> None:
    schema, fixture = schema_and_fixture()
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(fixture)


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("policyDecisionMaxCpuMicros", 1000),
        ("launcherP99Micros", 50000),
        ("launcherMaxMicros", 100000),
    ],
)
def test_hook_enablement_rejects_any_budget_boundary_or_overrun(
    field: str, value: int
) -> None:
    schema, fixture = schema_and_fixture()
    fixture[field] = value
    with pytest.raises(ValidationError):
        Draft202012Validator(schema).validate(fixture)
