"""Algorithm evidence helpers for graph turbo responses."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import (
    AlgorithmMetrics,
    AlgorithmTraceStep,
    GraphProfile,
    Node,
    RankExplanation,
    ReadLoopGuard,
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
    receipt_reasons: Mapping[str, tuple[str, ...]] | None = None,
) -> tuple[RankExplanation, ...]:
    receipt_reasons = receipt_reasons or {}
    explanations: list[RankExplanation] = []
    for node in ranked:
        reasons = [
            "typed-ppr",
            f"kind:{node.kind}",
            f"depth:{best_depth.get(node.id, 99)}",
        ]
        bonus = node_kind_bonus(profile.name, node.kind)
        if bonus != 0.0:
            reasons.append(f"kind-bonus:{bonus:+.2f}")
        if node.id in seed_ids:
            reasons.append("seed")
        if node.kind in kind_budgets:
            reasons.append(f"kind-budget:{kind_budgets[node.kind]}")
        reasons.extend(receipt_reasons.get(node.id, ()))
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
    read_loop_guard: ReadLoopGuard | None = None,
    read_memory_suppressed_count: int = 0,
    receipt_boost_count: int = 0,
    receipt_penalty_count: int = 0,
) -> tuple[AlgorithmTraceStep, ...]:
    steps = [
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
    ]
    if read_loop_guard is not None:
        steps.append(
            AlgorithmTraceStep(
                "read-loop-guard",
                "python",
                {
                    "directCodeActionCount": read_loop_guard.direct_code_action_count,
                    "duplicateSelectorCount": read_loop_guard.duplicate_selector_count,
                    "adjacentRangeWindowCount": read_loop_guard.adjacent_range_window_count,
                    "sameOwnerScanCount": read_loop_guard.same_owner_scan_count,
                },
            )
        )
    if read_memory_suppressed_count:
        steps.append(
            AlgorithmTraceStep(
                "read-memory-suppression",
                "python",
                {"suppressedSelectorCount": read_memory_suppressed_count},
            )
        )
    if receipt_boost_count or receipt_penalty_count:
        steps.append(
            AlgorithmTraceStep(
                "receipt-feedback",
                "python",
                {
                    "boostCount": receipt_boost_count,
                    "penaltyCount": receipt_penalty_count,
                },
            )
        )
    return tuple(steps)


def algorithm_metrics(
    graph: TypedGraph,
    *,
    selected_edge_count: int,
    reachable_node_count: int,
    ranked_node_count: int,
    path_count: int,
    merged_window_count: int,
    cache_status: str,
    read_loop_guard: ReadLoopGuard | None = None,
    read_memory_suppressed_count: int = 0,
    receipt_boost_count: int = 0,
    receipt_penalty_count: int = 0,
) -> AlgorithmMetrics:
    guard = read_loop_guard or ReadLoopGuard(0, 0, 0, 0, ())
    return AlgorithmMetrics(
        node_count=len(graph.nodes),
        edge_count=len(graph.edges),
        selected_edge_count=selected_edge_count,
        reachable_node_count=reachable_node_count,
        ranked_node_count=ranked_node_count,
        path_count=path_count,
        merged_window_count=merged_window_count,
        cache_status=cache_status,
        read_loop_direct_code_action_count=guard.direct_code_action_count,
        read_loop_duplicate_selector_count=guard.duplicate_selector_count,
        read_loop_adjacent_range_window_count=guard.adjacent_range_window_count,
        read_loop_same_owner_scan_count=guard.same_owner_scan_count,
        read_memory_suppressed_count=read_memory_suppressed_count,
        receipt_boost_count=receipt_boost_count,
        receipt_penalty_count=receipt_penalty_count,
    )
