# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path

from jsonschema.exceptions import ValidationError
import pytest

from unit.schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "schemas"
FIXTURE = SCHEMAS / "fixtures/project-topology-inference-receipt/valid-reachability.v1.json"


@pytest.fixture()
def receipt() -> dict:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def validator():
    return schema_validator_for(SCHEMAS / "project-topology-inference-receipt.v1.schema.json")


def test_complete_topology_inference_receipt_is_valid(receipt) -> None:
    validator().validate(receipt)


@pytest.mark.parametrize("field", ["relation", "ruleId"])
def test_relation_sensitive_receipt_cannot_erase_derived_identity(receipt, field) -> None:
    changed = deepcopy(receipt)
    del changed["relationships"][0][field]
    with pytest.raises(ValidationError):
        validator().validate(changed)


def test_relation_sensitive_receipt_requires_ascent_semantic_digest(receipt) -> None:
    del receipt["ascentSemanticDigest"]
    with pytest.raises(ValidationError):
        validator().validate(receipt)


@pytest.mark.parametrize("field", ["nextAction", "recommendedNext", "planner", "activationGeneration"])
def test_topology_inference_receipt_rejects_planner_and_fence_fields(receipt, field) -> None:
    receipt[field] = "forbidden"
    with pytest.raises(ValidationError):
        validator().validate(receipt)


def test_topology_inference_receipt_requires_exactly_one_terminal(receipt) -> None:
    changed = deepcopy(receipt)
    changed["terminal"]["terminalCount"] = 2
    with pytest.raises(ValidationError):
        validator().validate(changed)


def test_topology_inference_receipt_cannot_omit_premise_witnesses(receipt) -> None:
    receipt["relationships"][1]["premiseEdgeIds"] = []
    with pytest.raises(ValidationError):
        validator().validate(receipt)
