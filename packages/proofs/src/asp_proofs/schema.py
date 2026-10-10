# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""JSON Schema repository for proof package boundaries."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping

from jsonschema import Draft202012Validator


class ProofSchemaError(ValueError):
    """Raised when a proof-domain document violates its public schema."""


def load_json_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ProofSchemaError(f"expected JSON object: {path}")
    return value


def validate_mapping(schema_path: Path, value: Mapping[str, Any]) -> None:
    errors = sorted(
        Draft202012Validator(load_json_object(schema_path)).iter_errors(value),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        raise ProofSchemaError(errors[0].message)
