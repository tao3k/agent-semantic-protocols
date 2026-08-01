"""Verify Org-native Relationship Contracts and chain-bound observations."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from ._relationship_contract_artifacts import observe_artifacts
from ._relationship_contract_authority import verify_relationship_authority
from ._relationship_contract_graph import (
    canonical_dependency_digest,
    validate_relationship_graph,
    validate_rfc_dependency_graph,
)
from ._relationship_contract_model import (
    RELATIONSHIP_CONTRACT_CANONICALIZATION,
    RELATIONSHIP_CONTRACT_RECEIPT_SCHEMA_ID,
    RelationshipContractVerificationError,
    require_mapping,
    require_receipt_chain_id,
    sha256_text,
    validate_rfc_clause,
)
from ._relationship_contract_org import verify_org_contracts
from ._relationship_contract_packet import validate_relationship_contract
from ._relationship_contract_projection import (
    query_rfc_section,
    resolve_rfc_source_path,
)

__all__ = [
    "RELATIONSHIP_CONTRACT_CANONICALIZATION",
    "RELATIONSHIP_CONTRACT_RECEIPT_SCHEMA_ID",
    "RelationshipContractVerificationError",
    "verify_relationship_contract",
]


def _load_relationship_contract(packet_path: Path) -> tuple[str, dict[str, Any]]:
    packet = require_mapping(json.loads(packet_path.read_bytes()), "handoff packet")
    receipt_chain_id = require_receipt_chain_id(packet.get("receiptChainId"))
    contract = validate_relationship_contract(packet.get("relationshipContract"))
    return receipt_chain_id, contract


def _index_clauses(raw_clauses: list[object]) -> dict[str, dict[str, Any]]:
    clauses: dict[str, dict[str, Any]] = {}
    for index, raw_clause in enumerate(raw_clauses):
        clause = validate_rfc_clause(raw_clause, index)
        clause_id = clause["clauseId"]
        if clause_id in clauses:
            raise RelationshipContractVerificationError(
                f"duplicate relationship section commitment id: {clause_id}"
            )
        clauses[clause_id] = clause
    validate_rfc_dependency_graph(clauses)
    return clauses


def _observe_clause(
    clause_id: str,
    clause: dict[str, Any],
    clauses: dict[str, dict[str, Any]],
    repository_root: Path,
    orgize: str | Path,
) -> dict[str, Any]:
    source_path = resolve_rfc_source_path(repository_root, clause["sourcePath"])
    normalized_raw = query_rfc_section(
        orgize,
        source_path,
        clause["outlinePath"],
        clause_id,
    )
    source_sha256 = sha256_text(normalized_raw)
    if source_sha256 != clause["sourceSha256"]:
        raise RelationshipContractVerificationError(
            f"{clause_id} sourceSha256 mismatch: "
            f"expected {clause['sourceSha256']}, observed {source_sha256}"
        )
    return {
        "clauseId": clause_id,
        "sourcePath": clause["sourcePath"],
        "outlinePath": clause["outlinePath"],
        "sourceSha256": source_sha256,
        "dependsOn": sorted(clause["dependsOn"]),
        "dependencySetSha256": canonical_dependency_digest(
            clause["dependsOn"], clauses
        ),
    }


def verify_relationship_contract(
    packet_path: Path,
    repository_root: Path,
    *,
    orgize: str | Path = "orgize",
) -> dict[str, Any]:
    """Verify the Relationship Contract application and return its receipt."""

    receipt_chain_id, contract = _load_relationship_contract(packet_path)
    profile = contract["applicationProfiles"]["sectionCommitments"]
    section_commitments = _index_clauses(profile["commitments"])
    validate_relationship_graph(
        contract["artifacts"],
        contract["relationships"],
        contract["evidenceGates"],
    )
    authority_receipt = verify_relationship_authority(
        contract["authority"],
        contract["artifacts"],
        contract["relationships"],
        repository_root,
        orgize,
    )
    artifact_receipts = observe_artifacts(
        contract["artifacts"], repository_root, orgize
    )
    contract_receipts = verify_org_contracts(
        [profile["orgContract"]], section_commitments, repository_root, orgize
    )
    observed = [
        _observe_clause(
            clause_id,
            section_commitments[clause_id],
            section_commitments,
            repository_root,
            orgize,
        )
        for clause_id in sorted(section_commitments)
    ]
    return {
        "schemaId": RELATIONSHIP_CONTRACT_RECEIPT_SCHEMA_ID,
        "schemaVersion": "1",
        "receiptChainId": receipt_chain_id,
        "authority": authority_receipt,
        "artifactCount": len(artifact_receipts),
        "artifacts": artifact_receipts,
        "relationshipCount": len(contract["relationships"]),
        "relationships": contract["relationships"],
        "evidenceGateCount": len(contract["evidenceGates"]),
        "evidenceGates": contract["evidenceGates"],
        "applicationProfiles": {
            "sectionCommitments": {
                "canonicalization": profile["canonicalization"],
                "orgContract": contract_receipts[0],
                "commitmentCount": len(observed),
                "commitments": observed,
            }
        },
    }
