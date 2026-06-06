"""Graph turbo request construction from semantic search packets."""

from __future__ import annotations

import hashlib
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .artifact_targets import PacketTargets, packet_targets

_REQUEST_SCHEMA_ID = "agent.semantic-protocols.semantic-graph-turbo-request"


def search_artifact_dir(root: Path) -> Path:
    if root.name == "search":
        return root
    candidate = root / "search"
    return candidate if candidate.is_dir() else root


def search_packet_to_graph_turbo_request(
    packet: Mapping[str, Any], *, budget: int = 10
) -> dict[str, object] | None:
    targets = packet_targets(packet)
    nodes: dict[tuple[str, str], dict[str, object]] = {}
    edges: dict[tuple[str, str, str], dict[str, object]] = {}
    query_id, seed_ids, owner_ids, test_ids, dependency_ids, item_ids = _request_node_ids(
        nodes,
        targets,
    )
    _request_edges(edges, query_id, owner_ids, test_ids, dependency_ids, item_ids)
    if not nodes or not edges or not seed_ids:
        return None
    return _request_packet(targets.profile, seed_ids, nodes, edges, budget)


def _request_node_ids(
    nodes: dict[tuple[str, str], dict[str, object]],
    targets: PacketTargets,
) -> tuple[str, list[str], list[str], list[str], list[str], list[str]]:
    query_id = _query_node_id(nodes, targets.query)
    seed_ids = [query_id] if query_id else []
    owner_ids = _node_ids(nodes, "owner", "path", targets.owners, action="owner")
    test_ids = _node_ids(nodes, "test", "path", targets.tests, action="tests")
    dependency_ids = _node_ids(
        nodes, "dependency", "pkg", targets.dependencies, action="deps"
    )
    item_ids = _node_ids(nodes, "item", "symbol", targets.items, action="code")
    seed_ids = seed_ids or owner_ids[:2]
    return query_id, seed_ids, owner_ids, test_ids, dependency_ids, item_ids


def _request_edges(
    edges: dict[tuple[str, str, str], dict[str, object]],
    query_id: str,
    owner_ids: list[str],
    test_ids: list[str],
    dependency_ids: list[str],
    item_ids: list[str],
) -> None:
    _connect_query_edges(edges, query_id, owner_ids, item_ids)
    _connect_owner_edges(edges, owner_ids, test_ids, dependency_ids, item_ids)


def _query_node_id(
    nodes: dict[tuple[str, str], dict[str, object]], query: str
) -> str:
    if not query:
        return ""
    return _add_node(nodes, "query", "term", query, action="fzf")


def _node_ids(
    nodes: dict[tuple[str, str], dict[str, object]],
    kind: str,
    role: str,
    values: tuple[str, ...],
    *,
    action: str,
) -> list[str]:
    return [_add_node(nodes, kind, role, value, action=action) for value in values]


def _connect_query_edges(
    edges: dict[tuple[str, str, str], dict[str, object]],
    query_id: str,
    owner_ids: list[str],
    item_ids: list[str],
) -> None:
    if not query_id:
        return
    for node_id in [*owner_ids, *item_ids]:
        _add_edge(edges, query_id, node_id, "matches")


def _connect_owner_edges(
    edges: dict[tuple[str, str, str], dict[str, object]],
    owner_ids: list[str],
    test_ids: list[str],
    dependency_ids: list[str],
    item_ids: list[str],
) -> None:
    for index, test_id in enumerate(test_ids):
        if owner_ids:
            _add_edge(edges, owner_ids[index % len(owner_ids)], test_id, "covers")
    for dependency_id in dependency_ids:
        for owner_id in owner_ids[:4]:
            _add_edge(edges, owner_id, dependency_id, "uses")
    for item_id in item_ids:
        for owner_id in owner_ids[:2]:
            _add_edge(edges, owner_id, item_id, "contains")


def _request_packet(
    profile: str,
    seed_ids: list[str],
    nodes: dict[tuple[str, str], dict[str, object]],
    edges: dict[tuple[str, str, str], dict[str, object]],
    budget: int,
) -> dict[str, object]:
    return {
        "schemaId": _REQUEST_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": profile,
        "algorithm": "typed-ppr-diverse",
        "seedIds": seed_ids,
        "budget": budget,
        "kindBudgets": {"owner": 4, "dependency": 2, "test": 3, "item": 3, "hot": 2},
        "windowMerge": {"enabled": True, "maxGapLines": 8},
        "pathBudget": 5,
        "pathMaxHops": 4,
        "cache": {"enabled": True},
        "graph": {
            "nodes": list(nodes.values()),
            "edges": list(edges.values()),
        },
    }


def _add_node(
    nodes: dict[tuple[str, str], dict[str, object]],
    kind: str,
    role: str,
    value: str,
    *,
    action: str,
) -> str:
    key = (kind, value)
    node_id = f"{kind}:{_digest(value)}"
    nodes.setdefault(
        key,
        {
            "id": node_id,
            "kind": kind,
            "role": role,
            "value": value,
            "action": action,
        },
    )
    return node_id


def _add_edge(
    edges: dict[tuple[str, str, str], dict[str, object]],
    source: str,
    target: str,
    relation: str,
) -> None:
    if source == target:
        return
    edges.setdefault(
        (source, target, relation),
        {"source": source, "target": target, "relation": relation},
    )


def _digest(value: str) -> str:
    return hashlib.sha1(value.encode("utf-8")).hexdigest()[:12]
