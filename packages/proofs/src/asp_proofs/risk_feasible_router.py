# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Reference model for executable, risk-admitted Pareto routing."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class CompletionKey:
    """The observable completion obligation a route promises to satisfy."""

    generation: int
    coverage_digest: str
    completion_mode: str
    completion_gap: int


@dataclass(frozen=True, slots=True)
class CostVector:
    """Independently ordered route costs; lower is better in every dimension."""

    graph_hops: int
    interaction_rounds: int
    search_payload_tokens: int
    uncached_model_work_tokens: int
    verification_work: int = 0
    search_executions: int = 0
    model_prefix_recomputations: int = 0


@dataclass(frozen=True, slots=True)
class RiskRouteCandidate:
    """A route together with its completion, cost, and safety evidence."""

    route_id: str
    initial_potential: int
    final_potential: int
    completion_key: CompletionKey
    route_feasible: bool
    risk_feasible: bool
    cost: CostVector


def completion_equivalent(
    left: RiskRouteCandidate, right: RiskRouteCandidate
) -> bool:
    """Return whether two routes discharge exactly the same obligation."""

    return left.completion_key == right.completion_key


def admitted(candidate: RiskRouteCandidate) -> bool:
    """Return whether a route is executable and within its risk budget."""

    return candidate.route_feasible and candidate.risk_feasible


def cost_no_worse(left: CostVector, right: CostVector) -> bool:
    """Return whether ``left`` is componentwise no more costly than ``right``."""

    return (
        left.graph_hops <= right.graph_hops
        and left.interaction_rounds <= right.interaction_rounds
        and left.search_payload_tokens <= right.search_payload_tokens
        and left.uncached_model_work_tokens <= right.uncached_model_work_tokens
        and left.verification_work <= right.verification_work
        and left.search_executions <= right.search_executions
        and left.model_prefix_recomputations
        <= right.model_prefix_recomputations
    )


def dominates(left: RiskRouteCandidate, right: RiskRouteCandidate) -> bool:
    """Return whether ``left`` strictly Pareto-dominates ``right``.

    Admission and completion equivalence are part of dominance, so an unsafe
    route can never remove a safe route and routes for different completion
    obligations remain incomparable.
    """

    return (
        admitted(left)
        and admitted(right)
        and completion_equivalent(left, right)
        and cost_no_worse(left.cost, right.cost)
        and left.cost != right.cost
    )


def _canonical_key(candidate: RiskRouteCandidate) -> tuple[object, ...]:
    completion = candidate.completion_key
    cost = candidate.cost
    return (
        candidate.route_id,
        candidate.initial_potential,
        candidate.final_potential,
        completion.generation,
        completion.coverage_digest,
        completion.completion_mode,
        completion.completion_gap,
        candidate.route_feasible,
        candidate.risk_feasible,
        cost.graph_hops,
        cost.interaction_rounds,
        cost.search_payload_tokens,
        cost.uncached_model_work_tokens,
        cost.verification_work,
        cost.search_executions,
        cost.model_prefix_recomputations,
    )


def pareto_frontier(
    candidates: Iterable[RiskRouteCandidate],
) -> tuple[RiskRouteCandidate, ...]:
    """Return the admitted, non-dominated candidates in canonical order."""

    admitted_candidates = tuple(candidate for candidate in candidates if admitted(candidate))
    frontier = tuple(
        candidate
        for candidate in admitted_candidates
        if not any(
            other is not candidate and dominates(other, candidate)
            for other in admitted_candidates
        )
    )
    return tuple(sorted(frontier, key=_canonical_key))
