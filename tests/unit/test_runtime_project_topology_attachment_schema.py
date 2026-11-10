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
    "fixtures/runtime-project-topology-attachment/valid-current-generation.v1.json"
)


@pytest.fixture()
def attachment() -> dict:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def validator():
    return schema_validator_for(
        SCHEMAS / "runtime-project-topology-attachment.v1.schema.json"
    )


def test_current_runtime_project_topology_attachment_is_structurally_valid(
    attachment,
) -> None:
    validator().validate(attachment)


@pytest.mark.parametrize(
    "forbidden_field",
    ["activationGeneration", "nextAction", "recommendedNext", "planner"],
)
def test_attachment_rejects_fence_or_planner_fields(
    attachment, forbidden_field
) -> None:
    attachment[forbidden_field] = "forbidden"
    with pytest.raises(ValidationError):
        validator().validate(attachment)


def test_attachment_requires_exactly_one_success_terminal(attachment) -> None:
    changed = deepcopy(attachment)
    changed["terminal"]["terminalCount"] = 2
    with pytest.raises(ValidationError):
        validator().validate(changed)
    changed = deepcopy(attachment)
    changed["terminal"]["reasonKind"] = "topology-binding-mismatch"
    with pytest.raises(ValidationError):
        validator().validate(changed)


def test_attachment_requires_a_complete_identity_product(attachment) -> None:
    del attachment["topologyLibraryBinding"]["parserCatalogDigest"]
    with pytest.raises(ValidationError):
        validator().validate(attachment)
