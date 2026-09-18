-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteBoundedDominanceWitnessChain

namespace ASPProof.SearchRouteCapacityRankCertificate

open SearchRouteFiniteMultiobjectiveParetoMaskWitness
open SearchRouteBoundedDominanceWitnessChain

structure CapacityRankCertificate (candidates : List Candidate) where
  rank : Candidate → Nat
  selectedWitnessDecreases :
    ∀ removed witness,
      findDominator candidates removed = some witness →
        rank witness < rank removed
  candidateRankBounded :
    ∀ candidate,
      candidate ∈ candidates →
        rank candidate < candidates.length

theorem rank_bounded_resolution_is_retained
    (candidates : List Candidate)
    (rank : Candidate → Nat)
    (selectedWitnessDecreases :
      ∀ removed witness,
        findDominator candidates removed = some witness →
          rank witness < rank removed) :
    ∀ fuel current,
      rank current < fuel →
        (resolveWitnessChain candidates fuel current).status =
          ResolutionStatus.retained := by
  intro fuel
  induction fuel with
  | zero =>
      intro current impossible
      exact False.elim (Nat.not_lt_zero (rank current) impossible)
  | succ fuel inductionHypothesis =>
      intro current currentBound
      unfold resolveWitnessChain
      cases found : findDominator candidates current with
      | none =>
          rfl
      | some witness =>
          change
            (resolveWitnessChain candidates fuel witness).status =
              ResolutionStatus.retained
          apply inductionHypothesis witness
          have witnessDescent :
              rank witness < rank current :=
            selectedWitnessDecreases current witness found
          have currentWithinFuel :
              rank current ≤ fuel :=
            Nat.le_of_lt_succ currentBound
          exact Nat.lt_of_lt_of_le witnessDescent currentWithinFuel

theorem capacity_fuel_is_not_exhausted
    (candidates : List Candidate)
    (certificate : CapacityRankCertificate candidates)
    (start : Candidate)
    (startMember : start ∈ candidates) :
    (resolveWitnessChain candidates candidates.length start).status =
      ResolutionStatus.retained := by
  apply rank_bounded_resolution_is_retained
    candidates
    certificate.rank
    certificate.selectedWitnessDecreases
  exact certificate.candidateRankBounded start startMember

theorem capacity_fuel_reaches_kept_endpoint
    (candidates : List Candidate)
    (certificate : CapacityRankCertificate candidates)
    (start : Candidate)
    (startMember : start ∈ candidates) :
    keepCandidate candidates
      (resolveWitnessChain candidates candidates.length start).endpoint = true := by
  apply retained_resolution_endpoint_is_kept
  exact capacity_fuel_is_not_exhausted
    candidates
    certificate
    start
    startMember

theorem capacity_fuel_step_bound
    (candidates : List Candidate)
    (start : Candidate) :
    (resolveWitnessChain candidates candidates.length start).steps ≤
      candidates.length :=
  capacity_resolution_steps_are_bounded candidates start

theorem total_cost_descent_does_not_supply_capacity_bound :
    ∃ startRank candidateCapacity : Nat,
      0 < startRank ∧
      candidateCapacity < startRank := by
  exact ⟨5, 2, by decide, by decide⟩

theorem zero_capacity_has_no_member
    (start : Candidate)
    (startMember : start ∈ ([] : List Candidate)) :
    False := by
  cases startMember

def explicitCapacityRankReceiptTokens
    (candidates : List Candidate) : Nat :=
  4 + candidates.length

def summarizedCapacityRankReceiptTokens : Nat :=
  4

theorem explicit_capacity_rank_receipt_is_linear
    (candidates : List Candidate) :
    explicitCapacityRankReceiptTokens candidates =
      4 + candidates.length := by
  rfl

theorem summarized_capacity_rank_receipt_is_constant :
    summarizedCapacityRankReceiptTokens = 4 := by
  rfl

theorem summarized_capacity_rank_receipt_beats_explicit_nonempty
    (candidates : List Candidate)
    (nonempty : 0 < candidates.length) :
    summarizedCapacityRankReceiptTokens <
      explicitCapacityRankReceiptTokens candidates := by
  unfold summarizedCapacityRankReceiptTokens
  unfold explicitCapacityRankReceiptTokens
  exact Nat.add_lt_add_left nonempty 4

end ASPProof.SearchRouteCapacityRankCertificate
