# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Semantic admission tests for the reusable Project Topology library."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

import pytest

from asp_proofs.search_topology_settlement import (
    SettlementError,
    validate_topology_library,
)


ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads(
    (ROOT / "schemas/project-topology-library.v1.schema.json").read_text()
)
PROJECT_WORKSPACE_SCHEMA = json.loads(
    (ROOT / "schemas/project-workspace-binding.v1.schema.json").read_text()
)
VALID = ROOT / "schemas/fixtures/project-topology-library/valid-polyglot.v1.json"


@pytest.fixture()
def library():
    return json.loads(VALID.read_text())


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


def validate_library(library, receipts):
    validate_topology_library(library, SCHEMA, PROJECT_WORKSPACE_SCHEMA, receipts)


def test_complete_topology_library_requires_an_independently_admitted_rebuild(library):
    validate_library(library, admitted_receipts(library))
    with pytest.raises(SettlementError) as caught:
        validate_library(library, {})
    assert caught.value.reason_kind == "topology-rebuild-receipt-unadmitted"


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("sourceGenerationDigest", "blake3-256:" + "d" * 64),
        ("inferenceProgramDigest", "blake3-256:" + "e" * 64),
    ],
)
def test_rebuild_receipt_cannot_replay_across_source_or_program(
    library, field, value
):
    library["fromScratchRebuildReceipt"][field] = value
    admitted = admitted_receipts(library)
    with pytest.raises(SettlementError) as caught:
        validate_library(library, admitted)
    assert caught.value.reason_kind == "topology-rebuild-receipt-mismatch"


def test_accepted_semantics_require_the_exact_admitted_annotation_receipt(library):
    annotation = library["nodes"][3]["annotation"]
    annotation.update(
        {"state": "accepted", "admissionReceiptRef": "annotation-admission-1"}
    )
    library["semanticAdmissionReceipts"] = [
        {
            "id": "annotation-admission-1",
            "annotationNodeId": "meaning",
            "semanticTopologyDigest": library["identities"]["semanticTopologyDigest"],
            "authority": "source-contract",
            "state": "admitted",
        }
    ]
    admitted = admitted_receipts(library, "annotation-admission-1")
    validate_library(library, admitted)
    library["semanticAdmissionReceipts"][0]["semanticTopologyDigest"] = (
        "blake3-256:" + "f" * 64
    )
    with pytest.raises(SettlementError) as caught:
        validate_library(library, admitted)
    assert caught.value.reason_kind == "annotation-admission-receipt-mismatch"


def test_segment_inventory_must_exactly_cover_active_nodes(library):
    library["segments"][0]["nodeIds"].remove("refresh")
    with pytest.raises(SettlementError) as caught:
        validate_library(library, admitted_receipts(library))
    assert caught.value.reason_kind == "topology-segment-node-membership-mismatch"


def test_deleted_premise_cannot_leave_a_derived_descendant(library):
    derived = {
        "id": "derived-publication",
        "segmentId": None,
        "from": "refresh",
        "to": "publication",
        "relation": "DOCUMENTED_PATH",
        "modality": "derived",
        "bindingDigest": library["identities"]["inferenceProgramDigest"],
        "witnesses": ["proof-path-42"],
        "proofRef": "proof-path-42",
    }
    library["edges"].append(derived)
    library["closure"]["derivedEdgeIds"] = ["derived-publication"]
    library["closure"]["proofDependencies"] = [
        {"derivedEdgeId": "derived-publication", "premiseEdgeIds": ["declares-refresh"]}
    ]
    library["generation"]["removedEdgeIds"] = ["declares-refresh"]
    library["edges"] = [
        edge for edge in library["edges"] if edge["id"] != "declares-refresh"
    ]
    library["segments"][0]["edgeIds"].remove("declares-refresh")
    with pytest.raises(SettlementError) as caught:
        validate_library(library, admitted_receipts(library))
    assert caught.value.reason_kind == "topology-derived-dependency-invalidated"


def test_derived_closure_requires_a_dependency_record(library):
    changed = deepcopy(library)
    edge = changed["edges"][0]
    changed["segments"][0]["edgeIds"].remove(edge["id"])
    edge["segmentId"] = None
    edge["modality"] = "derived"
    edge["bindingDigest"] = changed["identities"]["inferenceProgramDigest"]
    edge["proofRef"] = "proof-declares-refresh"
    changed["closure"]["derivedEdgeIds"] = [edge["id"]]
    with pytest.raises(SettlementError) as caught:
        validate_library(changed, admitted_receipts(changed))
    assert caught.value.reason_kind == "topology-derived-dependency-missing"
