import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/host-agent-lifecycle-attestation.v1.schema.json").read_text()
)
FIXTURES = ROOT / "schemas/fixtures/host-agent-lifecycle-attestation"


def test_host_lifecycle_attestation_accepts_host_owned_identity() -> None:
    instance = json.loads((FIXTURES / "valid.v1.json").read_text())
    Draft202012Validator(SCHEMA).validate(instance)


def test_host_lifecycle_attestation_rejects_manager_shadow_fields() -> None:
    instance = json.loads(
        (FIXTURES / "invalid-manager-shadow-fields.v1.json").read_text()
    )
    errors = list(Draft202012Validator(SCHEMA).iter_errors(instance))
    assert errors
    assert "Additional properties are not allowed" in errors[0].message
