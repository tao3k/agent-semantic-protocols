# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Frontier and coverage invariants for the V1 Search topology settlement."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from asp_proofs.search_topology_settlement import (
    SettlementError,
    validate_settlement as _validate_settlement,
)


_ROOT = Path(__file__).resolve().parents[3]
_VALID = (
    _ROOT
    / "schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
)
_SCHEMA = json.loads(
    (_ROOT / "schemas/search-topology-settlement.v1.schema.json").read_text()
)
_LIBRARY_SCHEMA = json.loads(
    (_ROOT / "schemas/project-topology-library.v1.schema.json").read_text()
)
_WORKSPACE_SCHEMA = json.loads(
    (_ROOT / "schemas/project-workspace-binding.v1.schema.json").read_text()
)


def _packet() -> dict:
    return json.loads(_VALID.read_text())


def _validate(packet: dict) -> None:
    _validate_settlement(
        packet,
        _SCHEMA,
        _LIBRARY_SCHEMA,
        _WORKSPACE_SCHEMA,
    )


def test_partial_frontier_requires_its_matching_coverage_reference() -> None:
    packet = _packet()
    del packet["frontiers"][0]["coverageRef"]
    with pytest.raises(SettlementError) as caught:
        _validate(packet)
    assert caught.value.reason_kind == "frontier-classification-mismatch"


def test_extraneous_coverage_is_not_part_of_the_projection() -> None:
    packet = _packet()
    packet["coverageCertificates"].append(
        {
            "id": "coverage-unused",
            "relation": "CALLS",
            "targetKind": "Method",
            "scope": "partial",
            "digest": "blake3-256:" + "a" * 64,
        }
    )
    with pytest.raises(SettlementError) as caught:
        _validate(packet)
    assert caught.value.reason_kind == "frontier-coverage-extraneous"
