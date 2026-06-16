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
    query_adjustments: Mapping[str, Mapping[str, float]] | None = None,
) -> tuple[RankExplanation, ...]:
    receipt_reasons = receipt_reasons or {}
    relation_reasons = relation_reasons or {}
    query_adjustments = query_adjustments or {}
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
        reasons.extend(_query_adjustment_reasons(query_adjustments.get(node.id, {})))
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


def _query_adjustment_reasons(adjustments: Mapping[str, float]) -> tuple[str, ...]:
    reasons: list[str] = []
    for name in ("seedPrior", "packageCohesion", "queryClauseCoverage"):
        value = adjustments.get(name)
        if not isinstance(value, int | float) or value == 0:
            continue
        reason_name = {
            "seedPrior": "query-seed-prior",
            "packageCohesion": "query-package-cohesion",
            "queryClauseCoverage": "query-clause-coverage",
        }[name]
        reasons.append(f"{reason_name}:{value:+.2f}")
    return tuple(reasons)
