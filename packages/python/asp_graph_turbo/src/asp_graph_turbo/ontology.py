"""Semantic fact ontology adapters for graph-turbo requests."""

from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any

from .constants import ALGORITHM_ID

_TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")

_NODE_TOP_LEVEL_KEYS = {
    "id",
    "kind",
    "role",
    "value",
    "action",
    "weight",
    "locator",
    "location",
    "path",
    "owner",
    "ownerPath",
    "symbol",
    "matchText",
    "syntaxQuery",
    "name",
    "startLine",
    "endLine",
    "start",
    "end",
}

_EDGE_TOP_LEVEL_KEYS = {"source", "target", "relation", "weight"}


def ontology_catalog_to_graph_request(
    catalog: Mapping[str, Any],
    *,
    query: str,
    seed_id: str = "query:semantic-fact-ontology",
    profile: str = "owner-query",
    budget: int = 8,
) -> dict[str, object]:
    """Project a semantic-fact ontology catalog into a graph-turbo request."""

    graph = ontology_catalog_to_graph_packet(catalog, query=query, seed_id=seed_id)
    return {
        "schemaId": "agent.semantic-protocols.semantic-graph-turbo-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "surface": "search-typed-frontier",
        "queryTerms": [query],
        "profile": profile,
        "algorithm": ALGORITHM_ID,
        "seedIds": [seed_id],
        "budget": budget,
        "kindBudgets": {
            "query": 1,
            "field": 3,
            "type": 3,
            "collection": 3,
            "hot": 3,
            "item": 3,
        },
        "windowMerge": {"enabled": True, "maxGapLines": 8},
        "pathBudget": 4,
        "pathMaxHops": 4,
        "cache": {"enabled": True},
        "graph": graph,
    }


def ontology_catalog_to_graph_packet(
    catalog: Mapping[str, Any],
    *,
    query: str | None = None,
    seed_id: str = "query:semantic-fact-ontology",
) -> dict[str, object]:
    """Flatten ontology fixtures into graph-turbo-compatible nodes and edges."""

    nodes: list[dict[str, object]] = []
    edges: list[dict[str, object]] = []
    if query is not None:
        nodes.append(_query_node(seed_id, query))

    seen_nodes: set[str] = {seed_id} if query is not None else set()
    for fixture in _fixtures(catalog):
        fixture_fields = _fixture_fields(fixture)
        fixture_nodes, match_edges = _fixture_graph_nodes(
            fixture,
            fixture_fields,
            query=query,
            seed_id=seed_id,
            seen_nodes=seen_nodes,
        )
        nodes.extend(fixture_nodes)
        edges.extend(match_edges)
        edges.extend(_fixture_graph_edges(fixture, fixture_fields))

    return {"nodes": nodes, "edges": edges}


def _query_node(seed_id: str, query: str) -> dict[str, object]:
    return {"id": seed_id, "kind": "query", "role": "term", "value": query}


def _fixture_graph_nodes(
    fixture: Mapping[str, Any],
    fixture_fields: dict[str, object],
    *,
    query: str | None,
    seed_id: str,
    seen_nodes: set[str],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    nodes = []
    match_edges = []
    for raw_node in _nodes(fixture):
        graph_node = _node_to_graph_node(raw_node, fixture_fields)
        node_id = str(graph_node["id"])
        _record_seen_node(node_id, seen_nodes)
        nodes.append(graph_node)
        if query is not None and _matches_query(query, graph_node):
            match_edges.append(_query_match_edge(seed_id, node_id, fixture_fields))
    return nodes, match_edges


def _fixture_graph_edges(
    fixture: Mapping[str, Any],
    fixture_fields: dict[str, object],
) -> list[dict[str, object]]:
    return [
        _edge_to_graph_edge(raw_edge, fixture_fields) for raw_edge in _edges(fixture)
    ]


def _record_seen_node(node_id: str, seen_nodes: set[str]) -> None:
    if node_id in seen_nodes:
        raise ValueError(f"duplicate ontology node id: {node_id}")
    seen_nodes.add(node_id)


def _query_match_edge(
    seed_id: str,
    node_id: str,
    fixture_fields: dict[str, object],
) -> dict[str, object]:
    return {
        "source": seed_id,
        "target": node_id,
        "relation": "matches",
        "fields": {
            "provenance": "parser",
            "confidence": "exact",
            "fixtureId": fixture_fields["fixtureId"],
        },
    }


def _fixtures(catalog: Mapping[str, Any]) -> tuple[Mapping[str, Any], ...]:
    fixtures = catalog.get("fixtures")
    if not isinstance(fixtures, list) or not all(
        isinstance(item, Mapping) for item in fixtures
    ):
        raise ValueError("ontology catalog fixtures must be a list of objects")
    return tuple(fixtures)


def _nodes(fixture: Mapping[str, Any]) -> tuple[Mapping[str, Any], ...]:
    nodes = fixture.get("nodes")
    if not isinstance(nodes, list) or not all(
        isinstance(item, Mapping) for item in nodes
    ):
        raise ValueError("ontology fixture nodes must be a list of objects")
    return tuple(nodes)


def _edges(fixture: Mapping[str, Any]) -> tuple[Mapping[str, Any], ...]:
    edges = fixture.get("edges")
    if not isinstance(edges, list) or not all(
        isinstance(item, Mapping) for item in edges
    ):
        raise ValueError("ontology fixture edges must be a list of objects")
    return tuple(edges)


def _fixture_fields(fixture: Mapping[str, Any]) -> dict[str, object]:
    return {
        "fixtureId": _string_field(fixture, "fixtureId"),
        "languageId": _string_field(fixture, "languageId"),
        "providerId": _string_field(fixture, "providerId"),
        "collectionFamily": _string_field(fixture, "collectionFamily"),
        "collectionImpl": _string_field(fixture, "collectionImpl"),
        "queryIntent": str(fixture.get("queryIntent") or ""),
    }


def _node_to_graph_node(
    raw_node: Mapping[str, Any], fixture_fields: Mapping[str, object]
) -> dict[str, object]:
    graph_node = {
        key: value for key, value in raw_node.items() if key in _NODE_TOP_LEVEL_KEYS
    }
    if "matchText" not in graph_node:
        graph_node["matchText"] = str(raw_node.get("value") or "")
    semantic_fields = {
        key: value for key, value in raw_node.items() if key not in _NODE_TOP_LEVEL_KEYS
    }
    graph_node["fields"] = {
        **fixture_fields,
        **semantic_fields,
        "semanticFactKind": raw_node.get("kind"),
    }
    return graph_node


def _edge_to_graph_edge(
    raw_edge: Mapping[str, Any], fixture_fields: Mapping[str, object]
) -> dict[str, object]:
    graph_edge = {
        key: value for key, value in raw_edge.items() if key in _EDGE_TOP_LEVEL_KEYS
    }
    edge_fields = {
        key: value for key, value in raw_edge.items() if key not in _EDGE_TOP_LEVEL_KEYS
    }
    graph_edge["fields"] = {**fixture_fields, **edge_fields}
    return graph_edge


def _matches_query(query: str, node: Mapping[str, object]) -> bool:
    query_tokens = _tokens(query)
    if not query_tokens:
        return False
    if not _matches_explicit_kind(query_tokens, node):
        return False
    candidate_tokens = {
        token for value in _candidate_text(node) for token in _tokens(value)
    }
    return all(token in candidate_tokens for token in query_tokens)


def _matches_explicit_kind(
    query_tokens: tuple[str, ...], node: Mapping[str, object]
) -> bool:
    kind = node.get("kind")
    if "field" in query_tokens:
        return kind == "field"
    if "type" in query_tokens:
        return kind == "type"
    if "collection" in query_tokens:
        return kind == "collection"
    return True


def _candidate_text(node: Mapping[str, object]) -> tuple[str, ...]:
    text = [
        str(node.get("id") or ""),
        str(node.get("kind") or ""),
        str(node.get("role") or ""),
        str(node.get("value") or ""),
        str(node.get("symbol") or ""),
        str(node.get("matchText") or ""),
    ]
    fields = node.get("fields")
    if isinstance(fields, Mapping):
        text.extend(_field_text(fields))
    return tuple(text)


def _field_text(fields: Mapping[str, object]) -> tuple[str, ...]:
    values: list[str] = []
    for value in fields.values():
        if isinstance(value, Mapping):
            values.extend(str(item) for item in value.values())
        elif isinstance(value, list):
            values.extend(str(item) for item in value)
        else:
            values.append(str(value))
    return tuple(values)


def _tokens(value: str) -> tuple[str, ...]:
    return tuple(token.lower() for token in _TOKEN_RE.findall(value))


def _string_field(item: Mapping[str, Any], name: str) -> str:
    value = item.get(name)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{name} must be a non-empty string")
    return value
