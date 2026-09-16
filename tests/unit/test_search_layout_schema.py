# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import copy
import json
from pathlib import Path

import jsonschema
import pytest


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = json.loads((ROOT / "schemas/search-layout.v1.schema.json").read_text())
FIXTURE = json.loads(
    (
        ROOT
        / "schemas/fixtures/search-layout/rg-tantivy-structural-scope.v1.json"
    ).read_text()
)


def test_default_search_layout_selects_explicit_structural_scope_before_fallback() -> None:
    jsonschema.Draft202012Validator(SCHEMA).validate(FIXTURE)
    steps = FIXTURE["root"]["steps"]
    assert steps[0]["kind"] == "shared-scope"
    assert [branch["operator"] for branch in steps[0]["branches"]] == [
        "rg",
        "tantivy",
    ]
    assert steps[0]["fusionPolicy"] == "intersection-rerank"
    assert steps[1]["kind"] == "structural-scope"
    assert steps[1]["selectionPolicy"] == "explicit-inputs-replace-fallback"
    assert [stage["operator"] for stage in steps[1]["explicitInputs"]] == [
        "syntax",
        "native-syntax",
    ]
    assert steps[1]["explicitFusionPolicy"] == "union"
    assert steps[1]["fallback"]["operator"] == "native-parser"
    assert steps[1]["fallback"]["inputBinding"] == "predecessor-scope"


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("root", "steps", 0, "kind"), "serial"),
        (("root", "steps", 0, "fusionPolicy"), "implicit"),
    ],
)
def test_search_layout_rejects_implicit_or_removed_composition(
    path: tuple[object, ...], value: str
) -> None:
    candidate = copy.deepcopy(FIXTURE)
    target: object = candidate
    for segment in path[:-1]:
        target = target[segment]  # type: ignore[index]
    target[path[-1]] = value  # type: ignore[index]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.Draft202012Validator(SCHEMA).validate(candidate)
