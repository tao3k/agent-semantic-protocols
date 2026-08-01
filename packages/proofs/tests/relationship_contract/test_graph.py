"""Test generic Relationship Contract graph referential integrity."""

from __future__ import annotations

from asp_proofs._relationship_contract_graph import validate_relationship_graph
from asp_proofs.relationship_contract import RelationshipContractVerificationError

from ._support import RelationshipContractFixture


class RelationshipContractGraphTest(RelationshipContractFixture):
    """Reject missing endpoints, gates, and directional invalidation cycles."""

    def setUp(self) -> None:
        super().setUp()
        self.artifacts = [{"artifactId": "artifact.a"}, {"artifactId": "artifact.b"}]
        self.relationship = {
            "relationshipId": "edge.v1",
            "subject": "artifact.a",
            "predicate": "depends-on",
            "object": "artifact.b",
            "impact": "invalidates-subject",
            "evidenceGates": ["gate.v1"],
        }

    def test_rejects_missing_endpoint(self) -> None:
        self.relationship["object"] = "artifact.missing"
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "missing artifact endpoints"
        ):
            validate_relationship_graph(
                self.artifacts, [self.relationship], ["gate.v1"]
            )

    def test_rejects_undeclared_evidence_gate(self) -> None:
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "undeclared evidence gates"
        ):
            validate_relationship_graph(self.artifacts, [self.relationship], ["other"])

    def test_rejects_directional_invalidation_cycle(self) -> None:
        reverse = {
            **self.relationship,
            "relationshipId": "edge.reverse.v1",
            "impact": "invalidates-object",
        }
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "relationship invalidation cycle"
        ):
            validate_relationship_graph(
                self.artifacts, [self.relationship, reverse], ["gate.v1"]
            )
