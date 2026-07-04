"""Build proof obligation and recipe planning artifacts."""

from __future__ import annotations

import hashlib
from typing import Any


def sha256_text(value: str) -> str:
    return "sha256:" + hashlib.sha256(value.encode("utf-8")).hexdigest()


def build_obligation(now: str) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-proof-obligation",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.formal-verification",
        "protocolVersion": "1",
        "obligationId": "proof-obligation:search-packet-selector-identity:v1",
        "project": "agent-semantic-protocols",
        "kind": "architecture-invariant",
        "status": "open",
        "createdAt": now,
        "claim": {
            "summary": "SearchPacket identity must be selector-owned, not line-number-owned.",
            "why": (
                "Line numbers are unstable under active agent edits and cannot be durable "
                "search identity."
            ),
            "mustHoldAt": "provider-result-construction-boundary",
            "mustNotBeMovedTo": [
                "renderer-consumer-fallback",
                "agent-facing-output-normalization",
            ],
        },
        "topology": {
            "sliceId": "topology-slice:search-packet-rendering",
            "ownerSelectors": [
                "schema:semantic-search-packet.v1",
                "rust:provider-result-builder",
                "rust:search-renderer",
            ],
            "hotEdges": [
                "provider-result-builder -> semantic-search-packet.v1",
                "semantic-search-packet.v1 -> search-renderer",
            ],
        },
        "branchEffects": {
            "illegalBranches": [
                {
                    "branchId": "renderer-path-line-fallback",
                    "reason": "Consumer fallback hides a producer-owned identity invariant.",
                }
            ],
            "legalBranches": [
                {
                    "branchId": "fix-provider-selector-construction",
                    "reason": "The invariant belongs at the packet producer boundary.",
                },
                {
                    "branchId": "schema-migration-if-contract-changed",
                    "reason": "If identity semantics changed, update schema and receipts explicitly.",
                },
            ],
        },
    }


def build_recipe(
    environment: str,
    timeout_seconds: float,
    statement_path: str,
    proof_path: str,
    formal_statement: str,
    candidate_proof: str,
) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-proof-recipe",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.formal-verification",
        "protocolVersion": "1",
        "recipeId": "proof-recipe:search-packet-selector-identity:axle:v1",
        "obligationId": "proof-obligation:search-packet-selector-identity:v1",
        "owner": "asp-policy-scenarios",
        "goal": "Verify that valid SearchPacket identity never permits renderer path+line fallback.",
        "executor": {
            "kind": "lean-axle",
            "tool": "verify_proof",
            "environment": environment,
            "ignoreImports": True,
            "timeoutSeconds": timeout_seconds,
        },
        "inputs": {
            "formalStatement": {
                "path": statement_path,
                "digest": sha256_text(formal_statement),
            },
            "candidateProof": {
                "path": proof_path,
                "digest": sha256_text(candidate_proof),
            },
        },
        "output": {
            "receiptPath": "proof-receipt.json",
            "attachToEvidenceGraph": True,
            "updateTopologyBranchLegality": True,
        },
        "agentDisclosure": {
            "level0": "Selector identity is verified. Do not add path+line fallback in renderer.",
            "level1": "Show receipt id, trust level, failed declarations, and valid schema version.",
            "level2": "Reveal full Lean statement and candidate proof only when debugging the proof lane.",
        },
    }
