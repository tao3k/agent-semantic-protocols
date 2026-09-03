import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/lexical-generation-plan.v1.schema.json").read_text()
)


def digest(value: str) -> str:
    return f"blake3-256:{value * 64}"


def test_lexical_generation_plan_schema_accepts_reuse_and_rebuild() -> None:
    receipt = {
        "schemaId": "agent.semantic-protocols.lexical-generation-plan",
        "schemaVersion": "1",
        "analyzerDigest": digest("a"),
        "inventoryDigest": digest("b"),
        "admittedOwnerDigest": digest("c"),
        "lexicalFactDigest": digest("d"),
        "entries": [
            {
                "ownerPath": "src/lib.rs",
                "contentDigest": digest("e"),
                "shardKey": digest("f"),
                "disposition": "reuse",
                "priorArtifactDigest": digest("1"),
                "queryKeys": ["owner", "search"],
            },
            {
                "ownerPath": "src/new.rs",
                "contentDigest": digest("2"),
                "shardKey": digest("3"),
                "disposition": "rebuild",
                "priorArtifactDigest": None,
                "queryKeys": ["new"],
            },
        ],
        "retiredArtifactDigests": [digest("4")],
        "planDigest": digest("5"),
    }
    jsonschema.Draft202012Validator(SCHEMA).validate(receipt)

    receipt["entries"][0]["disposition"] = "rescan"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(receipt)
