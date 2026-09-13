-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Mathlib

namespace ASPProof.AXLE.SearchRouteRiskFeasibleGraphSelectionCost

structure RouteCost where
  graphHops : Nat
  modelPrefixRecomputations : Nat
  deriving DecidableEq

structure Candidate where
  cost : RouteCost
  routeWithinBudget : Bool
  sharedTokens : Nat
  perClaimTokens : Nat
  claimCount : Nat
  batchCount : Nat
  hardFallbackCap : Nat
  tailValid : Bool
  deriving DecidableEq

def hardRoundSafe (candidate : Candidate) : Prop :=
  candidate.batchCount + candidate.hardFallbackCap ≤ candidate.claimCount

def hardTokenSafe (candidate : Candidate) : Prop :=
  (candidate.batchCount + candidate.hardFallbackCap) * candidate.sharedTokens +
      candidate.hardFallbackCap * candidate.perClaimTokens ≤
    candidate.claimCount * candidate.sharedTokens

def riskFeasible (candidate : Candidate) : Prop :=
  hardRoundSafe candidate ∧ hardTokenSafe candidate ∧ candidate.tailValid = true

def admitted (candidate : Candidate) : Prop :=
  candidate.routeWithinBudget = true ∧ riskFeasible candidate

def hopFirstBetter (left right : Candidate) : Prop :=
  left.cost.graphHops < right.cost.graphHops

def noWorse (left right : RouteCost) : Prop :=
  left.graphHops ≤ right.graphHops ∧
  left.modelPrefixRecomputations ≤ right.modelPrefixRecomputations

def shortUnsafe : Candidate :=
  { cost := { graphHops := 1, modelPrefixRecomputations := 1 }
    routeWithinBudget := true
    sharedTokens := 10
    perClaimTokens := 2
    claimCount := 8
    batchCount := 2
    hardFallbackCap := 6
    tailValid := true }

def cachedShortUnsafe : Candidate :=
  { cost := { graphHops := 1, modelPrefixRecomputations := 0 }
    routeWithinBudget := true
    sharedTokens := 10
    perClaimTokens := 2
    claimCount := 8
    batchCount := 2
    hardFallbackCap := 6
    tailValid := true }

def longerSafe : Candidate :=
  { cost := { graphHops := 2, modelPrefixRecomputations := 1 }
    routeWithinBudget := true
    sharedTokens := 10
    perClaimTokens := 2
    claimCount := 8
    batchCount := 2
    hardFallbackCap := 5
    tailValid := true }

theorem cap_six_passes_round_but_fails_token :
    hardRoundSafe shortUnsafe ∧ ¬ hardTokenSafe shortUnsafe := by
  norm_num [hardRoundSafe, hardTokenSafe, shortUnsafe]

theorem cap_five_passes_round_and_token :
    hardRoundSafe longerSafe ∧ hardTokenSafe longerSafe := by
  norm_num [hardRoundSafe, hardTokenSafe, longerSafe]

theorem short_route_is_hop_first_better :
    hopFirstBetter shortUnsafe longerSafe := by
  norm_num [hopFirstBetter, shortUnsafe, longerSafe]

theorem hop_first_preference_does_not_imply_risk_admission :
    hopFirstBetter shortUnsafe longerSafe ∧
    shortUnsafe.routeWithinBudget = true ∧
    ¬ riskFeasible shortUnsafe ∧
    admitted longerSafe := by
  norm_num [hopFirstBetter, riskFeasible, admitted, hardRoundSafe,
    hardTokenSafe, shortUnsafe, longerSafe]

theorem longer_feasible_route_is_admitted_over_shorter_route :
    admitted longerSafe ∧ ¬ admitted shortUnsafe := by
  norm_num [admitted, riskFeasible, hardRoundSafe, hardTokenSafe,
    shortUnsafe, longerSafe]

theorem cache_cost_improvement_does_not_create_admission :
    noWorse cachedShortUnsafe.cost shortUnsafe.cost ∧
    ¬ admitted cachedShortUnsafe := by
  norm_num [noWorse, admitted, riskFeasible, hardRoundSafe, hardTokenSafe,
    cachedShortUnsafe, shortUnsafe]

structure EvaluationIdentity where
  routeDigest : Nat
  riskDigest : Nat
  deriving DecidableEq

def legacyRouteComparable (left right : EvaluationIdentity) : Prop :=
  left.routeDigest = right.routeDigest

def compositeComparable (left right : EvaluationIdentity) : Prop :=
  left = right

def currentIdentity : EvaluationIdentity :=
  { routeDigest := 100, riskDigest := 200 }

def staleRiskIdentity : EvaluationIdentity :=
  { routeDigest := 100, riskDigest := 201 }

theorem legacy_route_identity_does_not_prove_composite_comparability :
    legacyRouteComparable currentIdentity staleRiskIdentity ∧
    ¬ compositeComparable currentIdentity staleRiskIdentity := by
  simp [legacyRouteComparable, compositeComparable, currentIdentity,
    staleRiskIdentity]

end ASPProof.AXLE.SearchRouteRiskFeasibleGraphSelectionCost
