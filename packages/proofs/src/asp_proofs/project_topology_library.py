"""Semantic admission for one reusable Project Topology library generation."""

from __future__ import annotations

from collections.abc import Mapping

from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError

from ._topology_admission import SettlementError, require_topology


def _validate_shape_and_rebuild(
    library: dict, schema: dict, admitted_receipts: Mapping[str, dict]
) -> None:
    require_topology(
        "fromScratchRebuildReceipt" in library, "topology-rebuild-receipt-missing"
    )
    try:
        Draft202012Validator(schema).validate(library)
    except ValidationError as exc:
        raise SettlementError("topology-library-schema-invalid") from exc
    generation = library["generation"]
    identities = library["identities"]
    rebuild = library["fromScratchRebuildReceipt"]
    require_topology(
        admitted_receipts.get(rebuild["id"]) == rebuild,
        "topology-rebuild-receipt-unadmitted",
    )
    require_topology(
        rebuild["sourceGenerationDigest"] == library["sourceGenerationDigest"]
        and rebuild["inferenceProgramDigest"]
        == identities["inferenceProgramDigest"]
        and rebuild["topologyGenerationDigest"] == generation["generationDigest"]
        and rebuild["recomputedLibraryDigest"] == library["libraryDigest"]
        and generation["fromScratchEquivalentDigest"] == library["libraryDigest"],
        "topology-rebuild-receipt-mismatch",
    )


def _index_segments(library: dict) -> dict[str, dict]:
    segments: dict[str, dict] = {}
    for segment in library["segments"]:
        require_topology(segment["id"] not in segments, "topology-duplicate-segment-id")
        segments[segment["id"]] = segment
    require_topology(
        all(item in segments for item in library["generation"]["rebuiltSegmentIds"]),
        "topology-rebuilt-segment-unresolved",
    )
    return segments


def _index_nodes(library: dict, segments: dict[str, dict]) -> dict[str, dict]:
    nodes: dict[str, dict] = {}
    by_segment: dict[str, set[str]] = {key: set() for key in segments}
    for node in library["nodes"]:
        require_topology(node["id"] not in nodes, "topology-duplicate-node-id")
        require_topology(
            node["segmentId"] in segments, "topology-node-segment-unresolved"
        )
        nodes[node["id"]] = node
        by_segment[node["segmentId"]].add(node["id"])
    require_topology(
        all(item not in nodes for item in library["generation"]["removedNodeIds"]),
        "topology-removed-node-still-active",
    )
    require_topology(
        all(
            set(segment["nodeIds"]) == by_segment[key]
            for key, segment in segments.items()
        ),
        "topology-segment-node-membership-mismatch",
    )
    return nodes


def _validate_annotations(
    library: dict, nodes: dict[str, dict], admitted_receipts: Mapping[str, dict]
) -> None:
    semantic_digest = library["identities"]["semanticTopologyDigest"]
    receipts: dict[str, dict] = {}
    for receipt in library["semanticAdmissionReceipts"]:
        require_topology(
            receipt["id"] not in receipts, "duplicate-semantic-admission-receipt"
        )
        receipts[receipt["id"]] = receipt
    for node_id, node in nodes.items():
        annotation = node.get("annotation")
        if annotation is None:
            continue
        require_topology(
            annotation["bindingDigest"] == semantic_digest,
            "topology-annotation-binding-mismatch",
        )
        if annotation["state"] != "accepted":
            continue
        receipt_ref = annotation["admissionReceiptRef"]
        receipt = receipts.get(receipt_ref)
        require_topology(
            receipt is not None
            and admitted_receipts.get(receipt_ref) == receipt
            and receipt["annotationNodeId"] == node_id
            and receipt["semanticTopologyDigest"] == semantic_digest,
            "annotation-admission-receipt-mismatch",
        )


def _index_edges(
    library: dict, segments: dict[str, dict], nodes: dict[str, dict]
) -> tuple[dict[str, dict], set[str]]:
    edges: dict[str, dict] = {}
    by_segment: dict[str, set[str]] = {key: set() for key in segments}
    derived: set[str] = set()
    identities = library["identities"]
    for edge in library["edges"]:
        edge_id = edge["id"]
        require_topology(edge_id not in edges, "topology-duplicate-edge-id")
        require_topology(
            edge["segmentId"] in segments, "topology-edge-segment-unresolved"
        )
        require_topology(
            edge["from"] in nodes and edge["to"] in nodes, "topology-dangling-edge"
        )
        edges[edge_id] = edge
        by_segment[edge["segmentId"]].add(edge_id)
        bindings = {
            "parser-direct": segments[edge["segmentId"]]["skeletonDigest"],
            "declared": segments[edge["segmentId"]]["skeletonDigest"],
            "derived": identities["inferenceProgramDigest"],
            "proposed": identities["semanticTopologyDigest"],
        }
        require_topology(
            edge["bindingDigest"] == bindings[edge["modality"]],
            "topology-edge-binding-mismatch",
        )
        if edge["modality"] == "derived":
            derived.add(edge_id)
    removed = set(library["generation"]["removedEdgeIds"])
    require_topology(not (removed & edges.keys()), "topology-removed-edge-still-active")
    require_topology(
        all(
            set(segment["edgeIds"]) == by_segment[key]
            for key, segment in segments.items()
        ),
        "topology-segment-edge-membership-mismatch",
    )
    return edges, derived


def _validate_closure(library: dict, edges: dict[str, dict], derived: set[str]) -> None:
    closure = library["closure"]
    require_topology(
        set(closure["derivedEdgeIds"]) == derived, "topology-derived-closure-mismatch"
    )
    dependencies: dict[str, set[str]] = {}
    for item in closure["proofDependencies"]:
        derived_id = item["derivedEdgeId"]
        require_topology(
            derived_id not in dependencies, "topology-derived-dependency-duplicate"
        )
        dependencies[derived_id] = set(item["premiseEdgeIds"])
    require_topology(
        set(dependencies) == derived, "topology-derived-dependency-missing"
    )
    removed = set(library["generation"]["removedEdgeIds"])
    for premises in dependencies.values():
        require_topology(
            not (premises & removed), "topology-derived-dependency-invalidated"
        )
        require_topology(
            premises <= edges.keys(), "topology-derived-dependency-unresolved"
        )


def validate_topology_library(
    library: dict, schema: dict, admitted_receipts: Mapping[str, dict]
) -> None:
    """Validate one complete library against independently admitted receipts."""

    _validate_shape_and_rebuild(library, schema, admitted_receipts)
    segments = _index_segments(library)
    nodes = _index_nodes(library, segments)
    _validate_annotations(library, nodes, admitted_receipts)
    edges, derived = _index_edges(library, segments, nodes)
    _validate_closure(library, edges, derived)
