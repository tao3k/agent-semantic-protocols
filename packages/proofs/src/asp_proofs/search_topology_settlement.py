# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Semantic admission for a Search projection of the reusable Project Topology.

The JSON schema validates transport shape. This module checks cross-record
claims that JSON Schema cannot establish. It performs no filesystem, network,
provider, proof-engine, or rendering work.
"""

from __future__ import annotations

from collections.abc import Mapping

from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError
from referencing import Registry, Resource

from ._topology_admission import SettlementError, require_topology
from .project_topology_library import validate_topology_library

_require = require_topology


def _validate_shape(
    packet: dict,
    schema: dict,
    topology_schema: dict,
    project_workspace_schema: dict,
) -> None:
    topology_schema_id = topology_schema.get("$id")
    _require(
        isinstance(topology_schema_id, str) and bool(topology_schema_id),
        "topology-schema-identity-missing",
    )
    project_workspace_schema_id = project_workspace_schema.get("$id")
    _require(
        isinstance(project_workspace_schema_id, str)
        and bool(project_workspace_schema_id),
        "project-workspace-schema-identity-missing",
    )
    registry = Registry().with_resources(
        [
            (topology_schema_id, Resource.from_contents(topology_schema)),
            (
                project_workspace_schema_id,
                Resource.from_contents(project_workspace_schema),
            ),
        ]
    )
    try:
        Draft202012Validator(schema, registry=registry).validate(packet)
    except ValidationError as exc:
        raise SettlementError("schema-invalid") from exc


def _index_nodes(packet: dict) -> tuple[dict[str, dict], set[str]]:
    nodes: dict[str, dict] = {}
    selectors: set[str] = set()
    for node in packet["nodes"]:
        node_id = node["id"]
        _require(node_id not in nodes, "duplicate-node-id")
        nodes[node_id] = node
        selector = node.get("selector")
        if selector is not None:
            selectors.add(selector)
        annotation = node.get("annotation")
        if annotation is not None:
            _require(
                annotation["bindingDigest"]
                == packet["binding"]["semanticTopologyDigest"],
                "annotation-binding-mismatch",
            )
    return nodes, selectors


def _validate_edges(packet: dict, nodes: dict[str, dict]) -> set[str]:
    authority_by_modality = {
        "parser-direct": ("provider-parser", "provider-witness"),
        "declared": ("source-contract", "contract-witness"),
        "derived": ("project-topology-inference.v1", "proof-dag"),
        "proposed": ("model-proposal", "model-premises"),
    }
    derived_count = 0
    proof_refs: set[str] = set()
    for edge in packet["edges"]:
        _require(edge["from"] in nodes and edge["to"] in nodes, "dangling-edge")
        expected = authority_by_modality[edge["modality"]]
        _require(
            (edge["producerAuthority"], edge["evidenceAuthority"]) == expected,
            "edge-authority-modality-mismatch",
        )
        if edge["modality"] == "derived":
            derived_count += 1
            proof_refs.add(edge["proofRef"])
    _require(
        derived_count == packet["inference"]["derivedRelationCount"],
        "derived-count-mismatch",
    )
    return proof_refs


def _index_coverage(packet: dict) -> dict[str, dict]:
    coverage: dict[str, dict] = {}
    for certificate in packet["coverageCertificates"]:
        certificate_id = certificate["id"]
        _require(certificate_id not in coverage, "duplicate-coverage-id")
        coverage[certificate_id] = certificate
    return coverage


def _validate_frontiers(
    packet: dict, nodes: dict[str, dict], coverage: dict[str, dict]
) -> None:
    for frontier in packet["frontiers"]:
        _require(frontier["anchor"] in nodes, "dangling-frontier")
        if frontier["state"] == "certified-missing":
            certificate = coverage.get(frontier["coverageRef"])
            _require(certificate is not None, "frontier-coverage-unresolved")
            _require(
                certificate["scope"] == "complete"
                and certificate["relation"] == frontier["relation"]
                and certificate["targetKind"] == frontier["targetKind"],
                "frontier-coverage-incomplete",
            )


def _validate_materialization(
    packet: dict, selectors: set[str], proof_refs: set[str]
) -> None:
    materialization = packet["materializationSet"]
    selected = materialization["selectors"]
    _require(selected == sorted(selected), "materialization-order")
    _require(
        all(selector in selectors for selector in selected),
        "materialization-selector-unavailable",
    )
    _require(
        all(proof in proof_refs for proof in materialization["proofDependencies"]),
        "proof-dependency-unresolved",
    )


def _validate_outcome(packet: dict) -> None:
    inference = packet["inference"]
    materialization = packet["materializationSet"]
    terminal = packet["terminal"]
    termination = inference["terminationKind"]
    result_state = packet["resultState"]

    if termination == "fixed-point":
        _require(
            inference["candidateRelationSetDigest"]
            == inference["nextRelationSetDigest"],
            "fixed-point-not-reached",
        )
        _require(terminal["state"] == "ready", "inference-terminal-mismatch")
        if materialization["state"] == "available":
            _require(result_state == "materializable", "result-state-mismatch")
        else:
            _require(result_state == "empty", "result-state-mismatch")
        return

    _require(materialization["state"] == "empty", "incomplete-materialization")
    _require(
        materialization["selectors"] == []
        and materialization["proofDependencies"] == [],
        "incomplete-materialization",
    )
    if termination == "budget-exhausted":
        _require(
            result_state == "incomplete" and terminal["state"] == "incomplete",
            "inference-terminal-mismatch",
        )
        _require(
            inference["candidateRelationSetDigest"]
            != inference["nextRelationSetDigest"],
            "budget-terminal-claims-fixed-point",
        )
    else:
        _require(
            result_state == "blocked" and terminal["state"] == "failed",
            "inference-terminal-mismatch",
        )
    _require(
        inference["reasonKind"] == terminal["reasonKind"],
        "inference-terminal-mismatch",
    )


def validate_settlement(
    packet: dict,
    schema: dict,
    topology_schema: dict,
    project_workspace_schema: dict,
) -> None:
    """Validate one immutable settlement against an explicit offline schema set."""

    _validate_shape(packet, schema, topology_schema, project_workspace_schema)
    nodes, selectors = _index_nodes(packet)
    proof_refs = _validate_edges(packet, nodes)
    coverage = _index_coverage(packet)
    _validate_frontiers(packet, nodes, coverage)
    _validate_materialization(packet, selectors, proof_refs)
    _validate_outcome(packet)


def validate_settlement_for_library(
    packet: dict,
    settlement_schema: dict,
    library: dict,
    library_schema: dict,
    project_workspace_schema: dict,
    admitted_receipts: Mapping[str, dict],
) -> None:
    """Admit a settlement against one independently admitted topology library.

    ``admitted_receipts`` is supplied by the receipt authority. Receipt
    objects embedded in the candidate cannot authorize themselves.
    """

    validate_settlement(
        packet, settlement_schema, library_schema, project_workspace_schema
    )
    validate_topology_library(
        library, library_schema, project_workspace_schema, admitted_receipts
    )

    generation = library["generation"]
    identities = library["identities"]
    expected_binding = {
        "projectWorkspaceIdentity": library["projectWorkspace"][
            "projectWorkspaceIdentity"
        ],
        "sourceGenerationDigest": library["sourceGenerationDigest"],
        "providerCatalogDigest": library["providerCatalogDigest"],
        "topologyLibraryDigest": library["libraryDigest"],
        "topologyGenerationDigest": generation["generationDigest"],
        "structuralTopologyDigest": identities["structuralTopologyDigest"],
        "semanticTopologyDigest": identities["semanticTopologyDigest"],
        "inferenceProgramDigest": identities["inferenceProgramDigest"],
    }
    _require(
        all(
            packet["binding"].get(field) == value
            for field, value in expected_binding.items()
        ),
        "topology-library-binding-mismatch",
    )

    receipts: dict[str, dict] = {}
    for receipt in library["semanticAdmissionReceipts"]:
        receipt_id = receipt["id"]
        _require(receipt_id not in receipts, "duplicate-semantic-admission-receipt")
        receipts[receipt_id] = receipt
    library_nodes = {node["id"]: node for node in library["nodes"]}

    for node in packet["nodes"]:
        annotation = node.get("annotation")
        if annotation is None or annotation["state"] != "accepted":
            continue
        receipt_ref = annotation["admissionReceiptRef"]
        receipt = receipts.get(receipt_ref)
        library_node = library_nodes.get(node["id"])
        _require(
            receipt is not None
            and admitted_receipts.get(receipt_ref) == receipt
            and receipt["annotationNodeId"] == node["id"]
            and receipt["semanticTopologyDigest"]
            == identities["semanticTopologyDigest"]
            and library_node is not None
            and library_node.get("annotation") == annotation,
            "annotation-admission-receipt-mismatch",
        )
