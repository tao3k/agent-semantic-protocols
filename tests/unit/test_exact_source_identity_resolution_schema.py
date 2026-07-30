import json
from pathlib import Path

import jsonschema


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas" / "exact-source-query-result.v1.schema.json").read_text()
)


def test_identity_incomplete_is_a_typed_exact_resolution_state() -> None:
    packet = {
        "schemaId": "agent.semantic-protocols.provider-native-exact-projection",
        "schemaVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "requestedStructuralSelector": "rust://src/cli.rs#item/method/parse",
        "resolutionState": "identity-incomplete",
        "reasonKind": "canonical-item-scope-required",
        "rootDigest": "a" * 64,
        "ownerPath": "src/cli.rs",
        "itemKind": "method",
        "itemName": "parse",
        "candidates": [
            "rust://src/cli.rs#item/method/parse/scope/implementation-owner/type/CliOptions"
        ],
        "actualKinds": [],
        "recommendedNext": {
            "command": "asp rust search owner src/cli.rs items --query 'parse' --workspace . --view seeds"
        },
    }

    jsonschema.validate(packet, SCHEMA)
