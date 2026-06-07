"""Graph-turbo projection tests for semantic fact ontology fixtures."""

from __future__ import annotations

import json
from pathlib import Path

from asp_graph_turbo import (
    TypedGraph,
    ontology_catalog_to_graph_request,
    rank_frontier,
    render_compact,
)
from unit.schema_validation import schema_validator_for


_REPO_ROOT = Path(__file__).resolve().parents[2]
_ONTOLOGY_FIXTURES = (
    _REPO_ROOT / "schemas" / "semantic-fact-ontology.fixtures.v1.json"
)
_GRAPH_TURBO_REQUEST_SCHEMA = (
    _REPO_ROOT / "schemas" / "semantic-graph-turbo-request.v1.schema.json"
)

_EXPECTED_IMPLS = {
    ("rust", "sequence"): "Vec",
    ("rust", "map"): "HashMap",
    ("typescript", "sequence"): "Array",
    ("typescript", "map"): "Map",
    ("python", "sequence"): "list",
    ("python", "map"): "dict",
    ("julia", "sequence"): "Vector",
    ("julia", "map"): "Dict",
}


def _load_ontology_catalog() -> dict[str, object]:
    return json.loads(_ONTOLOGY_FIXTURES.read_text(encoding="utf-8"))


def test_ontology_catalog_projects_to_valid_graph_turbo_request() -> None:
    request = ontology_catalog_to_graph_request(
        _load_ontology_catalog(),
        query="rust Vec collection field",
    )

    schema_validator_for(_GRAPH_TURBO_REQUEST_SCHEMA).validate(request)

    graph = TypedGraph.from_packet(request)
    result = rank_frontier(
        graph,
        profile=request["profile"],
        seeds=request["seedIds"],
        limit=request["budget"],
        kind_budgets=request["kindBudgets"],
        cache_enabled=False,
    )
    compact = render_compact(result)

    ranked_ids = [node.id for node in result.ranked_nodes]
    assert "rust:field:cache_entries" in ranked_ids
    assert "rust:type:vec_entry" in ranked_ids
    assert "rust:collection:vec_entry" in ranked_ids
    assert ranked_ids.index("rust:field:cache_entries") < ranked_ids.index(
        "rust:collection:vec_entry"
    )
    assert "selector=" in compact
    assert "symbol=entries" in compact


def test_ontology_fixture_matrix_is_queryable_as_homologous_graph_facts() -> None:
    catalog = _load_ontology_catalog()

    for (language_id, collection_family), collection_impl in _EXPECTED_IMPLS.items():
        request = ontology_catalog_to_graph_request(
            catalog,
            query=f"{language_id} {collection_impl} collection field",
            budget=6,
        )
        graph = TypedGraph.from_packet(request)
        result = rank_frontier(
            graph,
            profile=request["profile"],
            seeds=request["seedIds"],
            limit=request["budget"],
            kind_budgets=request["kindBudgets"],
            cache_enabled=False,
        )

        matching_field_nodes = [
            node
            for node in result.ranked_nodes
            if node.kind == "field"
            and node.id.startswith(f"{language_id}:field:")
            and collection_impl in node.value
            and node.fields["fields"]["collectionFamily"] == collection_family
        ]
        assert matching_field_nodes, (language_id, collection_family, collection_impl)
