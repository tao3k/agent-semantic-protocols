"""Query-token coverage helpers for graph-turbo ranking."""

from __future__ import annotations

import re
from collections import Counter
from collections.abc import Mapping
from typing import Any

from .model import Node, TypedGraph
from .query_token_priority import (
    QUERY_TOKEN_UNCOVERED_BONUS,
    exact_query_tokens_for_node,
    query_token_balance_weights,
)
from .selector import graph_turbo_selector_for_node as selector_for_node

QUERY_TOKEN_COMPOUND_MATCH_RATIO = 0.25
_TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")


def query_tokens_for_seed_nodes(
    graph: TypedGraph,
    seed_ids: tuple[str, ...],
) -> tuple[str, ...]:
    seen: set[str] = set()
    tokens: list[str] = []
    for node_id in seed_ids:
        node = graph.nodes.get(node_id)
        if node is None or node.kind != "query":
            continue
        for token in _TOKEN_RE.findall(node.value.lower()):
            if token in seen:
                continue
            seen.add(token)
            tokens.append(token)
    return tuple(tokens)


def query_token_balance_bonus(
    node: Node,
    query_tokens: tuple[str, ...],
    covered_query_tokens: set[str],
    *,
    token_weights: Mapping[str, float] | None = None,
) -> float:
    if not query_tokens or node.kind == "query":
        return 0.0
    weights = token_weights or query_token_balance_weights(query_tokens)
    exact_matched = exact_query_tokens_for_node(node, query_tokens)
    broad_matched = query_tokens_for_node(node, query_tokens)
    bonus = sum(
        weights.get(token, QUERY_TOKEN_UNCOVERED_BONUS)
        for token in exact_matched
        if token not in covered_query_tokens
    )
    bonus += sum(
        weights.get(token, QUERY_TOKEN_UNCOVERED_BONUS)
        * QUERY_TOKEN_COMPOUND_MATCH_RATIO
        for token in broad_matched
        if token not in exact_matched and token not in covered_query_tokens
    )
    return bonus


def query_tokens_for_node(
    node: Node,
    query_tokens: tuple[str, ...],
    *,
    include_query_node: bool = True,
) -> tuple[str, ...]:
    if not include_query_node and node.kind == "query":
        return ()
    text = _node_text(node).lower()
    return tuple(token for token in query_tokens if token in text)


def repair_query_token_coverage(
    ranked: list[Node],
    remaining: list[Node],
    scores: Mapping[str, float],
    query_tokens: tuple[str, ...],
    coverage_limit: int,
    *,
    token_weights: Mapping[str, float] | None = None,
) -> list[Node]:
    if not query_tokens or not ranked or coverage_limit <= 0:
        return ranked
    repaired = list(ranked)
    for token in _missing_query_tokens(repaired[:coverage_limit], query_tokens):
        selected_ids = {node.id for node in repaired}
        selected_selectors = {
            selector_for_node(node)
            for node in repaired
            if selector_for_node(node) is not None
        }
        candidate = _best_remaining_token_candidate(
            remaining,
            scores,
            token,
            selected_ids,
            selected_selectors,
            token_weights=token_weights,
        )
        if candidate is None:
            continue
        replace_index = _query_token_replacement_index(
            repaired,
            query_tokens,
            coverage_limit,
        )
        if replace_index is not None:
            repaired[replace_index] = candidate
    return repaired


def _missing_query_tokens(
    nodes: list[Node], query_tokens: tuple[str, ...]
) -> tuple[str, ...]:
    covered = set()
    for node in nodes:
        covered.update(
            exact_query_tokens_for_node(node, query_tokens, include_query_node=False)
        )
    return tuple(token for token in query_tokens if token not in covered)


def _best_remaining_token_candidate(
    remaining: list[Node],
    scores: Mapping[str, float],
    token: str,
    selected_ids: set[str],
    selected_selectors: set[str],
    *,
    token_weights: Mapping[str, float] | None = None,
) -> Node | None:
    candidates = [
        node
        for node in remaining
        if node.id not in selected_ids
        and node.kind != "query"
        and token in exact_query_tokens_for_node(node, (token,))
        and (
            selector_for_node(node) is None
            or selector_for_node(node) not in selected_selectors
        )
    ]
    if not candidates:
        candidates = [
            node
            for node in remaining
            if node.id not in selected_ids
            and node.kind != "query"
            and token in query_tokens_for_node(node, (token,))
            and (
                selector_for_node(node) is None
                or selector_for_node(node) not in selected_selectors
            )
        ]
    if not candidates:
        return None
    weights = token_weights or query_token_balance_weights((token,))
    return max(
        candidates,
        key=lambda node: (
            weights.get(token, QUERY_TOKEN_UNCOVERED_BONUS),
            token in exact_query_tokens_for_node(node, (token,)),
            scores.get(node.id, 0.0),
            _source_preferred_node(node),
            node.kind,
            node.id,
        ),
    )


def _query_token_replacement_index(
    ranked: list[Node],
    query_tokens: tuple[str, ...],
    coverage_limit: int,
) -> int | None:
    upper = min(len(ranked), coverage_limit)
    if upper == 0:
        return None
    coverage_counts: Counter[str] = Counter()
    for node in ranked[:upper]:
        coverage_counts.update(
            exact_query_tokens_for_node(node, query_tokens, include_query_node=False)
        )
    protected_prefix = 1
    for index in range(upper - 1, protected_prefix - 1, -1):
        node = ranked[index]
        if node.kind == "query":
            continue
        node_tokens = exact_query_tokens_for_node(
            node,
            query_tokens,
            include_query_node=False,
        )
        if not node_tokens or all(coverage_counts[token] > 1 for token in node_tokens):
            return index
    return None


def _node_text(node: Node) -> str:
    return " ".join(
        [
            node.value,
            _value_text(node.fields),
        ]
    )


def _value_text(value: Any) -> str:
    if isinstance(value, Mapping):
        return " ".join(_value_text(item) for item in value.values())
    if isinstance(value, (list, tuple)):
        return " ".join(_value_text(item) for item in value)
    return str(value)


def _source_preferred_node(node: Node) -> bool:
    path = str(node.fields.get("path") or node.fields.get("ownerPath") or "")
    return not (
        "/tests/" in path
        or path.endswith("/tests")
        or "/benches/" in path
        or path.endswith("/benches")
        or "/examples/" in path
        or path.endswith("/examples")
        or "stress-test/" in path
    )
