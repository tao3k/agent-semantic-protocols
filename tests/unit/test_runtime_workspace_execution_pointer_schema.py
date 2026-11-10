from copy import deepcopy
from pathlib import Path

import pytest
from jsonschema.exceptions import ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas/runtime-workspace-execution-pointer.v1.schema.json"


def digest(character: str) -> str:
    return f"blake3-256:{character * 64}"


def pointer() -> dict:
    return {
        "schemaId": "agent.semantic-protocols.runtime-workspace-execution-pointer",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-main",
        "generationDigest": digest("1"),
        "sourceRootDigest": digest("2"),
        "executionPublicationDigest": digest("3"),
    }


def test_complete_execution_pointer_is_schema_valid() -> None:
    schema_validator_for(SCHEMA).validate(pointer())


@pytest.mark.parametrize(
    "field",
    ["generationDigest", "sourceRootDigest", "executionPublicationDigest"],
)
def test_execution_pointer_rejects_a_missing_product_digest(field: str) -> None:
    candidate = deepcopy(pointer())
    del candidate[field]
    with pytest.raises(ValidationError):
        schema_validator_for(SCHEMA).validate(candidate)


def test_execution_pointer_rejects_activation_generation_identity() -> None:
    candidate = pointer()
    candidate["activationGeneration"] = 84
    with pytest.raises(ValidationError):
        schema_validator_for(SCHEMA).validate(candidate)
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
