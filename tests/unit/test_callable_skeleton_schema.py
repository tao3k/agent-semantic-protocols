# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]


def test_callable_skeleton_payload_schema() -> None:
    schema = json.loads((ROOT / "schemas/callable-skeleton.schema.json").read_text())
    Draft202012Validator.check_schema(schema)
    fixture = json.loads(
        (ROOT / "schemas/fixtures/semantic-projection.callable-skeleton.v1.json").read_text()
    )
    Draft202012Validator(schema).validate(fixture["payload"])
