"""Rank explanation projection for graph turbo responses."""

from __future__ import annotations

from collections.abc import Iterable, Mapping

from .model import GraphProfile, Node, RankExplanation


def rank_explanations(
    ranked: Iterable[Node],
    profile: GraphProfile,
    scores: Mapping[str, float],
    best_depth: Mapping[str, int],
    seed_ids: tuple[str, ...],
    kind_budgets: Mapping[str, int],
    receipt_reasons: Mapping[str, tuple[str, ...]] | None = None,
    relation_reasons: Mapping[str, tuple[str, ...]] | None = None,
) -> tuple[RankExplanation, ...]:
    receipt_reasons = receipt_reasons or {}
    relation_reasons = relation_reasons or {}
    explanations: list[RankExplanation] = []
    for node in ranked:
        reasons = [
            "typed-ppr",
            f"kind:{node.kind}",
            f"depth:{best_depth.get(node.id, 99)}",
        ]
        bonus = profile.kind_bonus.get(node.kind, 0.0)
        if bonus != 0.0:
            reasons.append(f"kind-bonus:{bonus:+.2f}")
        if node.id in seed_ids:
            reasons.append("seed")
        if node.kind in kind_budgets:
            reasons.append(f"kind-budget:{kind_budgets[node.kind]}")
        reasons.extend(relation_reasons.get(node.id, ()))
        reasons.extend(receipt_reasons.get(node.id, ()))
        explanations.append(
            RankExplanation(
                node_id=node.id,
                score=scores.get(node.id, 0.0),
                depth=best_depth.get(node.id, 99),
                reasons=tuple(reasons),
            )
        )
    return tuple(explanations)
