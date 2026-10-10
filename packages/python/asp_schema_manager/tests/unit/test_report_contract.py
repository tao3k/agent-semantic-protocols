# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Management report contract validation tests."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from asp_schema_manager.audit import audit_workspace


def test_repository_report_matches_shared_contract() -> None:
    workspace_root = Path(__file__).resolve().parents[5]
    report = audit_workspace(workspace_root)
    schema = json.loads(
        (workspace_root / "schemas/asp-schema-management-report.v1.schema.json").read_text(
            encoding="utf-8"
        )
    )

    Draft202012Validator(schema).validate(report)
