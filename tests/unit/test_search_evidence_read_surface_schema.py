# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Shape admission for compact, language-neutral Search read surfaces."""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, ValidationError

from unit.schema_validation import schema_validator_for


SCHEMA_PATH = (
    Path(__file__).resolve().parents[2]
    / "schemas"
    / "search-evidence-read-surface.v1.schema.json"
)


@pytest.fixture(scope="module")
def validator():
    result = schema_validator_for(SCHEMA_PATH)
    Draft202012Validator.check_schema(result.schema)
    return result


def packet(node):
    return {
        "schemaId": "agent.semantic-protocols.search-evidence-read-surface",
        "schemaVersion": "1",
        "binding": "blake3-256:" + "a" * 64,
        "contextBudgetLines": 80,
        "nodes": [node],
        "edges": [],
    }


def source(**surface):
    return {"id": "s", "kind": "SourceHit", "witness": "w1", **surface}


@pytest.mark.parametrize(
    "node",
    [
        {"id": "r", "kind": "Function", "witness": "w1", "selector": "rust://src/lib.rs#item/function/run"},
        {"id": "h", "kind": "Heading", "witness": "w1", "selector": "org://docs/design.org#heading/Design"},
        source(path="config/runtime.json", jq=".runtime | {artifactDigest}"),
        source(path="notes/design.txt", lines=[[80, 159]]),
        source(path="文档/my notes.txt", lines=[[1, 2], [8, 10]]),
    ],
)
def test_accepts_each_supported_read_surface(validator, node):
    validator.validate(packet(node))


@pytest.mark.parametrize(
    "node",
    [
        source(selector="rust://src/lib.rs#x", path="src/lib.rs", lines=[[1, 2]]),
        source(path="config/a.json", jq=".a", lines=[[1, 2]]),
        source(selector="rust://src/lib.rs#x", jq=".a"),
        source(path="config/a.json"),
        source(jq=".a"),
        source(path="config/a.json", jq=""),
        source(path="notes/a.txt", lines=[]),
        source(path="notes/a.txt", lines=[[0, 1]]),
        source(path="notes/a.txt", lines=[[1, 2, 3]]),
        source(path="notes/a.txt", lines=[[1.5, 2]]),
        source(selector="not-a-selector"),
        source(selector="rust://"),
        source(selector="rust://src/a b.rs"),
        source(selector="rust://src/lib.rs\n"),
        {**source(path="config/a.json", jq=".a"), "kind": "Function"},
        {**source(path="notes/a.txt", lines=[[1, 2]]), "kind": "Heading"},
        {**source(path="notes/a.txt", lines=[[1, 2]]), "unexpected": True},
        {**source(path="notes/a.txt", lines=[[1, 2]]), "id": "S"},
        {**source(path="notes/a.txt", lines=[[1, 2]]), "id": "s\n"},
        {**source(path="notes/a.txt", lines=[[1, 2]]), "witness": ""},
    ],
)
def test_rejects_invalid_or_ambiguous_node_shapes(validator, node):
    with pytest.raises(ValidationError):
        validator.validate(packet(node))


@pytest.mark.parametrize("path", ["/etc/config.json", "../config.json", "a/../b", "./a", "a/./b", "a/..", "a/.", "a\\b", "a//b", ""])
def test_rejects_non_relative_or_noncanonical_paths(validator, path):
    with pytest.raises(ValidationError):
        validator.validate(packet(source(path=path, jq=".")))


@pytest.mark.parametrize(
    ("key", "value"),
    [
        ("schemaVersion", 1),
        ("binding", "blake3-256:" + "A" * 64),
        ("binding", "blake3-256:" + "a" * 63),
        ("binding", "blake3-256:" + "a" * 64 + "\n"),
        ("contextBudgetLines", 0),
        ("contextBudgetLines", 101),
        ("nodes", []),
        ("unexpected", True),
    ],
)
def test_rejects_invalid_packet_fields(validator, key, value):
    item = packet(source(path="a.json", jq="."))
    item[key] = value
    with pytest.raises(ValidationError):
        validator.validate(item)


def test_validates_edge_shape_without_claiming_semantic_admission(validator):
    item = packet(source(path="a.json", jq="."))
    item["edges"] = [{"from": "s", "to": "missing", "relation": "SUPPORTS", "witness": "edge1"}]
    validator.validate(item)
    for replacement in [{"relation": "GUESSED"}, {"from": "Bad"}, {"witness": ""}, {"unknown": 1}]:
        invalid = deepcopy(item)
        invalid["edges"][0].update(replacement)
        with pytest.raises(ValidationError):
            validator.validate(invalid)


def test_order_uniqueness_and_budget_are_semantic_validator_obligations(validator):
    item = packet(source(path="a.txt", lines=[[9, 2], [1, 100]]))
    item["nodes"].append(deepcopy(item["nodes"][0]))
    validator.validate(item)
