"""Pin the semantic fact RFC and graph-turbo vocabulary boundaries."""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages" / "python" / "asp_graph_turbo" / "src"))

from asp_graph_turbo.profiles import DEFAULT_PROFILES  # noqa: E402
from asp_graph_turbo.protocol_vocabulary import (  # noqa: E402
    FACT_GRAPH_ONLY_RELATIONS,
    GRAPH_TURBO_INTERNAL_RELATIONS,
    ONTOLOGY_ONLY_RELATIONS,
    ONTOLOGY_TO_FACT_GRAPH_CONFIDENCE,
    ONTOLOGY_TO_FACT_GRAPH_FRESHNESS,
)


_ONTOLOGY_SCHEMA = _ROOT / "schemas" / "semantic-fact-ontology.v1.schema.json"
_FACT_GRAPH_SCHEMA = _ROOT / "schemas" / "semantic-fact-graph.v1.schema.json"
_DEPENDENCY_TOPOLOGY_SCHEMA = (
    _ROOT / "schemas" / "semantic-dependency-topology.v1.schema.json"
)


def _load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def _schema_enum(path: Path, *keys: str) -> frozenset[str]:
    node: Any = _load_json(path)
    for key in keys:
        node = node[key]
    return frozenset(node["enum"])


def test_semantic_fact_rfc_numbers_are_unique() -> None:
    rfcs: dict[str, list[str]] = defaultdict(list)

    for path in sorted((_ROOT / "docs" / "10-19-rfcs").iterdir()):
        if path.suffix != ".org":
            continue
        match = re.search(r"(?m)^#\+RFC:\s*(\d+)\s*$", path.read_text(encoding="utf-8"))
        if match:
            rfcs[match.group(1)].append(path.name)

    duplicates = {number: paths for number, paths in rfcs.items() if len(paths) > 1}
    assert duplicates == {}
    assert rfcs["013"] == ["10.13-software-criterion-policy.org"]
    assert rfcs["014"] == ["10.14-semantic-fact-frontier-feedback.org"]


def test_semantic_fact_relation_boundaries_are_explicit() -> None:
    ontology_relations = _schema_enum(_ONTOLOGY_SCHEMA, "$defs", "relation")
    fact_graph_relations = _schema_enum(
        _FACT_GRAPH_SCHEMA, "$defs", "edge", "properties", "relation"
    )

    assert ontology_relations - fact_graph_relations == ONTOLOGY_ONLY_RELATIONS
    assert fact_graph_relations - ontology_relations == FACT_GRAPH_ONLY_RELATIONS


def test_graph_turbo_profile_relations_are_declared_or_internal() -> None:
    ontology_relations = _schema_enum(_ONTOLOGY_SCHEMA, "$defs", "relation")
    fact_graph_relations = _schema_enum(
        _FACT_GRAPH_SCHEMA, "$defs", "edge", "properties", "relation"
    )
    dependency_topology_relations = _schema_enum(
        _DEPENDENCY_TOPOLOGY_SCHEMA, "$defs", "relation"
    )
    schema_relations = (
        ontology_relations | fact_graph_relations | dependency_topology_relations
    )
    profile_relations = frozenset(
        relation
        for profile in DEFAULT_PROFILES.values()
        for relation in profile.allowed_relations
    )

    assert profile_relations - schema_relations == GRAPH_TURBO_INTERNAL_RELATIONS


def test_confidence_and_freshness_mappings_cover_runtime_graph_values() -> None:
    ontology_confidence = _schema_enum(_ONTOLOGY_SCHEMA, "$defs", "confidence")
    fact_graph_confidence = _schema_enum(_FACT_GRAPH_SCHEMA, "$defs", "confidence")
    ontology_freshness = _schema_enum(_ONTOLOGY_SCHEMA, "$defs", "freshness")
    fact_graph_freshness = _schema_enum(_FACT_GRAPH_SCHEMA, "$defs", "freshness")

    assert frozenset(ONTOLOGY_TO_FACT_GRAPH_CONFIDENCE) == ontology_confidence
    assert (
        frozenset().union(*ONTOLOGY_TO_FACT_GRAPH_CONFIDENCE.values())
        == fact_graph_confidence
    )
    assert frozenset(ONTOLOGY_TO_FACT_GRAPH_FRESHNESS) == ontology_freshness
    assert (
        frozenset().union(*ONTOLOGY_TO_FACT_GRAPH_FRESHNESS.values())
        == fact_graph_freshness
    )
