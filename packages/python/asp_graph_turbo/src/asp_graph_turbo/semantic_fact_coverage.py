"""Semantic fact coverage repair for graph-turbo ranking."""

from __future__ import annotations

from collections import Counter
from collections.abc import Mapping

from .model import Node, TypedGraph
from .query_token_balance import query_tokens_for_node

SEMANTIC_FACT_KINDS = frozenset({"type"})
SEMANTIC_FACT_RELATIONS = {"type": "has_type"}
PROTECTED_FACT_KINDS = frozenset({"collection", "type"})


def repair_semantic_fact_coverage(
    graph: TypedGraph,
    ranked: list[Node],
    scores: Mapping[str, float],
    query_tokens: tuple[str, ...],
    coverage_limit: int,
) -> list[Node]:
    if not ranked or coverage_limit <= 0:
        return ranked
    repaired = list(ranked)
    for kind in SEMANTIC_FACT_KINDS:
        if _has_kind(repaired[:coverage_limit], kind):
            continue
        candidate, protected_ids = _best_semantic_fact_candidate(
            graph,
            repaired[:coverage_limit],
            scores,
            kind,
        )
        if candidate is None:
            continue
        replace_index = _semantic_fact_replacement_index(
            repaired,
            query_tokens,
            protected_ids,
            coverage_limit,
        )
        if replace_index is not None:
            repaired[replace_index] = candidate
    return repaired


def _has_kind(nodes: list[Node], kind: str) -> bool:
    return any(node.kind == kind for node in nodes)


def _best_semantic_fact_candidate(
    graph: TypedGraph,
    selected: list[Node],
    scores: Mapping[str, float],
    kind: str,
) -> tuple[Node | None, frozenset[str]]:
    relation = SEMANTIC_FACT_RELATIONS.get(kind)
    selected_ids = {node.id for node in selected}
    linked: list[tuple[Node, frozenset[str]]] = []
    if relation is not None:
        for edge in graph.edges:
            if edge.relation != relation:
                continue
            candidate_id, protected_ids = _edge_candidate(edge.source, edge.target, selected_ids)
            if candidate_id is None:
                continue
            candidate = graph.nodes.get(candidate_id)
            if candidate is not None and candidate.kind == kind:
                linked.append((candidate, protected_ids))
    if linked:
        return max(linked, key=lambda item: (scores.get(item[0].id, 0.0), item[0].id))
    candidates = [
        node
        for node in graph.nodes.values()
        if node.kind == kind and node.id not in selected_ids
    ]
    if not candidates:
        return None, frozenset()
    return max(candidates, key=lambda node: (scores.get(node.id, 0.0), node.id)), frozenset()


def _edge_candidate(
    source_id: str,
    target_id: str,
    selected_ids: set[str],
) -> tuple[str | None, frozenset[str]]:
    if source_id in selected_ids and target_id not in selected_ids:
        return target_id, frozenset({source_id})
    if target_id in selected_ids and source_id not in selected_ids:
        return source_id, frozenset({target_id})
    return None, frozenset()


def _semantic_fact_replacement_index(
    ranked: list[Node],
    query_tokens: tuple[str, ...],
    protected_ids: frozenset[str],
    coverage_limit: int,
) -> int | None:
    upper = min(len(ranked), coverage_limit)
    if upper == 0:
        return None
    token_counts = _query_token_counts(ranked[:upper], query_tokens)
    kind_counts = Counter(node.kind for node in ranked[:upper])
    for index in range(upper - 1, 0, -1):
        node = ranked[index]
        if node.id in protected_ids:
            continue
        if node.kind in PROTECTED_FACT_KINDS and kind_counts[node.kind] <= 1:
            continue
        node_tokens = query_tokens_for_node(
            node,
            query_tokens,
            include_query_node=False,
        )
        if not node_tokens or all(token_counts[token] > 1 for token in node_tokens):
            return index
    return None


def _query_token_counts(nodes: list[Node], query_tokens: tuple[str, ...]) -> Counter[str]:
    counts: Counter[str] = Counter()
    for node in nodes:
        counts.update(query_tokens_for_node(node, query_tokens, include_query_node=False))
    return counts
