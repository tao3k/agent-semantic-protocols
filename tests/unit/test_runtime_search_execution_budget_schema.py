# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads(
    (ROOT / "schemas/runtime-search-execution-budget.v1.schema.json").read_text()
)
VALIDATOR = Draft202012Validator(SCHEMA)


def receipt() -> dict[str, object]:
    return {
        "schemaId": "agent.semantic-protocols.runtime-search-execution-budget",
        "schemaVersion": "1",
        "authority": "runtime-generation",
        "generationDigest": f"blake3-256:{'a' * 64}",
        "cardinality": {
            "indexedOwnerCount": 700,
            "corpusByteCount": 90000,
            "corpusLineCount": 8000,
            "graphNodeCount": 600,
            "graphEdgeCount": 4000,
        },
        "limits": {
            "rgMatchCount": 8000,
            "lexicalOwnerCount": 700,
            "syntaxSelectorCount": 90000,
            "graphCandidateOwnerCount": 700,
            "graphDepth": 16,
            "graphNodeCount": 256,
            "graphEdgeCount": 1024,
            "graphResultCount": 30,
            "evidenceItemCount": 30,
        },
    }


def test_runtime_search_execution_budget_schema_accepts_typed_receipt() -> None:
    Draft202012Validator.check_schema(SCHEMA)
    VALIDATOR.validate(receipt())


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("authority",), "agent-planner"),
        (("limits", "rgMatchCount"), 0),
        (("limits", "graphNodeCount"), 257),
        (("limits", "evidenceItemCount"), 31),
    ],
)
def test_runtime_search_execution_budget_schema_rejects_widening(
    path: tuple[str, ...], value: object
) -> None:
    invalid = copy.deepcopy(receipt())
    target = invalid
    for component in path[:-1]:
        target = target[component]  # type: ignore[index,assignment]
    target[path[-1]] = value  # type: ignore[index]
    with pytest.raises(ValidationError):
        VALIDATOR.validate(invalid)
