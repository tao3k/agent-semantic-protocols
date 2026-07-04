"""Validate minimal formal proof artifact schema fixtures."""

from __future__ import annotations

import unittest

from .support import (
    assessment,
    claims,
    load_validator,
    packet_projection,
    schema_projection,
    validation_errors,
)


class SemanticProofArtifactFixtureTests(unittest.TestCase):
    def test_proof_obligation_is_valid(self) -> None:
        validator = load_validator("semantic-proof-obligation.v1.schema.json")
        payload = {
            "schemaId": "agent.semantic-protocols.semantic-proof-obligation",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.formal-verification",
            "protocolVersion": "1",
            "obligationId": "proof-obligation:search-packet-selector-identity:v1",
            "project": "agent-semantic-protocols",
            "kind": "architecture-invariant",
            "status": "open",
            "claim": {
                "summary": "SearchPacket identity must be selector-owned.",
                "why": "Line numbers are unstable under active agent edits.",
                "mustHoldAt": "provider-result-construction-boundary",
                "mustNotBeMovedTo": ["renderer-consumer-fallback"],
            },
            "topology": {
                "sliceId": "topology-slice:search-packet-rendering",
                "ownerSelectors": ["schema:semantic-search-packet.v1"],
            },
            "branchEffects": {
                "illegalBranches": [
                    {
                        "branchId": "renderer-path-line-fallback",
                        "reason": "Consumer fallback hides producer-owned identity.",
                    }
                ],
                "legalBranches": [
                    {
                        "branchId": "fix-provider-selector-construction",
                        "reason": "The invariant belongs at the producer boundary.",
                    }
                ],
            },
        }

        self.assertEqual(validation_errors(validator, payload), [])

    def test_proof_recipe_is_valid(self) -> None:
        validator = load_validator("semantic-proof-recipe.v1.schema.json")
        payload = {
            "schemaId": "agent.semantic-protocols.semantic-proof-recipe",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.formal-verification",
            "protocolVersion": "1",
            "recipeId": "proof-recipe:search-packet-selector-identity:axle:v1",
            "obligationId": "proof-obligation:search-packet-selector-identity:v1",
            "owner": "asp-policy-scenarios",
            "goal": "Verify that valid SearchPacket identity blocks renderer path+line fallback.",
            "executor": {
                "kind": "lean-axle",
                "tool": "verify_proof",
                "environment": "lean-4.31.0",
            },
            "inputs": {
                "formalStatement": {"path": "formal-statement.lean", "digest": "sha256:" + "a" * 64},
                "candidateProof": {"path": "candidate-proof.lean", "digest": "sha256:" + "b" * 64},
            },
            "output": {
                "receiptPath": "proof-receipt.json",
                "attachToEvidenceGraph": True,
                "updateTopologyBranchLegality": True,
            },
            "agentDisclosure": {
                "level0": "Do not add renderer path+line fallback.",
                "level1": "Show receipt id and validated claims.",
                "level2": "Reveal Lean proof only when debugging proof lane.",
            },
        }

        self.assertEqual(validation_errors(validator, payload), [])

    def test_proof_receipt_and_report_are_valid(self) -> None:
        receipt = {
            "schemaId": "agent.semantic-protocols.semantic-proof-receipt",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.formal-verification",
            "protocolVersion": "1",
            "receiptId": "proof-receipt:search-packet-selector-identity:v1",
            "obligationId": "proof-obligation:search-packet-selector-identity:v1",
            "recipeId": "proof-recipe:search-packet-selector-identity:axle:v1",
            "checker": "axle.verify_proof",
            "environment": "lean-4.31.0",
            "okay": True,
            "trustLevel": "verify-proof",
            "failedDeclarations": [],
            "formalStatementDigest": "sha256:" + "a" * 64,
            "candidateDigest": "sha256:" + "b" * 64,
            "timings": {"total_ms": 678},
            "schemaProjection": schema_projection(),
            "packetProjection": packet_projection(),
            "validatedClaims": claims(),
            "defensiveEngineeringAssessment": assessment(),
            "branchLegalityUpdate": {
                "illegal": ["renderer-path-line-fallback"],
                "legal": ["fix-provider-selector-construction"],
            },
            "summaryForAgent": "AXLE verified that the defensive renderer accepts invalid output.",
        }
        report = {
            "schemaId": "agent.semantic-protocols.semantic-formal-verification-report",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.formal-verification",
            "protocolVersion": "1",
            "reportId": "formal-verification-report:search-packet-selector-identity:v1",
            "receiptId": receipt["receiptId"],
            "schemaProjection": schema_projection(),
            "packetProjection": packet_projection(),
            "validatedClaims": claims(),
            "defensiveEngineeringAssessment": assessment(),
            "rawAxleResponse": "axle-response.json",
        }

        self.assertEqual(
            validation_errors(load_validator("semantic-proof-receipt.v1.schema.json"), receipt),
            [],
        )
        self.assertEqual(
            validation_errors(load_validator("semantic-formal-verification-report.v1.schema.json"), report),
            [],
        )

    def test_rejects_legacy_schema_field(self) -> None:
        validator = load_validator("semantic-proof-receipt.v1.schema.json")
        errors = validation_errors(
            validator,
            {
                "schema": "semantic-proof-receipt.v1",
                "receiptId": "proof-receipt:search-packet-selector-identity:v1",
            },
        )

        self.assertTrue(any("Additional properties" in error for error in errors))
        self.assertTrue(any("'schemaId' is a required property" in error for error in errors))
