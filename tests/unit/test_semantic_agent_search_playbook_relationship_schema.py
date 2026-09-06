import unittest

from tests.unit.test_semantic_agent_search_playbook_receipt_schema import (
    search_playbook_receipt,
    validator,
)


class SearchPlaybookRelationshipSchemaTests(unittest.TestCase):
    def test_relationship_receipt_admits_exact_python_graph_evidence(self):
        value = search_playbook_receipt()
        value["intent"] = "relationship"
        value["evidence"]["pythonGraph"] = {
            "state": "executed",
            "reasonKind": "exact-generation-resident-evaluation",
            "backend": "asp-python-graphs",
            "generationDigest": "blake3-256:" + "9" * 64,
            "projectionDigest": "blake3-256:" + "8" * 64,
            "candidateOwnerIds": ["owner:runtime"],
        }
        value["metrics"]["stageElapsedMicros"]["pythonGraph"] = 17
        validator().validate(value)

    def test_stage_reordering_or_replacement_is_rejected(self):
        value = search_playbook_receipt()
        value["plan"]["stages"][3] = {
            "family": "reason",
            "capabilityId": "reasoning.meta-relational",
        }
        self.assertTrue(list(validator().iter_errors(value)))

    def test_manual_multi_command_route_is_rejected(self):
        value = search_playbook_receipt()
        value["route"] = ["asp rust search prime", "asp rg -query router"]
        self.assertTrue(list(validator().iter_errors(value)))


if __name__ == "__main__":
    unittest.main()
