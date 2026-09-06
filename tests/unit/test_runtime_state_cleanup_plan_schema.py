import json
from pathlib import Path

from jsonschema import Draft202012Validator


SCHEMA_PATH = Path("schemas/runtime-state-cleanup-plan.v1.schema.json")
COMMIT_SCHEMA_PATH = Path(
    "schemas/runtime-state-cleanup-commit-receipt.v1.schema.json"
)


def load_schema() -> dict:
    return json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))


def test_runtime_state_cleanup_plan_is_a_closed_v1_contract() -> None:
    schema = load_schema()
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaId"]["const"] == (
        "agent.semantic-protocols.runtime-state-cleanup-plan"
    )
    assert schema["properties"]["schemaVersion"]["const"] == "1"
    assert schema["additionalProperties"] is False


def test_cleanup_entries_require_reachability_evidence() -> None:
    entry = load_schema()["properties"]["entries"]["items"]

    assert entry["additionalProperties"] is False
    assert entry["required"] == ["relativePath", "reasonKind", "evidenceClass"]
    assert "legacy-runtime-authority" in entry["properties"]["reasonKind"]["enum"]
    assert "active-healthy-unreachable" in entry["properties"]["evidenceClass"]["enum"]


def test_cleanup_commit_is_bound_to_both_bundle_selectors() -> None:
    required = set(load_schema()["required"])

    assert "expectedActiveBundleDigest" in required
    assert "expectedHealthyBundleDigest" in required
    assert "planDigest" in required


def test_cleanup_has_no_activation_generation_authority() -> None:
    serialized = json.dumps(load_schema(), sort_keys=True)

    assert "activationGeneration" not in serialized
    assert "candidate" not in serialized.lower()


def test_cleanup_commit_receipt_has_an_independent_closed_identity() -> None:
    schema = json.loads(COMMIT_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)

    assert schema["properties"]["schemaId"]["const"] == (
        "agent.semantic-protocols.runtime-state-cleanup-commit-receipt"
    )
    assert schema["properties"]["schemaVersion"]["const"] == "1"
    assert schema["additionalProperties"] is False
    assert set(schema["required"]) == {
        "schemaId",
        "schemaVersion",
        "state",
        "planDigest",
        "reasonKind",
        "deletedCount",
    }
