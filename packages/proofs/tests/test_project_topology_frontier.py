# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Truth-table tests for V1 Project Topology expected relations."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

import pytest

from asp_proofs.project_topology_library import validate_topology_library
from asp_proofs.search_topology_settlement import SettlementError


ROOT = Path(__file__).resolve().parents[3]
SCHEMA = json.loads(
    (ROOT / "schemas/project-topology-library.v1.schema.json").read_text()
)
WORKSPACE_SCHEMA = json.loads(
    (ROOT / "schemas/project-workspace-binding.v1.schema.json").read_text()
)
VALID = ROOT / "schemas/fixtures/project-topology-library/valid-polyglot.v1.json"


def _library() -> dict:
    return json.loads(VALID.read_text())


def _receipts(library: dict) -> dict[str, dict]:
    receipt = library["fromScratchRebuildReceipt"]
    return {receipt["id"]: deepcopy(receipt)}


def _validate(library: dict) -> None:
    validate_topology_library(library, SCHEMA, WORKSPACE_SCHEMA, _receipts(library))


def _unresolved(library: dict, coverage: str) -> None:
    library["expectedRelations"] = [
        {
            "id": "expected-config",
            "anchor": "registry",
            "target": "refresh",
            "relation": "READS_CONFIG",
            "targetKind": "Method",
            "depth": 1,
            "coverage": coverage,
        }
    ]
    states = {
        "none": ("unknown", "binding-not-established"),
        "partial": ("unknown", "coverage-open"),
        "complete": ("certified-missing", "complete-coverage-no-witness"),
    }
    state, reason = states[coverage]
    frontier = {
        "anchor": "registry",
        "target": "refresh",
        "relation": "READS_CONFIG",
        "targetKind": "Method",
        "depth": 1,
        "state": state,
        "reason": reason,
    }
    if coverage != "none":
        frontier["coverageRef"] = "coverage-config"
        library["coverageCertificates"] = [
            {
                "id": "coverage-config",
                "relation": "READS_CONFIG",
                "targetKind": "Method",
                "scope": coverage,
                "digest": "blake3-256:" + "9" * 64,
            }
        ]
    library["frontiers"] = [frontier]


@pytest.mark.parametrize("coverage", ["none", "partial", "complete"])
def test_unresolved_relation_truth_table_is_admitted(coverage: str) -> None:
    library = _library()
    _unresolved(library, coverage)
    _validate(library)


def test_wrong_frontier_reason_is_rejected() -> None:
    library = _library()
    _unresolved(library, "partial")
    library["frontiers"][0]["reason"] = "binding-not-established"
    with pytest.raises(SettlementError) as caught:
        _validate(library)
    assert caught.value.reason_kind == "topology-frontier-classification-mismatch"


def test_proposed_edge_cannot_close_source_owned_expectation() -> None:
    library = _library()
    _unresolved(library, "none")
    library["edges"].append(
        {
            "id": "proposed-config",
            "segmentId": None,
            "from": "registry",
            "to": "refresh",
            "relation": "READS_CONFIG",
            "modality": "proposed",
            "bindingDigest": library["identities"]["semanticTopologyDigest"],
            "witnesses": ["model-proposal-1"],
        }
    )
    _validate(library)


def test_positive_source_edge_forbids_a_frontier() -> None:
    library = _library()
    _unresolved(library, "none")
    library["expectedRelations"][0]["relation"] = "DECLARES"
    library["frontiers"][0]["relation"] = "DECLARES"
    with pytest.raises(SettlementError) as caught:
        _validate(library)
    assert caught.value.reason_kind == "topology-positive-frontier-conflict"


def test_coverage_inventory_cannot_float_free_of_a_frontier() -> None:
    library = _library()
    library["coverageCertificates"] = [
        {
            "id": "coverage-unused",
            "relation": "READS_CONFIG",
            "targetKind": "Method",
            "scope": "partial",
            "digest": "blake3-256:" + "8" * 64,
        }
    ]
    with pytest.raises(SettlementError) as caught:
        _validate(library)
    assert caught.value.reason_kind == "topology-frontier-coverage-extraneous"
