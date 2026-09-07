-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness

namespace ASPProof.SearchRouteBoundedDominanceWitnessChain

open ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness

inductive ResolutionStatus where
  | retained
  | exhausted
deriving DecidableEq

structure WitnessResolution where
  endpoint : Candidate
  steps : Nat
  status : ResolutionStatus
deriving DecidableEq

def resolveWitnessChain
    (candidates : List Candidate) :
    Nat → Candidate → WitnessResolution
  | 0, current =>
      match findDominator candidates current with
      | none =>
          {
            endpoint := current
            steps := 0
            status := ResolutionStatus.retained
          }
      | some _ =>
          {
            endpoint := current
            steps := 0
            status := ResolutionStatus.exhausted
          }
  | fuel + 1, current =>
      match findDominator candidates current with
      | none =>
          {
            endpoint := current
            steps := 0
            status := ResolutionStatus.retained
          }
      | some witness =>
          let tail := resolveWitnessChain candidates fuel witness
          {
            endpoint := tail.endpoint
            steps := Nat.succ tail.steps
            status := tail.status
          }

theorem resolution_steps_bounded_by_fuel
    (candidates : List Candidate)
    (fuel : Nat)
    (start : Candidate) :
    (resolveWitnessChain candidates fuel start).steps ≤ fuel := by
  induction fuel generalizing start with
  | zero =>
      unfold resolveWitnessChain
      cases findDominator candidates start <;> exact Nat.le_refl 0
  | succ fuel inductionHypothesis =>
      unfold resolveWitnessChain
      cases found : findDominator candidates start with
      | none =>
          exact Nat.zero_le _
      | some witness =>
          change
            Nat.succ
                (resolveWitnessChain candidates fuel witness).steps ≤
              Nat.succ fuel
          exact
            Nat.succ_le_succ
              (inductionHypothesis witness)

theorem retained_resolution_endpoint_is_kept
    (candidates : List Candidate)
    (fuel : Nat)
    (start : Candidate)
    (retained :
      (resolveWitnessChain candidates fuel start).status =
        ResolutionStatus.retained) :
    keepCandidate
        candidates
        (resolveWitnessChain candidates fuel start).endpoint =
      true := by
  induction fuel generalizing start with
  | zero =>
      unfold resolveWitnessChain at retained ⊢
      cases found : findDominator candidates start with
      | none =>
          exact
            findDominator_none_candidate_kept
              candidates start found
      | some witness =>
          rw [found] at retained
          cases retained
  | succ fuel inductionHypothesis =>
      unfold resolveWitnessChain at retained ⊢
      cases found : findDominator candidates start with
      | none =>
          exact
            findDominator_none_candidate_kept
              candidates start found
      | some witness =>
          rw [found] at retained
          change
            (resolveWitnessChain candidates fuel witness).status =
              ResolutionStatus.retained at retained
          change
            keepCandidate
                candidates
                (resolveWitnessChain candidates fuel witness).endpoint =
              true
          exact inductionHypothesis witness retained

def resolveWithCandidateCapacity
    (candidates : List Candidate)
    (start : Candidate) : WitnessResolution :=
  resolveWitnessChain candidates candidates.length start

theorem capacity_resolution_steps_are_bounded
    (candidates : List Candidate)
    (start : Candidate) :
    (resolveWithCandidateCapacity candidates start).steps ≤
      candidates.length :=
  resolution_steps_bounded_by_fuel
    candidates candidates.length start

def exampleCandidates : List Candidate :=
  [slowerRoute, fasterRoute, slowestRoute]

theorem one_step_fuel_is_insufficient_for_example :
    (resolveWitnessChain exampleCandidates 1 slowestRoute).status =
      ResolutionStatus.exhausted := by
  decide

theorem two_step_fuel_reaches_retained_example_endpoint :
    (resolveWitnessChain exampleCandidates 2 slowestRoute).status =
        ResolutionStatus.retained ∧
      (resolveWitnessChain exampleCandidates 2 slowestRoute).endpoint =
        fasterRoute ∧
      (resolveWitnessChain exampleCandidates 2 slowestRoute).steps = 2 := by
  decide

def totalCost (cost : CostVector) : Nat :=
  cost.graphHops +
    cost.projectedTokens +
    cost.interactionRounds +
    cost.searchCacheMisses +
    cost.modelCacheMisses

structure WitnessEdge
    (removed witness : Candidate) where
  dominates :
    StrictDominates witness removed = true
  rankDescent :
    totalCost witness.cost < totalCost removed.cost

inductive DescendingWitnessChain :
    Candidate → Candidate → Nat → Prop
  | endpoint (candidate : Candidate) :
      DescendingWitnessChain candidate candidate 0
  | step
      {removed witness endpoint : Candidate}
      {length : Nat} :
      WitnessEdge removed witness →
      DescendingWitnessChain witness endpoint length →
      DescendingWitnessChain removed endpoint (Nat.succ length)

theorem descending_chain_rank_relation
    {start endpoint : Candidate}
    {length : Nat}
    (chain : DescendingWitnessChain start endpoint length) :
    (length = 0 ∧ start = endpoint) ∨
      totalCost endpoint.cost < totalCost start.cost := by
  induction chain with
  | endpoint candidate =>
      exact Or.inl ⟨rfl, rfl⟩
  | step edge tailChain inductionHypothesis =>
      apply Or.inr
      cases inductionHypothesis with
      | inl zeroEndpoint =>
          rw [← zeroEndpoint.2]
          exact edge.rankDescent
      | inr tailDescent =>
          exact Nat.lt_trans tailDescent edge.rankDescent

theorem nonempty_descending_chain_cannot_cycle
    (candidate : Candidate)
    (length : Nat) :
    ¬ DescendingWitnessChain
        candidate candidate (Nat.succ length) := by
  intro chain
  have relation := descending_chain_rank_relation chain
  cases relation with
  | inl zeroLength =>
      cases zeroLength.1
  | inr strictCycle =>
      exact Nat.lt_irrefl _ strictCycle

theorem slowestToSlower :
    WitnessEdge slowestRoute slowerRoute where
  dominates := by decide
  rankDescent := by decide

theorem slowerToFaster :
    WitnessEdge slowerRoute fasterRoute where
  dominates := by decide
  rankDescent := by decide

theorem exampleDescendingChain :
    DescendingWitnessChain slowestRoute fasterRoute 2 :=
  DescendingWitnessChain.step
    slowestToSlower
    (DescendingWitnessChain.step
      slowerToFaster
      (DescendingWitnessChain.endpoint fasterRoute))

theorem example_chain_has_retained_endpoint :
    keepCandidate exampleCandidates fasterRoute = true := by
  decide

structure WitnessChainReceiptBudget where
  fixedBytes : Nat
  bytesPerStep : Nat
  candidateCapacity : Nat

def witnessChainReceiptUpperBound
    (budget : WitnessChainReceiptBudget) : Nat :=
  budget.fixedBytes +
    budget.bytesPerStep * budget.candidateCapacity

theorem explicit_witness_chain_receipt_is_capacity_bounded
    (budget : WitnessChainReceiptBudget)
    (observedBytes : Nat)
    (withinBudget :
      observedBytes ≤ witnessChainReceiptUpperBound budget) :
    observedBytes ≤ witnessChainReceiptUpperBound budget :=
  withinBudget

def summarizedWitnessChainReceiptBytes
    (fixedSummaryBytes _chainLength : Nat) : Nat :=
  fixedSummaryBytes

theorem summarized_witness_chain_receipt_is_length_independent
    (fixedSummaryBytes firstLength secondLength : Nat) :
    summarizedWitnessChainReceiptBytes fixedSummaryBytes firstLength =
      summarizedWitnessChainReceiptBytes fixedSummaryBytes secondLength :=
  rfl

end ASPProof.SearchRouteBoundedDominanceWitnessChain
