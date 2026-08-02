"""Packet-side fixture objects for Relationship Contract tests."""

from __future__ import annotations

from typing import Any

from ._digests import source_digest


def packet_artifacts(
    authority_source: str,
    relationship_contract_source: str,
) -> list[dict[str, Any]]:
    """Build the typed fixture artifact table."""
    return [
        {
            "artifactId": "doc.fixture-authority",
            "artifactKind": "org-document",
            "artifactPath": "org/relationships/fixture.v1.org",
            "canonicalization": "org-source-raw-lf-utf8-v1",
            "expectedSha256": source_digest(authority_source),
        },
        {
            "artifactId": "contract.fixture-relationship",
            "artifactKind": "org-contract",
            "artifactPath": "org/contracts/relationship.v1.org",
            "canonicalization": "org-source-raw-lf-utf8-v1",
            "expectedSha256": source_digest(relationship_contract_source),
        },
    ]


def packet_relationships() -> list[dict[str, Any]]:
    """Build the typed fixture relationship table."""
    return [
        {
            "relationshipId": "fixture-conforms-v1",
            "subject": "doc.fixture-authority",
            "predicate": "conforms-to",
            "object": "contract.fixture-relationship",
            "impact": "invalidates-subject",
            "evidenceGates": ["fixture-relationship-gate"],
        }
    ]


def build_authority_evaluations(artifact_count: int) -> list[dict[str, Any]]:
    """Build passing fake contract evaluations for authority verification."""
    assertion = {"assertionId": "fixture.relationship", "status": "passed"}
    return [
        {
            "contractId": "relationship.document.v1",
            "assertions": [assertion],
        },
        *[
            {
                "contractId": "relationship.artifact.v1",
                "assertions": [assertion],
            }
            for _ in range(artifact_count)
        ],
        {
            "contractId": "relationship.edge.v1",
            "assertions": [assertion],
        },
    ]
