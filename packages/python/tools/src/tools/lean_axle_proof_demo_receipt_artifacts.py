"""Build proof receipt and report artifacts from AXLE responses."""

from __future__ import annotations

from typing import Any

from .lean_axle_proof_demo_plan_artifacts import sha256_text


def build_validated_claims(verified: bool) -> list[dict[str, Any]]:
    return [
        {
            "id": "producer-bug-packet-invalid",
            "meaning": "A packet with no selector and pathLine identity violates the SearchPacket contract.",
            "theorem": "producer_bug_packet_invalid",
            "verified": verified,
        },
        {
            "id": "defensive-renderer-accepts-invalid-packet",
            "meaning": "A renderer that accepts pathLine fallback accepts an invalid packet.",
            "theorem": "defensive_renderer_accepts_invalid_packet",
            "verified": verified,
        },
        {
            "id": "defensive-renderer-not-compliant",
            "meaning": "The defensive renderer does not satisfy rendererCompliant.",
            "theorem": "defensive_renderer_not_compliant",
            "verified": verified,
        },
        {
            "id": "producer-fixed-packet-valid",
            "meaning": "A producer-side selector fix restores the SearchPacket contract.",
            "theorem": "producer_fixed_packet_valid",
            "verified": verified,
        },
        {
            "id": "selector-only-renderer-compliant",
            "meaning": "A renderer that accepts only valid selector identity is compliant.",
            "theorem": "selector_only_renderer_compliant",
            "verified": verified,
        },
    ]


def build_receipt(
    obligation: dict[str, Any],
    recipe: dict[str, Any],
    environment: str,
    formal_statement: str,
    candidate_proof: str,
    response_json: dict[str, Any],
    schema_projection: dict[str, Any],
    packet_projection: dict[str, Any],
) -> dict[str, Any]:
    okay = bool(response_json["okay"])
    packet_contract_valid = bool(packet_projection["contractValid"])
    packet_identity_kind = packet_projection["identityKind"]
    branch_legal = [
        "fix-provider-selector-construction",
        "schema-migration-if-contract-changed",
    ]
    return {
        "schemaId": "agent.semantic-protocols.semantic-proof-receipt",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.formal-verification",
        "protocolVersion": "1",
        "receiptId": "proof-receipt:search-packet-selector-identity:v1",
        "obligationId": obligation["obligationId"],
        "recipeId": recipe["recipeId"],
        "checker": "axle.verify_proof",
        "environment": environment,
        "okay": okay,
        "trustLevel": "verify-proof",
        "failedDeclarations": response_json["failed_declarations"],
        "formalStatementDigest": sha256_text(formal_statement),
        "candidateDigest": sha256_text(candidate_proof),
        "timings": response_json["timings"],
        "schemaProjection": schema_projection,
        "packetProjection": packet_projection,
        "validatedClaims": build_validated_claims(okay),
        "defensiveEngineeringAssessment": {
            "result": "blocked" if okay else "unproven",
            "blockedBranch": "renderer-path-line-fallback",
            "whyBlocked": _why_renderer_fallback_is_blocked(
                okay,
                packet_contract_valid,
                packet_identity_kind,
            ),
            "correctBoundary": "provider-result-construction-boundary",
            "replacementBranches": branch_legal if okay else [],
        },
        "branchLegalityUpdate": {
            "illegal": ["renderer-path-line-fallback"] if okay else [],
            "legal": branch_legal if okay else [],
        },
        "summaryForAgent": _summary_for_agent(
            okay,
            packet_contract_valid,
            packet_identity_kind,
        ),
    }


def build_report(receipt: dict[str, Any]) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-formal-verification-report",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.formal-verification",
        "protocolVersion": "1",
        "reportId": "formal-verification-report:search-packet-selector-identity:v1",
        "receiptId": receipt["receiptId"],
        "schemaProjection": receipt["schemaProjection"],
        "packetProjection": receipt["packetProjection"],
        "validatedClaims": receipt["validatedClaims"],
        "defensiveEngineeringAssessment": receipt["defensiveEngineeringAssessment"],
        "rawAxleResponse": "axle-response.json",
    }


def _why_renderer_fallback_is_blocked(
    okay: bool,
    packet_contract_valid: bool,
    packet_identity_kind: str,
) -> str:
    if not okay:
        return "AXLE did not verify the proof, so this demo cannot block the branch."
    if packet_contract_valid:
        return (
            "The packet projection is contract-valid with "
            f"{packet_identity_kind} identity; renderer path+line fallback remains blocked "
            "by the global renderer compliance proof."
        )
    return (
        "The packet projection is contract-invalid with "
        f"{packet_identity_kind} identity, and the Lean model proves defensiveRenderer "
        "accepts invalid producer output."
    )


def _summary_for_agent(
    okay: bool,
    packet_contract_valid: bool,
    packet_identity_kind: str,
) -> str:
    if not okay:
        return "Proof failed. Do not prune branches from this receipt."
    if packet_contract_valid:
        return (
            "AXLE verified selector-owned packet identity. Do not add renderer path+line "
            "fallback; keep executable identity at the provider/schema boundary."
        )
    return (
        "AXLE verified a contract-invalid "
        f"{packet_identity_kind} packet. Do not add renderer fallback to path+line; "
        "fix the provider/schema boundary."
    )
