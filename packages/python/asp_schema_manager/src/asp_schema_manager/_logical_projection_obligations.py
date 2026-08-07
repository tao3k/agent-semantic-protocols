"""Compose semantic-proof obligation instances for schema projections."""

from __future__ import annotations

import hashlib
from typing import Any


def proof_obligations(
    *,
    schema_path: str,
    source_digest: str,
    closure_digest: str,
    facts: list[dict[str, str]],
) -> list[dict[str, Any]]:
    supported = sorted(
        {item["keyword"] for item in facts if item["support"] == "supported"}
    )
    unsupported = sorted(
        {item["keyword"] for item in facts if item["support"] == "unsupported"}
    )
    obligations = [
        _obligation(
            schema_path=schema_path,
            source_digest=source_digest,
            closure_digest=closure_digest,
            disposition="supported",
            keywords=supported,
            claim="Supported JSON Schema keyword semantics remain invariant in the Lean projection.",
        )
    ]
    if unsupported:
        obligations.append(
            _obligation(
                schema_path=schema_path,
                source_digest=source_digest,
                closure_digest=closure_digest,
                disposition="unsupported",
                keywords=unsupported,
                claim="Unsupported JSON Schema keywords remain explicit open proof obligations.",
            )
        )
    return obligations


def _obligation(
    *,
    schema_path: str,
    source_digest: str,
    closure_digest: str,
    disposition: str,
    keywords: list[str],
    claim: str,
) -> dict[str, Any]:
    identity = hashlib.sha256(
        f"{schema_path}\0{source_digest}\0{closure_digest}\0{disposition}".encode()
    ).hexdigest()[:20]
    obligation_id = f"schema.projection.{disposition}.{identity}"
    return {
        "schemaId": "agent.semantic-protocols.semantic-proof-obligation",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.formal-verification",
        "protocolVersion": "1",
        "obligationId": obligation_id,
        "project": "agent-semantic-protocols",
        "kind": "schema-invariant",
        "status": "open",
        "claim": {
            "summary": claim,
            "why": "Schema Manager projections are evidence only after exact digest-bound verification.",
            "mustHoldAt": schema_path,
            "mustNotBeMovedTo": ["asp-schema-manager-private-proof-contract"],
        },
        "topology": {
            "sliceId": f"schema.projection.{identity}",
            "ownerSelectors": [schema_path],
        },
        "branchEffects": {
            "illegalBranches": [
                {
                    "branchId": f"projection.{disposition}.unverified",
                    "reason": "No digest-bound semantic-proof receipt exists.",
                }
            ],
            "legalBranches": [
                {
                    "branchId": f"projection.{disposition}.verified",
                    "reason": "Axle or Lean checked the exact projection and obligation digests.",
                }
            ],
        },
        "fields": {
            "keywordDisposition": disposition,
            "keywords": keywords,
            "sourceContentDigest": source_digest,
            "resolvedReferenceClosureDigest": closure_digest,
        },
    }
