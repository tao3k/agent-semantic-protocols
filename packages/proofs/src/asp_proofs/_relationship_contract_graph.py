"""Verify the invalidation graph inside a Relationship Contract profile."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from typing import Any

from ._relationship_contract_model import (
    RelationshipContractVerificationError,
    sha256_text,
)


def canonical_dependency_digest(
    dependency_ids: Sequence[str], clauses: Mapping[str, Mapping[str, Any]]
) -> str:
    """Hash sorted direct dependencies with their recursive identities."""

    dependencies = [
        {
            "clauseId": dependency_id,
            "sourceSha256": clauses[dependency_id]["sourceSha256"],
            "dependencySetSha256": clauses[dependency_id]["dependencySetSha256"],
        }
        for dependency_id in sorted(dependency_ids)
    ]
    canonical = json.dumps(
        dependencies,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )
    return sha256_text(canonical)


def _require_dependencies_exist(
    clauses: Mapping[str, Mapping[str, Any]],
) -> None:
    for clause_id, clause in clauses.items():
        missing = sorted(set(clause["dependsOn"]) - clauses.keys())
        if missing:
            raise RelationshipContractVerificationError(
                f"{clause_id} has missing dependencies: " + ", ".join(missing)
            )


def _require_acyclic(clauses: Mapping[str, Mapping[str, Any]]) -> None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(clause_id: str, path: tuple[str, ...]) -> None:
        if clause_id in visiting:
            cycle_start = path.index(clause_id)
            cycle = path[cycle_start:] + (clause_id,)
            raise RelationshipContractVerificationError(
                "RFC commitment dependency cycle: " + " -> ".join(cycle)
            )
        if clause_id in visited:
            return
        visiting.add(clause_id)
        for dependency_id in clauses[clause_id]["dependsOn"]:
            visit(dependency_id, path + (clause_id,))
        visiting.remove(clause_id)
        visited.add(clause_id)

    for clause_id in sorted(clauses):
        visit(clause_id, ())


def _require_dependency_digests(
    clauses: Mapping[str, Mapping[str, Any]],
) -> None:
    for clause_id, clause in clauses.items():
        observed = canonical_dependency_digest(clause["dependsOn"], clauses)
        if observed != clause["dependencySetSha256"]:
            raise RelationshipContractVerificationError(
                f"{clause_id} dependencySetSha256 mismatch: "
                f"expected {clause['dependencySetSha256']}, observed {observed}"
            )


def validate_rfc_dependency_graph(
    clauses: Mapping[str, Mapping[str, Any]],
) -> None:
    """Reject unresolved, cyclic, or digest-inconsistent dependencies."""

    _require_dependencies_exist(clauses)
    _require_acyclic(clauses)
    _require_dependency_digests(clauses)


def _invalidation_arc(relationship: Mapping[str, Any]) -> tuple[str, str] | None:
    impact = relationship["impact"]
    if impact == "invalidates-subject":
        return str(relationship["object"]), str(relationship["subject"])
    if impact == "invalidates-object":
        return str(relationship["subject"]), str(relationship["object"])
    return None


def validate_relationship_graph(
    artifacts: Sequence[Mapping[str, Any]],
    relationships: Sequence[Mapping[str, Any]],
    evidence_gates: Sequence[str],
) -> None:
    """Require declared endpoints/gates and an acyclic changed-to-affected graph."""

    artifact_ids = {str(artifact["artifactId"]) for artifact in artifacts}
    declared_gates = set(evidence_gates)
    adjacency = {artifact_id: set() for artifact_id in artifact_ids}
    for relationship in relationships:
        relationship_id = str(relationship["relationshipId"])
        endpoints = {str(relationship["subject"]), str(relationship["object"])}
        missing_endpoints = sorted(endpoints - artifact_ids)
        if missing_endpoints:
            raise RelationshipContractVerificationError(
                f"{relationship_id} has missing artifact endpoints: "
                + ", ".join(missing_endpoints)
            )
        missing_gates = sorted(set(relationship["evidenceGates"]) - declared_gates)
        if missing_gates:
            raise RelationshipContractVerificationError(
                f"{relationship_id} has undeclared evidence gates: "
                + ", ".join(missing_gates)
            )
        arc = _invalidation_arc(relationship)
        if arc is not None:
            adjacency[arc[0]].add(arc[1])

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(artifact_id: str, path: tuple[str, ...]) -> None:
        if artifact_id in visiting:
            start = path.index(artifact_id)
            cycle = path[start:] + (artifact_id,)
            raise RelationshipContractVerificationError(
                "relationship invalidation cycle: " + " -> ".join(cycle)
            )
        if artifact_id in visited:
            return
        visiting.add(artifact_id)
        for affected in sorted(adjacency[artifact_id]):
            visit(affected, path + (artifact_id,))
        visiting.remove(artifact_id)
        visited.add(artifact_id)

    for artifact_id in sorted(artifact_ids):
        visit(artifact_id, ())
