"""Query-aware node weighting for graph-turbo ranking."""

from __future__ import annotations

import re
from collections.abc import Iterable, Mapping

from .model import Node, TypedGraph

_TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")
_MIN_TOKEN_LENGTH = 2
_MAX_QUERY_MATCH_BONUS = 0.75
_QUERY_WEIGHT_NODE_KINDS = {"collection", "field", "hot", "item", "owner", "type"}


def query_token_weights(
    graph: TypedGraph,
    *,
    profile_name: str,
    seed_ids: Iterable[str],
) -> Mapping[str, float]:
    if profile_name != "owner-query":
        return {}
    query_tokens = _query_tokens(graph, seed_ids)
    if not query_tokens:
        return {}
    candidate_texts = [
        _node_text(node)
        for node in graph.nodes.values()
        if node.kind in _QUERY_WEIGHT_NODE_KINDS
    ]
    token_weights: dict[str, float] = {}
    for token in query_tokens:
        frequency = sum(1 for text in candidate_texts if token in text)
        if frequency > 0:
            token_weights[token] = 1.0 / frequency
    return token_weights


def query_node_match_bonus(
    *,
    profile_name: str,
    token_weights: Mapping[str, float],
    node: Node,
) -> float:
    if profile_name != "owner-query" or node.kind not in _QUERY_WEIGHT_NODE_KINDS:
        return 0.0
    node_text = _node_text(node)
    if not node_text or not token_weights:
        return 0.0
    matched_tokens = {token for token in token_weights if token in node_text}
    if not matched_tokens:
        return 0.0
    matched_weight = sum(token_weights[token] for token in matched_tokens)
    total_weight = sum(token_weights.values())
    coverage = matched_weight / total_weight if total_weight > 0 else 0.0
    return min(_MAX_QUERY_MATCH_BONUS, 0.15 * matched_weight + 0.55 * coverage)


def _query_tokens(graph: TypedGraph, seed_ids: Iterable[str]) -> tuple[str, ...]:
    tokens: list[str] = []
    seen: set[str] = set()
    for seed_id in seed_ids:
        node = graph.nodes.get(seed_id)
        if node is None or node.kind != "query":
            continue
        for token in _tokens(str(node.value)):
            if token in seen:
                continue
            seen.add(token)
            tokens.append(token)
    return tuple(tokens)


def _node_text(node: Node) -> str:
    parts = [
        node.value,
        node.role,
        node.kind,
        str(node.fields.get("symbol") or ""),
        str(node.fields.get("name") or ""),
        str(node.fields.get("path") or ""),
        str(node.fields.get("ownerPath") or ""),
        str(node.fields.get("matchText") or ""),
        _node_fields_text(node),
        _semantic_alias_text(node),
    ]
    return " ".join(parts).lower()


def _node_fields_text(node: Node) -> str:
    fields = node.fields.get("fields")
    if not isinstance(fields, Mapping):
        return ""
    return " ".join(str(value) for value in fields.values())


def _semantic_alias_text(node: Node) -> str:
    aliases = {
        "collection": "collection collections list lists map maps set sets",
        "field": "field fields",
        "type": "type types",
    }
    return aliases.get(node.kind, "")


def _tokens(value: str) -> tuple[str, ...]:
    return tuple(
        token.lower()
        for token in _TOKEN_RE.findall(value)
        if len(token) >= _MIN_TOKEN_LENGTH
    )
