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
        "workspaceIdentity": "workspace-a",
        "generationDigest": "blake3-256:generation",
        "sourceRootDigest": "blake3-256:root",
        "query": "find graph router owner",
        "intent": "conceptual",
        "state": "completed",
        "plan": {
            "stages": [
                {"family": "acquire", "capabilityId": "search.indexed-lexical"},
                {"family": "reason", "capabilityId": "search.resident-graph"},
                {
                    "family": "verify",
                    "capabilityId": "search.ripgrep-verify-candidates",
                },
            ],
            "coverage": "candidates",
            "maxOwners": 32,
            "deadlineMs": 500,
        },
        "evidence": {
            "indexedLexical": {
                "backend": "tantivy-hot-overlay",
                "candidateOwnerIds": ["owner:router", "owner:runtime"],
                "indexedOwnerCount": 100,
                "admittedOwnerCount": 100,
                "complete": True,
            },
            "residentGraph": {
                "entryOwnerIds": ["owner:router", "owner:runtime"],
                "entryNodeIds": ["item:route"],
                "candidateOwnerIds": ["owner:router", "owner:test"],
                "closureState": "complete",
                "unresolvedFrontierCount": 0,
            },
            "ripgrep": {
                "mode": "verify-candidates",
                "verifiedOwnerIds": ["owner:router", "owner:test"],
                "matchedOwnerIds": ["owner:router"],
                "coveredOwnerCount": 2,
                "admittedOwnerCount": 100,
                "complete": True,
            },
            "correlation": {
                "lexicalGraphOverlapCount": 1,
                "lexicalGraphJaccardPermille": 333,
                "graphMarginalCandidateCount": 1,
                "verifiedUnionCandidateCount": 2,
            },
        },
        "decision": {
            "chosenPath": "owner:router",
            "explanation": "lexical identity, graph ownership, and source bytes agree",
            "residualUncertainty": [],
            "nextCommand": "asp rust query --selector rust://src/router.rs#item/function/route",
        },
        "metrics": {
            "totalElapsedMicros": 240,
            "laneElapsedMicros": {
                "indexedLexical": 80,
                "residentGraph": 120,
                "ripgrep": 40,
            },
            "commandCount": 1,
        },
    }


class SearchPlaybookReceiptSchemaTests(unittest.TestCase):
    def test_valid_tri_lane_receipt(self):
        Draft202012Validator(SCHEMA).validate(receipt())

    def test_seed_products_are_not_admitted(self):
        value = receipt()
        value["evidence"]["residentGraph"]["seeds"] = ["owner:router"]
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_reason_capability_is_extensible_without_changing_schema(self):
        value = receipt()
        value["plan"]["stages"][1] = {
            "family": "reason",
            "capabilityId": "reasoning.meta-relational",
        }
        Draft202012Validator(SCHEMA).validate(value)

    def test_rejects_manual_multi_command_route(self):
        value = receipt()
        value["route"] = ["asp rust search prime", "asp rg -query router"]
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_rejects_placeholder_next_command(self):
        value = receipt()
        value["decision"]["nextCommand"] = (
            "asp rust query --selector <selector> --workspace ."
        )
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))

    def test_absence_proof_requires_complete_coverage(self):
        value = receipt()
        value["intent"] = "absence-proof"
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))
        value["plan"]["coverage"] = "complete"
        Draft202012Validator(SCHEMA).validate(value)

    def test_failed_receipt_requires_typed_failure(self):
        value = receipt()
        value["state"] = "failed"
        value["failure"] = {
            "reasonKind": "generation-unavailable",
            "detail": "no admitted source generation",
        }
        Draft202012Validator(SCHEMA).validate(value)
        del value["failure"]
        self.assertTrue(list(Draft202012Validator(SCHEMA).iter_errors(value)))


if __name__ == "__main__":
    unittest.main()
