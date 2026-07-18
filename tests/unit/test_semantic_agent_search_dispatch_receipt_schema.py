"""Validate the exactly-once resident search dispatch receipt."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


_REPO_ROOT = Path(__file__).resolve().parents[2]


def receipt(state: str) -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.search-dispatch-receipt",
        "schemaVersion": "1",
        "dispatchIdentity": "dispatch-1",
        "rootSessionId": "root-1",
        "childSessionId": "child-1",
        "messageTargetId": "/root/asp_explorer",
        "commandDigest": f"sha256:{'a' * 64}",
        "state": state,
        "routingTerminal": True,
        "redispatchAllowed": False,
    }


class SearchDispatchReceiptSchemaTests(unittest.TestCase):
    def setUp(self) -> None:
        path = (
            _REPO_ROOT
            / "schemas"
            / "semantic-agent-search-dispatch-receipt.v1.schema.json"
        )
        self.validator = Draft202012Validator(json.loads(path.read_text(encoding="utf-8")))

    def errors(self, value: dict[str, object]) -> list[str]:
        return [error.message for error in self.validator.iter_errors(value)]

    def test_accepted_receipt_is_valid_without_owner(self) -> None:
        self.assertEqual([], self.errors(receipt("accepted")))

    def test_completed_receipt_requires_materialized_owner_and_command(self) -> None:
        value = receipt("completed")
        self.assertNotEqual([], self.errors(value))
        value["owner"] = {
            "languageId": "rust",
            "path": "src/owner.rs",
            "selector": "rust:item:src/owner.rs:owner",
        }
        value["nextCommand"] = [
            "asp",
            "rust",
            "query",
            "--selector",
            "rust:item:src/owner.rs:owner",
            "--workspace",
            ".",
            "--code",
        ]
        self.assertEqual([], self.errors(value))

    def test_receipt_cannot_authorize_redispatch(self) -> None:
        value = receipt("running")
        value["redispatchAllowed"] = True
        self.assertNotEqual([], self.errors(value))

    def test_completed_receipt_rejects_placeholder_next_command(self) -> None:
        value = receipt("completed")
        value["owner"] = {
            "languageId": "rust",
            "path": "src/owner.rs",
            "selector": "rust:item:src/owner.rs:owner",
        }
        value["nextCommand"] = ["asp", "rust", "search", "owner", "<owner-path>"]
        self.assertNotEqual([], self.errors(value))


if __name__ == "__main__":
    unittest.main()
