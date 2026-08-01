import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
RESPONSE_SCHEMA = ROOT / "schemas" / "provider-native-exact-response.v1.schema.json"


def _resolution(state: str) -> dict[str, object]:
    packet: dict[str, object] = {
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
            "command": "asp rust search lexical --query 'missing' --query 'function missing' --workspace . --view seeds"
        },
    }
    if state == "selector-stale":
        packet["activeGenerationDigest"] = f"blake3-256:{'a' * 64}"
        packet["rootDigest"] = "b" * 64
    return packet


def test_exact_resolution_states_are_schema_valid_semantic_results() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)

    for state in ("item-missing", "selector-stale", "kind-mismatch", "ambiguous"):
        validator.validate(_resolution(state))


def test_exact_resolution_requires_reason_and_recovery_action() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)
    packet = _resolution("selector-stale")
    packet.pop("recommendedNext")

    assert list(validator.iter_errors(packet))


def test_selector_stale_requires_active_generation_evidence() -> None:
    schema = json.loads(RESPONSE_SCHEMA.read_text())
    validator = Draft202012Validator(schema)
    packet = _resolution("selector-stale")
    packet.pop("activeGenerationDigest")

    assert list(validator.iter_errors(packet))
