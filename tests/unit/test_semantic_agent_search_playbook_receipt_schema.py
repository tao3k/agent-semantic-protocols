import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from tests.unit.search_playbook_receipt_fixture import digest, search_playbook_receipt


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/semantic-agent-search-playbook-receipt.v1.schema.json").read_text()
)
RUNTIME_SCHEMA = json.loads(
    (ROOT / "schemas/runtime-provider-search-receipt.v1.schema.json").read_text()
)
SCHEMA_REGISTRY = Registry().with_resource(
    RUNTIME_SCHEMA["$id"], Resource.from_contents(RUNTIME_SCHEMA)
)


def validator():
    return Draft202012Validator(SCHEMA, registry=SCHEMA_REGISTRY)


class SearchPlaybookReceiptSchemaTests(unittest.TestCase):
    def test_valid_ordered_native_syntax_playbook_receipt(self):
        validator().validate(search_playbook_receipt())

    def test_valid_cold_rg_receipt_is_exact_generation_bound(self):
        value = search_playbook_receipt()
        value["evidence"]["indexedLexical"].update(
            state="skipped", candidateOwnerIds=[], complete=False
        )
        value["evidence"]["residentGraph"].update(
            state="skipped",
            entryOwnerIds=[],
            entryNodeIds=[],
            candidateOwnerIds=[],
            closureState="unavailable",
        )
        value["evidence"]["ripgrep"] = {
            "state": "executed",
            "reasonKind": "content-generation-cold-recall",
            "mode": "immutable-generation-corpus",
            "generationDigest": value["generationDigest"],
            "coverageInputDigest": digest("a"),
            "candidateOwnerIds": value["decision"]["ownerPaths"],
            "processCount": 1,
            "complete": True,
        }
        validator().validate(value)

    def test_owner_only_native_syntax_evidence_is_rejected(self):
        value = search_playbook_receipt()
        del value["evidence"]["nativeSyntax"]["projections"]
        self.assertTrue(list(validator().iter_errors(value)))

    def test_seed_products_are_not_admitted(self):
        value = search_playbook_receipt()
        value["evidence"]["residentGraph"]["seeds"] = ["owner:router"]
        self.assertTrue(list(validator().iter_errors(value)))

    def test_rejects_any_prescribed_next_command(self):
        value = search_playbook_receipt()
        value["decision"]["nextCommand"] = "asp search playbook 'router ownership'"
        self.assertTrue(list(validator().iter_errors(value)))

    def test_absence_proof_requires_complete_coverage(self):
        value = search_playbook_receipt()
        value["intent"] = "absence-proof"
        self.assertTrue(list(validator().iter_errors(value)))
        value["plan"]["coverage"] = "complete"
        validator().validate(value)

    def test_failure_is_not_encoded_as_a_partial_playbook_receipt(self):
        value = search_playbook_receipt()
        value.update(state="failed", failure={"reasonKind": "generation-unavailable"})
        self.assertTrue(list(validator().iter_errors(value)))


if __name__ == "__main__":
    unittest.main()
