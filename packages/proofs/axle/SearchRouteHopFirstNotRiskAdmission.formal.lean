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

def shortUnsafe : Candidate :=
  { cost := { graphHops := 1, modelPrefixRecomputations := 1 }
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

theorem hop_first_preference_does_not_imply_risk_admission :
    hopFirstBetter shortUnsafe longerSafe ∧
    shortUnsafe.routeWithinBudget = true ∧
    ¬ riskFeasible shortUnsafe ∧
    admitted longerSafe := by
  sorry

end ASPProof.AXLE.SearchRouteRiskFeasibleGraphSelectionCost
