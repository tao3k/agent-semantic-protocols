# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

from jsonschema import Draft202012Validator

from tests.unit.schema_validator_support import local_schema_validator


ROOT = Path(__file__).resolve().parents[2]


def load_json(path: Path) -> dict:
    return json.loads(path.read_text())


def test_shared_provider_schemas_forbid_static_path_scope() -> None:
    provider_schema = load_json(ROOT / "schemas" / "provider-manifest.schema.json")
    assert "source" not in provider_schema["properties"]


def test_static_path_scope_is_rejected_instead_of_ignored() -> None:
    manifest = {"source": {"defaultSourceRoots": ["src"]}}
    validator = local_schema_validator(
        ROOT / "schemas" / "provider-manifest.schema.json",
        ROOT / "schemas" / "provider-project-resolution-descriptor.schema.json",
        ROOT / "schemas" / "provider-runtime-contract-descriptor.schema.json",
        ROOT / "schemas" / "asp-client-server-descriptor.schema.json",
        ROOT / "schemas" / "provider-query-pack-descriptor.schema.json",
    )

    errors = list(validator.iter_errors(manifest))
    assert any(
        error.validator == "additionalProperties"
        and "source" in error.message
        for error in errors
    )


def test_provider_method_ids_reject_policy_cli_commands() -> None:
    provider_schema = load_json(ROOT / "schemas" / "provider-manifest.schema.json")
    validator = Draft202012Validator(provider_schema["$defs"]["methodId"])

    assert list(validator.iter_errors("query/owner")) == []
    assert list(validator.iter_errors("check/changed"))
    assert list(validator.iter_errors("evidence/graph"))
    assert list(validator.iter_errors("verification/run"))
