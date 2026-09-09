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


def fixture(kind: str) -> dict:
    name = (
        "valid-runtime-bound-ready.v1.json"
        if kind == "receipt"
        else "valid-runtime-bound.v1.json"
    )
    path = SCHEMAS / f"fixtures/query-playbook-materialization-{kind}" / name
    return json.loads(path.read_text(encoding="utf-8"))


@pytest.mark.parametrize("kind", ["request", "receipt"])
def test_runtime_bound_query_materialization_packet_is_valid(kind: str) -> None:
    schema_validator_for(
        SCHEMAS / f"query-playbook-materialization-{kind}.v1.schema.json"
    ).validate(fixture(kind))


@pytest.mark.parametrize("kind", ["request", "receipt"])
@pytest.mark.parametrize(
    "field",
    [
        "runtimeWorkspaceExecutionPublicationDigest",
        "runtimeBundleDigest",
    ],
)
def test_query_materialization_requires_execution_identities(
    kind: str, field: str
) -> None:
    packet = fixture(kind)
    del packet[field]
    with pytest.raises(ValidationError):
        schema_validator_for(
            SCHEMAS / f"query-playbook-materialization-{kind}.v1.schema.json"
        ).validate(packet)


def test_failed_query_terminal_cannot_expose_partial_materialization() -> None:
    packet = deepcopy(fixture("receipt"))
    packet["terminal"] = {
        "state": "failed",
        "terminalCount": 1,
        "reasonKind": "selector-not-materialized",
    }
    with pytest.raises(ValidationError):
        schema_validator_for(
            SCHEMAS / "query-playbook-materialization-receipt.v1.schema.json"
        ).validate(packet)


@pytest.mark.parametrize("field", ["recommendedNext", "nextAction", "plannerDecision"])
def test_query_materialization_receipt_rejects_planner_fields(field: str) -> None:
    packet = fixture("receipt")
    packet[field] = "query-another-selector"
    with pytest.raises(ValidationError):
        schema_validator_for(
            SCHEMAS / "query-playbook-materialization-receipt.v1.schema.json"
        ).validate(packet)


@pytest.mark.parametrize(
    "field",
    [
        "topologyLibraryDigest",
        "topologyClosureDigest",
        "gqlRelationships",
        "recommendedNext",
        "explanation",
    ],
)
def test_query_materialization_rejects_search_owned_fields(field: str) -> None:
    packet = fixture("receipt")
    packet["materializations"][0][field] = (
        [] if field == "gqlRelationships" else "retired"
    )
    with pytest.raises(ValidationError):
        schema_validator_for(
            SCHEMAS / "query-playbook-materialization-receipt.v1.schema.json"
        ).validate(packet)
