"""Query-aware node weighting for graph-turbo ranking."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import Node, TypedGraph
from .query_package_cohesion import (
    query_package_cohesion_adjustment,
    query_package_cohesion_tokens,
)
from .query_token_priority import (
    QUERY_TOKEN_UNCOVERED_BONUS,
    query_token_balance_weights,
)
from .query_tokens import query_tokens_from_text

_MAX_QUERY_MATCH_BONUS = 0.75
_QUERY_SEED_WEIGHT = 1.20
_MATCHED_SEED_FLOOR = 0.35
_UNMATCHED_SEED_FLOOR = 0.20
_QUERY_WEIGHT_NODE_KINDS = {"collection", "field", "hot", "item", "owner", "type"}
_QUERY_MATCH_COMPOUND_RATIO = 0.25

__all__ = [
    "query_node_match_bonus",
    "query_package_cohesion_adjustment",
    "query_package_cohesion_tokens",
    "query_seed_personalization_weights",
    "query_token_weights",
]


def query_token_weights(
    graph: TypedGraph,
    *,
    profile_name: str,
    seed_ids: Iterable[str],
    query_clauses: Iterable[str] = (),
) -> Mapping[str, float]:
    if profile_name != "owner-query":
        return {}
    query_tokens = _query_tokens(graph, seed_ids)
    if not query_tokens:
        return {}
    clause_weight = query_token_balance_weights(
        query_tokens,
        tuple(query_clauses),
    )
    candidate_texts = [
        _node_text(node)
        for node in graph.nodes.values()
        if node.kind in _QUERY_WEIGHT_NODE_KINDS
    ]
    token_weights: dict[str, float] = {}
    for token in query_tokens:
        frequency = sum(1 for text in candidate_texts if token in text)
        if frequency > 0:
            multiplier = clause_weight.get(token, QUERY_TOKEN_UNCOVERED_BONUS)
            multiplier /= QUERY_TOKEN_UNCOVERED_BONUS
            token_weights[token] = multiplier / frequency
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
    exact_node_text = _node_exact_semantic_text(node)
    exact_tokens = {token for token in token_weights if token in exact_node_text}
    broad_tokens = {
        token
        for token in token_weights
        if token in node_text and token not in exact_tokens
    }
    matched_tokens = exact_tokens | broad_tokens
    if not matched_tokens:
        return 0.0
    matched_weight = sum(token_weights[token] for token in exact_tokens)
    matched_weight += sum(
        token_weights[token] * _QUERY_MATCH_COMPOUND_RATIO for token in broad_tokens
    )
    total_weight = sum(token_weights.values())
    coverage = matched_weight / total_weight if total_weight > 0 else 0.0
    return min(_MAX_QUERY_MATCH_BONUS, 0.15 * matched_weight + 0.55 * coverage)


def query_seed_personalization_weights(
    graph: TypedGraph,
    *,
    profile_name: str,
    seed_ids: Iterable[str],
) -> Mapping[str, float]:
    if profile_name != "owner-query":
        return {}
    seed_id_tuple = tuple(seed_ids)
    query_tokens = _query_tokens(graph, seed_id_tuple)
    if len(query_tokens) < 2:
        return {}
    token_weights = query_token_weights(
        graph,
        profile_name=profile_name,
        seed_ids=seed_id_tuple,
    )
    if not token_weights:
        return {}
    weights: dict[str, float] = {}
    for seed_id in seed_id_tuple:
        node = graph.nodes.get(seed_id)
        if node is None:
            continue
        if node.kind == "query":
            weights[seed_id] = _QUERY_SEED_WEIGHT
            continue
        if node.kind not in _QUERY_WEIGHT_NODE_KINDS:
            weights[seed_id] = _UNMATCHED_SEED_FLOOR
            continue
        bonus = query_node_match_bonus(
            profile_name=profile_name,
            token_weights=token_weights,
            node=node,
        )
        weights[seed_id] = (
            _MATCHED_SEED_FLOOR + bonus if bonus > 0.0 else _UNMATCHED_SEED_FLOOR
        )
    return weights


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
        _node_semantic_text(node),
        str(node.fields.get("path") or ""),
        str(node.fields.get("ownerPath") or ""),
    ]
    return " ".join(parts).lower()


def _node_semantic_text(node: Node) -> str:
    parts = [
        node.value,
        node.role,
        node.kind,
        str(node.fields.get("symbol") or ""),
        str(node.fields.get("name") or ""),
        str(node.fields.get("matchText") or ""),
        _node_fields_text(node),
        _semantic_alias_text(node),
    ]
    return " ".join(parts).lower()


def _node_exact_semantic_text(node: Node) -> str:
    parts = [
        node.value,
        node.role,
        node.kind,
        str(node.fields.get("symbol") or ""),
        str(node.fields.get("name") or ""),
        _node_fields_text(node),
        _semantic_alias_text(node),
    ]
    return " ".join(parts).lower()


def _node_path_text(node: Node) -> str:
    parts = [
        str(node.fields.get("path") or ""),
        str(node.fields.get("ownerPath") or ""),
    ]
    if node.kind in {"owner", "test"}:
        parts.append(node.value)
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
    return query_tokens_from_text(value)
