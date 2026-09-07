# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shape and negative contracts for a Search projection of Project Topology."""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas/search-topology-settlement.v1.schema.json"
VALID = (
    ROOT
    / "schemas/fixtures/search-topology-settlement/valid-derived-and-proposed.v1.json"
)


@pytest.fixture(scope="module")
def validator():
    result = schema_validator_for(SCHEMA)
    Draft202012Validator.check_schema(result.schema)
    return result


@pytest.fixture()
def packet():
    return json.loads(VALID.read_text())


def test_valid_single_gql_settlement(validator, packet):
    validator.validate(packet)


def test_json_selector_may_carry_a_compact_jq_projection(validator, packet):
    packet["nodes"].append(
        {
            "id": "json_packet",
            "language": "json",
            "kind": "JsonObject",
            "selector": "json://schemas/search.json#item/pointer/properties/packet",
            "projection": {
                "rank": 7,
                "depth": 2,
                "jq": "{artifactDigest,publicationNonce}",
            },
        }
    )
    validator.validate(packet)


@pytest.mark.parametrize(
    ("mutation", "path"),
    [
        (lambda p: p["edges"][0].pop("proofRef"), "derived edge without proof"),
        (
            lambda p: p["edges"][0].update({"modality": "parser-direct"}),
            "direct edge with derived provenance",
        ),
        (
            lambda p: p["rendering"].update({"ascentSourceExposed": True}),
            "Agent-facing Ascent leakage",
        ),
        (lambda p: p["rendering"].update({"gqlBlockCount": 2}), "multiple GQL results"),
        (lambda p: p["terminal"].update({"terminalCount": 2}), "multiple terminals"),
        (
            lambda p: p["inference"].update({"postRankingCertified": False}),
            "uncertified Python selection",
        ),
        (
            lambda p: p["inference"].pop("candidateRelationSetDigest"),
            "fixed point without candidate relation digest",
        ),
        (
            lambda p: p["inference"].pop("nextRelationSetDigest"),
            "fixed point without next relation digest",
        ),
        (
            lambda p: p["materializationSet"]["selectors"].append(
                p["materializationSet"]["selectors"][0]
            ),
            "duplicate Query selector",
        ),
        (
            lambda p: p["frontiers"][0].update({"state": "certified-missing"}),
            "missing without coverage certificate",
        ),
        (
            lambda p: p["nodes"][2]["annotation"].update(
                {"admissionReceiptRef": "receipt-1"}
            ),
            "proposed annotation pretending admission",
        ),
    ],
)
def test_rejects_boundary_violations(validator, packet, mutation, path):
    mutation(packet)
    with pytest.raises(ValidationError):
        validator.validate(packet)


def test_accepted_annotation_requires_admission_receipt(validator, packet):
    annotation = packet["nodes"][2]["annotation"]
    annotation["state"] = "accepted"
    with pytest.raises(ValidationError):
        validator.validate(packet)
    annotation["admissionReceiptRef"] = "org-contract-admission-1"
    validator.validate(packet)


def test_failed_terminal_requires_reason_kind(validator, packet):
    packet["terminal"]["state"] = "failed"
    with pytest.raises(ValidationError):
        validator.validate(packet)
    packet["terminal"]["reasonKind"] = "topology-proof-unresolved"
    validator.validate(packet)


def test_shape_validation_does_not_claim_graph_reference_admission(validator, packet):
    changed = deepcopy(packet)
    changed["edges"][0]["to"] = "not_declared"
    validator.validate(changed)
