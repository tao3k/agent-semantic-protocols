-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Mathlib

namespace ASPProof.AXLE.SearchRouteExecutableRiskAdmittedParetoFrontier

structure RouteCost where
  graphHops : Nat
  interactionRounds : Nat
  searchTokens : Nat
  modelTokens : Nat
  deriving DecidableEq

structure Candidate where
  routeId : Nat
  completionKey : Nat
  routeFeasible : Bool
  riskFeasible : Bool
  cost : RouteCost
  deriving DecidableEq

def admitted (candidate : Candidate) : Prop :=
  candidate.routeFeasible = true ∧ candidate.riskFeasible = true

def completionEquivalent (left right : Candidate) : Prop :=
  left.completionKey = right.completionKey

def costNoWorse (left right : RouteCost) : Prop :=
  left.graphHops ≤ right.graphHops ∧
  left.interactionRounds ≤ right.interactionRounds ∧
  left.searchTokens ≤ right.searchTokens ∧
  left.modelTokens ≤ right.modelTokens

def strictCostImprovement (left right : RouteCost) : Prop :=
  left.graphHops < right.graphHops ∨
  left.interactionRounds < right.interactionRounds ∨
  left.searchTokens < right.searchTokens ∨
  left.modelTokens < right.modelTokens

def dominates (left right : Candidate) : Prop :=
  admitted left ∧
  admitted right ∧
  completionEquivalent left right ∧
  costNoWorse left.cost right.cost ∧
  strictCostImprovement left.cost right.cost

def unsafeShort : Candidate :=
  { routeId := 1
    completionKey := 100
    routeFeasible := true
    riskFeasible := false
    cost := { graphHops := 1, interactionRounds := 1, searchTokens := 60, modelTokens := 10 } }

def safeLong : Candidate :=
  { routeId := 3
    completionKey := 100
    routeFeasible := true
    riskFeasible := true
    cost := { graphHops := 2, interactionRounds := 2, searchTokens := 80, modelTokens := 20 } }

theorem unsafe_shorter_route_does_not_dominate_admitted_route :
    ¬ dominates unsafeShort safeLong := by
  sorry

end ASPProof.AXLE.SearchRouteExecutableRiskAdmittedParetoFrontier
