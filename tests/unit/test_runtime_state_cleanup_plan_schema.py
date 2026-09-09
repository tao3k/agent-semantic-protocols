# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator


COMMIT_SCHEMA_PATH = Path(
    "schemas/runtime-state-cleanup-commit-receipt.v1.schema.json"
)


def test_cleanup_commit_receipt_has_an_independent_closed_identity() -> None:
    schema = json.loads(COMMIT_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaId"]["const"] == (
        "agent.semantic-protocols.runtime-state-cleanup-commit-receipt"
    )
    assert schema["properties"]["schemaVersion"]["const"] == "1"
    assert schema["additionalProperties"] is False
    assert set(schema["required"]) == {
        "schemaId",
        "schemaVersion",
        "state",
        "planDigest",
        "reasonKind",
        "deletedCount",
    }
