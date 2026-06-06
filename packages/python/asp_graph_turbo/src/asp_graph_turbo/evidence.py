"""Algorithm evidence helpers for graph turbo responses."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import (
    AlgorithmMetrics,
    AlgorithmTraceStep,
    GraphProfile,
    Node,
    RankExplanation,
    TypedGraph,
)
from .policy import node_kind_bonus


def rank_explanations(
    ranked: Iterable[Node],
    profile: GraphProfile,
    scores: Mapping[str, float],
    best_depth: Mapping[str, int],
    seed_ids: tuple[str, ...],
    kind_budgets: Mapping[str, int],
) -> tuple[RankExplanation, ...]:
    explanations: list[RankExplanation] = []
    for node in ranked:
        reasons = ["typed-ppr", f"kind:{node.kind}", f"depth:{best_depth.get(node.id, 99)}"]
        bonus = node_kind_bonus(profile.name, node.kind)
        if bonus != 0.0:
            reasons.append(f"kind-bonus:{bonus:+.2f}")
        if node.id in seed_ids:
            reasons.append("seed")
        if node.kind in kind_budgets:
            reasons.append(f"kind-budget:{kind_budgets[node.kind]}")
        explanations.append(
            RankExplanation(
                node_id=node.id,
                score=scores[node.id],
                depth=best_depth.get(node.id, 99),
                reasons=tuple(reasons),
            )
        )
    return tuple(explanations)


def algorithm_trace(
    graph: TypedGraph,
    profile: GraphProfile,
    cache_status: str,
    *,
    reachable_count: int,
    ranked_count: int,
    path_count: int,
    merged_window_count: int,
) -> tuple[AlgorithmTraceStep, ...]:
    return (
        AlgorithmTraceStep(
            "packet-fingerprint",
            "sha256",
            {"nodeCount": len(graph.nodes), "edgeCount": len(graph.edges)},
        ),
        AlgorithmTraceStep("graph-cache", "memory", {"status": cache_status}),
        AlgorithmTraceStep(
            "profile-policy",
            "python",
            {
                "profile": profile.name,
                "allowedRelationCount": len(profile.allowed_relations),
                "allowedTransitionCount": len(profile.allowed_transitions),
                "kindBonusCount": len(profile.kind_bonus),
            },
        ),
        AlgorithmTraceStep(
            "typed-ppr",
            "scipy-csr",
            {"profile": profile.name, "reachableNodeCount": reachable_count},
        ),
        AlgorithmTraceStep("diverse-rank", "python", {"rankedNodeCount": ranked_count}),
        AlgorithmTraceStep("typed-paths", "python", {"pathCount": path_count}),
        AlgorithmTraceStep(
            "window-merge",
            "python",
            {"mergedWindowCount": merged_window_count},
        ),
    )


def algorithm_metrics(
    graph: TypedGraph,
    *,
    selected_edge_count: int,
    reachable_node_count: int,
    ranked_node_count: int,
    path_count: int,
    merged_window_count: int,
    cache_status: str,
) -> AlgorithmMetrics:
    return AlgorithmMetrics(
        node_count=len(graph.nodes),
        edge_count=len(graph.edges),
        selected_edge_count=selected_edge_count,
        reachable_node_count=reachable_node_count,
        ranked_node_count=ranked_node_count,
        path_count=path_count,
        merged_window_count=merged_window_count,
        cache_status=cache_status,
    )
