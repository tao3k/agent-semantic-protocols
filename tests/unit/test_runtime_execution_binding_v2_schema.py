# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

from jsonschema.exceptions import ValidationError
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
FIXTURE = SCHEMAS / (
    "fixtures/query-playbook-materialization-request/valid-runtime-bound.v1.json"
)


def runtime_binding() -> dict:
    request = json.loads(FIXTURE.read_text(encoding="utf-8"))
    return request["runtimeExecutionBinding"]


def test_runtime_execution_binding_v2_admits_the_complete_workspace_product() -> None:
    schema_validator_for(SCHEMAS / "runtime-execution-binding.v2.schema.json").validate(
        runtime_binding()
    )


@pytest.mark.parametrize("legacy_field", ["projectId", "workspaceId"])
def test_runtime_execution_binding_v2_rejects_legacy_identity_fields(
    legacy_field: str,
) -> None:
    binding = runtime_binding()
    binding[legacy_field] = "legacy-identity"
    with pytest.raises(ValidationError):
        schema_validator_for(
            SCHEMAS / "runtime-execution-binding.v2.schema.json"
        ).validate(binding)


def test_runtime_execution_binding_v2_rejects_missing_workspace_root() -> None:
    binding = deepcopy(runtime_binding())
    del binding["projectWorkspace"]["workspaceRootPath"]
    with pytest.raises(ValidationError):
        schema_validator_for(
            SCHEMAS / "runtime-execution-binding.v2.schema.json"
        ).validate(binding)
