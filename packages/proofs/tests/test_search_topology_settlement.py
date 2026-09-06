"""Semantic admission for a Search projection of the Project Topology."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

import pytest

from asp_proofs.search_topology_settlement import (
    SettlementError,
    validate_settlement,
    validate_settlement_for_library,
)


ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads(
    (ROOT / "schemas/search-topology-settlement.v1.schema.json").read_text()
)
VALID = (
    ROOT
    / "schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
)
LIBRARY_SCHEMA = json.loads(
    (ROOT / "schemas/project-topology-library.v1.schema.json").read_text()
)
LIBRARY = ROOT / "schemas/fixtures/project-topology-library/valid-polyglot.v1.json"


@pytest.fixture()
def packet():
    return json.loads(VALID.read_text())


@pytest.fixture()
def library():
    return json.loads(LIBRARY.read_text())


def admitted_receipts(library, *semantic_receipt_ids):
    receipts = {
        library["fromScratchRebuildReceipt"]["id"]: deepcopy(
            library["fromScratchRebuildReceipt"]
        )
    }
    receipts.update(
        {
            receipt["id"]: deepcopy(receipt)
            for receipt in library["semanticAdmissionReceipts"]
            if receipt["id"] in semantic_receipt_ids
        }
    )
    return receipts


def test_valid_settlement_is_admitted(packet):
    validate_settlement(packet, SCHEMA, LIBRARY_SCHEMA)


def test_settlement_requires_an_explicit_offline_topology_schema(packet):
    topology_schema = deepcopy(LIBRARY_SCHEMA)
    topology_schema.pop("$id")
    with pytest.raises(SettlementError) as caught:
        validate_settlement(packet, SCHEMA, topology_schema)
    assert caught.value.reason_kind == "topology-schema-identity-missing"


@pytest.mark.parametrize(
    ("mutate", "reason"),
    [
        (lambda p: p["nodes"].append(deepcopy(p["nodes"][0])), "duplicate-node-id"),
        (lambda p: p["edges"][0].update({"to": "absent"}), "dangling-edge"),
        (lambda p: p["frontiers"][0].update({"anchor": "absent"}), "dangling-frontier"),
        (
            lambda p: p["materializationSet"]["selectors"].append(
                "rust://zzz/other.rs#item/function/other"
            ),
            "materialization-selector-unavailable",
        ),
        (
            lambda p: p["materializationSet"]["selectors"].reverse(),
            "materialization-order",
        ),
        (
            lambda p: p["fixedPoint"].update({"derivedRelationCount": 2}),
            "derived-count-mismatch",
        ),
        (
            lambda p: p["nodes"][2]["annotation"].update(
                {"bindingDigest": "blake3-256:" + "f" * 64}
            ),
            "annotation-binding-mismatch",
        ),
        (
            lambda p: p["materializationSet"].update(
                {"proofDependencies": ["absent-proof"]}
            ),
            "proof-dependency-unresolved",
        ),
    ],
)
def test_rejects_semantic_graph_drift(packet, mutate, reason):
    mutate(packet)
    with pytest.raises(SettlementError) as caught:
        validate_settlement(packet, SCHEMA, LIBRARY_SCHEMA)
    assert caught.value.reason_kind == reason


def test_certified_missing_requires_matching_complete_coverage(packet):
    packet["frontiers"][0].update(
        {"state": "certified-missing", "coverageRef": "coverage-admits"}
    )
    with pytest.raises(SettlementError) as caught:
        validate_settlement(packet, SCHEMA, LIBRARY_SCHEMA)
    assert caught.value.reason_kind == "frontier-coverage-incomplete"
    packet["coverageCertificates"][0]["scope"] = "complete"
    validate_settlement(packet, SCHEMA, LIBRARY_SCHEMA)


def test_direct_edges_cannot_claim_inference_authority(packet):
    edge = packet["edges"][0]
    edge["modality"] = "parser-direct"
    edge.pop("derivedBy")
    edge.pop("proofRef")
    edge["producerAuthority"] = "project-topology-inference.v1"
    with pytest.raises(SettlementError) as caught:
        validate_settlement(packet, SCHEMA, LIBRARY_SCHEMA)
    assert caught.value.reason_kind == "edge-authority-modality-mismatch"


def test_fixed_point_requires_identical_candidate_and_next_relation_sets(packet):
    packet["fixedPoint"]["nextRelationSetDigest"] = "blake3-256:" + "f" * 64
    with pytest.raises(SettlementError) as caught:
        validate_settlement(packet, SCHEMA, LIBRARY_SCHEMA)
    assert caught.value.reason_kind == "fixed-point-not-reached"


def test_settlement_is_jointly_bound_to_the_imported_topology_library(packet, library):
    validate_settlement_for_library(
        packet, SCHEMA, library, LIBRARY_SCHEMA, admitted_receipts(library)
    )
    packet["binding"]["providerCatalogDigest"] = "blake3-256:" + "f" * 64
    with pytest.raises(SettlementError) as caught:
        validate_settlement_for_library(
            packet, SCHEMA, library, LIBRARY_SCHEMA, admitted_receipts(library)
        )
    assert caught.value.reason_kind == "topology-library-binding-mismatch"


def test_accepted_annotation_requires_its_exact_admitted_receipt(packet, library):
    annotation = packet["nodes"][2]["annotation"]
    annotation.update(
        {"state": "accepted", "admissionReceiptRef": "annotation-admission-1"}
    )
    library["nodes"][3]["annotation"] = deepcopy(annotation)
    library["semanticAdmissionReceipts"] = [
        {
            "id": "annotation-admission-1",
            "annotationNodeId": "meaning",
            "semanticTopologyDigest": library["identities"]["semanticTopologyDigest"],
            "authority": "source-contract",
            "state": "admitted",
        }
    ]
    validate_settlement_for_library(
        packet,
        SCHEMA,
        library,
        LIBRARY_SCHEMA,
        admitted_receipts(library, "annotation-admission-1"),
    )
    library["semanticAdmissionReceipts"][0]["annotationNodeId"] = "publication"
    with pytest.raises(SettlementError) as caught:
        validate_settlement_for_library(
            packet,
            SCHEMA,
            library,
            LIBRARY_SCHEMA,
            admitted_receipts(library, "annotation-admission-1"),
        )
    assert caught.value.reason_kind == "annotation-admission-receipt-mismatch"


def test_self_declared_rebuild_digest_without_receipt_is_rejected(packet, library):
    library.pop("fromScratchRebuildReceipt")
    with pytest.raises(SettlementError) as caught:
        validate_settlement_for_library(packet, SCHEMA, library, LIBRARY_SCHEMA, {})
    assert caught.value.reason_kind == "topology-rebuild-receipt-missing"
