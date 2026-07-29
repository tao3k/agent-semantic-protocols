import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
RESPONSE_SCHEMA = ROOT / "schemas" / "provider-native-exact-response.v1.schema.json"


def _resolution(state: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.provider-native-exact-projection",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "agent.semantic-protocols.providers.rust.rs-harness",
        "ownerPath": "src/lib.rs",
        "requestedStructuralSelector": "rust://src/lib.rs#item/function/missing",
        "resolutionState": state,
        "reasonKind": f"typed-{state}",
        "itemKind": "function",
        "itemName": "missing",
        "candidates": [],
        "actualKinds": [],
        "recommendedNext": {
            "command": "asp rust search pipe 'missing' --workspace . --view seeds"
        },
    }


def test_exact_resolution_states_are_schema_valid_semantic_results() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)

    for state in ("item-missing", "owner-missing", "kind-mismatch", "ambiguous"):
        validator.validate(_resolution(state))


def test_exact_resolution_requires_reason_and_recovery_action() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)
    packet = _resolution("owner-missing")
    packet.pop("recommendedNext")

    assert list(validator.iter_errors(packet))
