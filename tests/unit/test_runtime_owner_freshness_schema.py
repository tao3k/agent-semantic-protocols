import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-owner-freshness-receipt.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def receipt() -> dict[str, object]:
    return {
        "schemaId": "asp.runtime-owner-freshness-receipt.v1",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-23cc5ba784c605ae",
        "generationDigest": f"blake3-256:{'a' * 64}",
        "ownerPath": "src/lib.rs",
        "ownerContentDigest": f"blake3-256:{'b' * 64}",
        "changed": False,
        "removed": False,
    }


def test_current_runtime_owner_receipt_is_valid() -> None:
    VALIDATOR.validate(receipt())


def test_removed_runtime_owner_requires_null_digest() -> None:
    candidate = receipt()
    candidate["removed"] = True
    with pytest.raises(ValidationError):
        VALIDATOR.validate(candidate)
    candidate["ownerContentDigest"] = None
    VALIDATOR.validate(candidate)


@pytest.mark.parametrize("owner_path", ["/src/lib.rs", "../src/lib.rs", "src/../lib.rs"])
def test_runtime_owner_receipt_rejects_escaping_path(owner_path: str) -> None:
    candidate = receipt()
    candidate["ownerPath"] = owner_path
    with pytest.raises(ValidationError):
        VALIDATOR.validate(candidate)
