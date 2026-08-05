"""Validate the bounded owner-missing topology packet contract."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas" / "search-owner-missing-topology.v1.schema.json").read_text()
)


def packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.search-owner-missing-topology",
        "schemaVersion": "1",
        "state": "owner-missing",
        "reasonKind": "owner-not-in-workspace",
        "languageId": "rust",
        "ownerPath": "crates/demo/src/lib.rs",
        "generationDigest": "generation-42",
        "rootDigest": "root-42",
        "nodes": [
            {
                "id": "owner:crates/demo/src/lib.rs",
                "kind": "owner",
                "role": "missing-owner",
                "value": "crates/demo/src/lib.rs",
                "action": "owner",
                "confidence": "exact",
                "path": "crates/demo/src/lib.rs",
            }
        ],
        "edges": [
            {
                "source": "generation:generation-42",
                "target": "owner:crates/demo/src/lib.rs",
                "relation": "omits_owner",
            }
        ],
        "actionFrontier": ["A1.lexical-owner-evidence"],
        "recommendedNext": (
            "asp rust search lexical --query crates/demo/src/lib.rs "
            "--workspace . --view seeds"
        ),
    }


def test_search_owner_missing_topology_schema_accepts_bounded_packet() -> None:
    Draft202012Validator.check_schema(SCHEMA)
    Draft202012Validator(SCHEMA).validate(packet())


def test_search_owner_missing_topology_schema_rejects_unbounded_nodes() -> None:
    value = packet()
    value["nodes"] = value["nodes"] * 13
    errors = list(Draft202012Validator(SCHEMA).iter_errors(value))
    assert any(error.validator == "maxItems" for error in errors)
