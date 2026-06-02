"""Validate provider-owned semantic read packet schema boundaries."""

from __future__ import annotations

import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def semantic_read_minimal_packet() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-read-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "binary": "rs-harness",
        "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
        "method": "query/direct-source-read",
        "projectRoot": "/workspace/project",
        "ownerPath": "src/lib.rs",
        "selector": "src/lib.rs",
        "fromHook": "direct-source-read",
        "outputMode": "read-packet",
        "sourceWindows": [
            {
                "ownerPath": "src/lib.rs",
                "itemName": "load",
                "itemKind": "fn",
                "location": {"path": "src/lib.rs", "line": 6, "endLine": 6},
                "read": "src/lib.rs:6-6",
                "startLine": 6,
                "endLine": 6,
                "lineCount": 1,
                "reason": "direct-selector",
                "text": "pub fn load() -> Thing { domain::make_thing() }",
                "truncated": False,
            }
        ],
        "truncated": False,
        "notes": [],
    }


class SemanticReadPacketSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        schema_path = _REPO_ROOT / "schemas" / "semantic-read-packet.v1.schema.json"
        with schema_path.open("r", encoding="utf-8") as handle:
            self.validator = Draft202012Validator(json.load(handle))

    def validation_errors(self, packet: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(packet)]

    def test_minimal_provider_read_packet_is_valid(self) -> None:
        self.assertEqual([], self.validation_errors(semantic_read_minimal_packet()))

    def test_read_packet_rejects_root_hook_protocol(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["protocolId"] = "agent.semantic-protocols.agent-hooks"
        errors = self.validation_errors(packet)
        self.assertTrue(any("was expected" in message for message in errors))

    def test_read_packet_requires_query_method(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["method"] = "agent/read"
        errors = self.validation_errors(packet)
        self.assertTrue(any("does not match" in message for message in errors))

    def test_selector_rejects_rank_prefixed_path(self) -> None:
        packet = semantic_read_minimal_packet()
        packet["selector"] = "0:src/lib.rs"
        errors = self.validation_errors(packet)
        self.assertTrue(any("does not match" in message for message in errors))

    def test_window_read_locator_rejects_rank_prefix_path(self) -> None:
        packet = semantic_read_minimal_packet()
        windows = packet["sourceWindows"]
        assert isinstance(windows, list)
        window = windows[0]
        assert isinstance(window, dict)
        window["read"] = "0:src/lib.rs:6-6"
        errors = self.validation_errors(packet)
        self.assertTrue(any("does not match" in message for message in errors))


if __name__ == "__main__":
    unittest.main()
