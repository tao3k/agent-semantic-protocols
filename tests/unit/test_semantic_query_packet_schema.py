import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

_REPO_ROOT = Path(__file__).resolve().parents[2]


def minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-query-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "method": "query/owner-items",
        "projectRoot": "/workspace/project",
        "ownerPath": "src/lib.rs",
        "query": "load|clone_value",
        "queryTerms": ["load", "clone_value"],
        "matchMode": "exact",
        "matches": [
            {
                "name": "load",
                "kind": "fn",
                "visibility": "public",
                "doc": False,
                "location": {"path": "src/lib.rs", "line": 6, "endLine": 6},
                "read": "src/lib.rs:6-6",
                "code": "pub fn load() -> Thing { domain::make_thing() }",
                "truncated": False,
            }
        ],
        "truncated": False,
        "notes": [],
    }


class SemanticQueryPacketSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "semantic-query-packet.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_minimal_query_packet_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(minimal_packet()))

    def test_read_locator_rejects_rank_prefix_path(self) -> None:
        packet = minimal_packet()
        packet["matches"][0]["read"] = "0:src/lib.rs:6-6"  # type: ignore[index]

        errors = self.validation_errors(packet)

        self.assertTrue(any("does not match" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
