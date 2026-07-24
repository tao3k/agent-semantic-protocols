import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/semantic-agent-search-playbook-receipt.v1.schema.json").read_text()
)


def receipt():
    return {
        "schemaId": "asp.search.playbook-receipt",
        "schemaVersion": "1",
        "workspace": ".",
        "intent": "find graph router owner",
        "state": "completed",
        "route": [{"command": "asp rust search owner src/router.rs items", "kind": "owner", "status": "hit"}],
        "evidence": ["owner:src/router.rs"],
        "topology": {"nodes": ["query:q", "owner:router"], "edges": ["q->router"], "frontier": []},
        "next": "asp rust query --selector rust://src/router.rs#item/function/route --workspace . --code",
        "metrics": {"commands": 1, "rounds": 1, "latencyMs": 10, "packetBytes": 120, "repeatedTriggers": 0, "missingEdges": []},
        "reflection": "owner materialized without restarting discovery",
    }


class SearchPlaybookReceiptSchemaTests(unittest.TestCase):
    def test_valid_receipt(self):
        Draft202012Validator(SCHEMA).validate(receipt())

    def test_rejects_duplicate_route(self):
        value = receipt()
        value["route"] = value["route"] * 2
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_rejects_placeholder_next(self):
        value = receipt()
        value["next"] = "asp rust query --selector <selector> --code"
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_rejects_faceless_prime_next(self):
        value = receipt()
        value["next"] = "asp search prime --workspace . --view seeds"
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_failed_receipt_requires_typed_failure_and_null_next(self):
        value = receipt()
        value["state"] = "failed"
        value["next"] = None
        value["failure"] = {
            "reasonKind": "provider-unavailable",
            "languageId": "c",
            "detail": "no registered ASP provider for this workspace language",
        }
        Draft202012Validator(SCHEMA).validate(value)

        del value["failure"]
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_route_kind_must_match_facade(self):
        value = receipt()
        value["route"][0] = {
            "command": "asp fd -query router .",
            "kind": "prime",
            "status": "hit",
        }
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

        value["route"][0]["kind"] = "fd"
        Draft202012Validator(SCHEMA).validate(value)

    def test_accepts_provider_ambiguity_closure(self):
        value = receipt()
        value["state"] = "failed"
        value["next"] = None
        value["failure"] = {
            "reasonKind": "candidate-provider-ambiguous",
            "detail": "two language harnesses claimed the same candidate",
        }
        Draft202012Validator(SCHEMA).validate(value)


if __name__ == "__main__":
    unittest.main()
