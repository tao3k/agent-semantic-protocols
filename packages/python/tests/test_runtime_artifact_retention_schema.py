# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate bounded runtime artifact retention receipts."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).parents[3]


def test_pruned_runtime_artifact_receipt_is_valid() -> None:
    schema = json.loads(
        (REPO_ROOT / "schemas/runtime-artifact-retention-receipt.schema.json").read_text()
    )
    fixture = json.loads(
        (
            REPO_ROOT
            / "schemas/fixtures/runtime-artifact-retention-receipt/valid-pruned.json"
        ).read_text()
    )
    Draft202012Validator.check_schema(schema)
    Draft202012Validator(schema).validate(fixture)
