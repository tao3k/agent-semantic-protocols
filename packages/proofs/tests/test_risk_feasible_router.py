from __future__ import annotations

import itertools
import unittest
from dataclasses import FrozenInstanceError

from asp_proofs.risk_feasible_router import (
    CompletionKey,
    CostVector,
    RiskRouteCandidate,
    admitted,
    completion_equivalent,
    cost_no_worse,
    dominates,
    pareto_frontier,
)


COMPLETION = CompletionKey(7, "sha256:coverage", "committed", 0)


def route(
    route_id: str,
    cost: CostVector,
    *,
    completion_key: CompletionKey = COMPLETION,
    initial_potential: int = 3,
    final_potential: int = 0,
    route_feasible: bool = True,
    risk_feasible: bool = True,
) -> RiskRouteCandidate:
    return RiskRouteCandidate(
        route_id=route_id,
        initial_potential=initial_potential,
        final_potential=final_potential,
        completion_key=completion_key,
        route_feasible=route_feasible,
        risk_feasible=risk_feasible,
        cost=cost,
    )


class RiskFeasibleRouterTests(unittest.TestCase):
    def test_value_objects_are_immutable(self) -> None:
        candidate = route("safe", CostVector(2, 2, 20, 30))

        with self.assertRaises(FrozenInstanceError):
            candidate.route_id = "changed"  # type: ignore[misc]

    def test_unsafe_cheaper_one_hop_route_is_excluded(self) -> None:
        unsafe = route(
            "unsafe-one-hop",
            CostVector(1, 1, 1, 1),
            risk_feasible=False,
        )

        self.assertFalse(admitted(unsafe))
        self.assertEqual(pareto_frontier([unsafe]), ())

    def test_safe_two_hop_route_is_retained(self) -> None:
        safe = route("safe-two-hop", CostVector(2, 2, 20, 30))
        unsafe = route(
            "unsafe-one-hop",
            CostVector(1, 1, 1, 1),
            risk_feasible=False,
        )

        self.assertEqual(pareto_frontier([unsafe, safe]), (safe,))

    def test_cross_completion_candidates_are_incomparable(self) -> None:
        other_completion = CompletionKey(7, "sha256:other", "committed", 0)
        cheap = route("cheap", CostVector(1, 1, 5, 10))
        expensive_other = route(
            "expensive-other",
            CostVector(3, 3, 50, 100),
            completion_key=other_completion,
        )

        self.assertFalse(completion_equivalent(cheap, expensive_other))
        self.assertFalse(dominates(cheap, expensive_other))
        self.assertEqual(
            pareto_frontier([expensive_other, cheap]),
            (cheap, expensive_other),
        )

    def test_dominated_admitted_candidate_is_pruned(self) -> None:
        better = route("better", CostVector(1, 1, 10, 20))
        worse = route("worse", CostVector(2, 1, 10, 30))

        self.assertTrue(cost_no_worse(better.cost, worse.cost))
        self.assertTrue(dominates(better, worse))
        self.assertEqual(pareto_frontier([worse, better]), (better,))

    def test_equal_cost_distinct_routes_are_both_retained(self) -> None:
        first = route("a-route", CostVector(2, 2, 10, 20))
        second = route("b-route", CostVector(2, 2, 10, 20))

        self.assertFalse(dominates(first, second))
        self.assertFalse(dominates(second, first))
        self.assertEqual(pareto_frontier([second, first]), (first, second))

    def test_frontier_is_invariant_under_input_permutation(self) -> None:
        candidates = (
            route("a", CostVector(1, 2, 20, 20)),
            route("b", CostVector(2, 1, 10, 20)),
            route("c", CostVector(3, 3, 30, 30)),
            route(
                "unsafe",
                CostVector(1, 1, 1, 1),
                risk_feasible=False,
            ),
        )
        expected = pareto_frontier(candidates)

        for permutation in itertools.permutations(candidates):
            self.assertEqual(pareto_frontier(permutation), expected)

    def test_every_omitted_admitted_candidate_has_a_retained_dominator(self) -> None:
        candidates = (
            route("fast", CostVector(1, 2, 20, 20)),
            route("lean", CostVector(2, 1, 10, 10)),
            route("middle", CostVector(2, 2, 20, 20)),
            route("slow", CostVector(3, 4, 40, 40)),
        )
        frontier = pareto_frontier(candidates)

        for candidate in candidates:
            if admitted(candidate) and candidate not in frontier:
                self.assertTrue(
                    any(dominates(retained, candidate) for retained in frontier),
                    candidate.route_id,
                )

    def test_cache_cost_improvement_does_not_create_admission(self) -> None:
        uncached = route(
            "uncached",
            CostVector(3, 3, 100, 100),
            route_feasible=False,
        )
        cached = route(
            "cached",
            CostVector(1, 1, 5, 5),
            route_feasible=False,
        )

        self.assertTrue(cost_no_worse(cached.cost, uncached.cost))
        self.assertFalse(admitted(cached))
        self.assertEqual(pareto_frontier([uncached, cached]), ())

    def test_empty_admitted_set_has_empty_frontier(self) -> None:
        over_budget = route(
            "over-budget",
            CostVector(1, 1, 1, 1),
            risk_feasible=False,
        )
        unavailable = route(
            "unavailable",
            CostVector(2, 2, 2, 2),
            route_feasible=False,
        )

        self.assertEqual(pareto_frontier([over_budget, unavailable]), ())


if __name__ == "__main__":
    unittest.main()
