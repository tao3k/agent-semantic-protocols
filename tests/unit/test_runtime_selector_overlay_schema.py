# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-selector-overlay-receipt.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-server-selector-overlay-receipt.v1",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-23cc5ba784c605ae",
        "generationDigest": f"blake3-256:{'a' * 64}",
        "projectionKind": "source",
        "structuralSelector": "rust://src/lib.rs#item/function/run",
        "inserted": True,
    }


def test_runtime_selector_overlay_receipt_is_valid() -> None:
    VALIDATOR.validate(receipt())


@pytest.mark.parametrize("field", ["workspaceIdentity", "structuralSelector"])
def test_runtime_selector_overlay_receipt_rejects_empty_identity(field: str) -> None:
    candidate = receipt()
    candidate[field] = ""
    with pytest.raises(ValidationError):
        VALIDATOR.validate(candidate)


def test_runtime_selector_overlay_receipt_rejects_non_blake3_generation() -> None:
    candidate = receipt()
    candidate["generationDigest"] = "not-a-generation-digest"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(candidate)


def test_runtime_selector_overlay_receipt_rejects_unknown_projection_kind() -> None:
    candidate = receipt()
    candidate["projectionKind"] = "legacy-code"
    with pytest.raises(ValidationError):
        VALIDATOR.validate(candidate)
