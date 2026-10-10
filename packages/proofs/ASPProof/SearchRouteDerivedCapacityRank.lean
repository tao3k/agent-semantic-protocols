-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCapacityRankCertificate

namespace ASPProof.SearchRouteDerivedCapacityRank

open SearchRouteFiniteMultiobjectiveParetoMaskWitness
open SearchRouteBoundedDominanceWitnessChain
open SearchRouteCapacityRankCertificate

theorem bool_and_true_parts :
    ∀ left right : Bool,
      (left && right) = true →
        left = true ∧ right = true
  | false, false, impossible => by
      cases impossible
  | false, true, impossible => by
      cases impossible
  | true, false, impossible => by
      cases impossible
  | true, true, _ =>
      ⟨rfl, rfl⟩

theorem bool_or_true_cases :
    ∀ left right : Bool,
      (left || right) = true →
        left = true ∨ right = true
  | false, false, impossible => by
      cases impossible
  | false, true, _ =>
      Or.inr rfl
  | true, false, _ =>
      Or.inl rfl
  | true, true, _ =>
      Or.inl rfl

theorem natLe_true_implies_le :
    ∀ left right,
      natLe left right = true →
        left ≤ right
  | 0, right, _ =>
      Nat.zero_le right
  | Nat.succ left, 0, impossible => by
      cases impossible
  | Nat.succ left, Nat.succ right, comparison => by
      change natLe left right = true at comparison
      exact Nat.succ_le_succ
        (natLe_true_implies_le left right comparison)

theorem natLt_true_implies_lt :
    ∀ left right,
      natLt left right = true →
        left < right
  | 0, 0, impossible => by
      cases impossible
  | 0, Nat.succ right, _ =>
      Nat.zero_lt_succ right
  | Nat.succ left, 0, impossible => by
      cases impossible
  | Nat.succ left, Nat.succ right, comparison => by
      change natLt left right = true at comparison
      exact Nat.succ_lt_succ
        (natLt_true_implies_lt left right comparison)

theorem lt_implies_natLt_true :
    ∀ left right,
      left < right →
        natLt left right = true
  | 0, 0, impossible => by
      exact False.elim (Nat.not_lt_zero 0 impossible)
  | 0, Nat.succ _, _ =>
      rfl
  | Nat.succ left, 0, impossible => by
      exact False.elim (Nat.not_lt_zero (Nat.succ left) impossible)
  | Nat.succ left, Nat.succ right, comparison => by
      change natLt left right = true
      exact lt_implies_natLt_true left right
        (Nat.lt_of_succ_lt_succ comparison)

theorem natLt_true_is_transitive
    (left middle right : Nat)
    (leftMiddle : natLt left middle = true)
    (middleRight : natLt middle right = true) :
    natLt left right = true := by
  apply lt_implies_natLt_true
  exact Nat.lt_trans
    (natLt_true_implies_lt left middle leftMiddle)
    (natLt_true_implies_lt middle right middleRight)

theorem strict_dominance_decreases_total_cost
    (left right : Candidate)
    (dominates : StrictDominates left right = true) :
    totalCost left.cost < totalCost right.cost := by
  unfold StrictDominates at dominates
  have dominanceParts :=
    bool_and_true_parts
      (NoWorse left.cost right.cost)
      (StrictlyImproves left.cost right.cost)
      dominates
  have noWorse := dominanceParts.1
  have improves := dominanceParts.2

  unfold NoWorse at noWorse
  have noWorse5 := bool_and_true_parts _ _ noWorse
  have noWorse4 := bool_and_true_parts _ _ noWorse5.1
  have noWorse3 := bool_and_true_parts _ _ noWorse4.1
  have noWorse2 := bool_and_true_parts _ _ noWorse3.1

  have graphHopsLe :=
    natLe_true_implies_le _ _ noWorse2.1
  have projectedTokensLe :=
    natLe_true_implies_le _ _ noWorse2.2
  have interactionRoundsLe :=
    natLe_true_implies_le _ _ noWorse3.2
  have searchCacheMissesLe :=
    natLe_true_implies_le _ _ noWorse4.2
  have modelCacheMissesLe :=
    natLe_true_implies_le _ _ noWorse5.2

  unfold StrictlyImproves at improves
  have improves5 := bool_or_true_cases _ _ improves
  cases improves5 with
  | inr modelCacheMissesImproves =>
      unfold totalCost
      exact Nat.add_lt_add_of_le_of_lt
        (Nat.add_le_add
          (Nat.add_le_add
            (Nat.add_le_add graphHopsLe projectedTokensLe)
            interactionRoundsLe)
          searchCacheMissesLe)
        (natLt_true_implies_lt _ _ modelCacheMissesImproves)
  | inl improves4 =>
      have improves4Parts := bool_or_true_cases _ _ improves4
      cases improves4Parts with
      | inr searchCacheMissesImproves =>
          unfold totalCost
          exact Nat.add_lt_add_of_lt_of_le
            (Nat.add_lt_add_of_le_of_lt
              (Nat.add_le_add
                (Nat.add_le_add graphHopsLe projectedTokensLe)
                interactionRoundsLe)
              (natLt_true_implies_lt _ _ searchCacheMissesImproves))
            modelCacheMissesLe
      | inl improves3 =>
          have improves3Parts := bool_or_true_cases _ _ improves3
          cases improves3Parts with
          | inr interactionRoundsImproves =>
              unfold totalCost
              exact Nat.add_lt_add_of_lt_of_le
                (Nat.add_lt_add_of_lt_of_le
                  (Nat.add_lt_add_of_le_of_lt
                    (Nat.add_le_add graphHopsLe projectedTokensLe)
                    (natLt_true_implies_lt _ _ interactionRoundsImproves))
                  searchCacheMissesLe)
                modelCacheMissesLe
          | inl improves2 =>
              have improves2Parts := bool_or_true_cases _ _ improves2
              cases improves2Parts with
              | inr projectedTokensImproves =>
                  unfold totalCost
                  exact Nat.add_lt_add_of_lt_of_le
                    (Nat.add_lt_add_of_lt_of_le
                      (Nat.add_lt_add_of_lt_of_le
                        (Nat.add_lt_add_of_le_of_lt
                          graphHopsLe
                          (natLt_true_implies_lt _ _ projectedTokensImproves))
                        interactionRoundsLe)
                      searchCacheMissesLe)
                    modelCacheMissesLe
              | inl graphHopsImproves =>
                  unfold totalCost
                  exact Nat.add_lt_add_of_lt_of_le
                    (Nat.add_lt_add_of_lt_of_le
                      (Nat.add_lt_add_of_lt_of_le
                        (Nat.add_lt_add_of_lt_of_le
                          (natLt_true_implies_lt _ _ graphHopsImproves)
                          projectedTokensLe)
                        interactionRoundsLe)
                      searchCacheMissesLe)
                    modelCacheMissesLe

theorem strict_dominance_decreases_total_cost_bool
    (left right : Candidate)
    (dominates : StrictDominates left right = true) :
    natLt (totalCost left.cost) (totalCost right.cost) = true :=
  lt_implies_natLt_true _ _
    (strict_dominance_decreases_total_cost left right dominates)

def lowerCostCount (candidates : List Candidate) (pivot : Candidate) : Nat :=
  match candidates with
  | [] => 0
  | head :: tail =>
      if natLt (totalCost head.cost) (totalCost pivot.cost)
      then Nat.succ (lowerCostCount tail pivot)
      else lowerCostCount tail pivot

theorem lower_cost_count_le_length
    (candidates : List Candidate)
    (pivot : Candidate) :
    lowerCostCount candidates pivot ≤ candidates.length := by
  induction candidates with
  | nil =>
      exact Nat.le_refl 0
  | cons head tail inductionHypothesis =>
      unfold lowerCostCount
      cases comparison :
        natLt (totalCost head.cost) (totalCost pivot.cost)
      with
      | false =>
          change lowerCostCount tail pivot ≤ Nat.succ tail.length
          exact Nat.le_succ_of_le inductionHypothesis
      | true =>
          change
            Nat.succ (lowerCostCount tail pivot) ≤
              Nat.succ tail.length
          exact Nat.succ_le_succ inductionHypothesis

theorem lower_cost_count_monotone
    (candidates : List Candidate)
    (lower upper : Candidate)
    (threshold :
      natLt (totalCost lower.cost) (totalCost upper.cost) = true) :
    lowerCostCount candidates lower ≤
      lowerCostCount candidates upper := by
  induction candidates with
  | nil =>
      exact Nat.le_refl 0
  | cons head tail inductionHypothesis =>
      unfold lowerCostCount
      cases headLower :
        natLt (totalCost head.cost) (totalCost lower.cost)
      <;> cases headUpper :
        natLt (totalCost head.cost) (totalCost upper.cost)
      · change
          lowerCostCount tail lower ≤
            lowerCostCount tail upper
        exact inductionHypothesis
      · change
          lowerCostCount tail lower ≤
            Nat.succ (lowerCostCount tail upper)
        exact Nat.le_succ_of_le inductionHypothesis
      · change
          Nat.succ (lowerCostCount tail lower) ≤
            lowerCostCount tail upper
        have contradiction :
            natLt (totalCost head.cost) (totalCost upper.cost) = true :=
          natLt_true_is_transitive _ _ _ headLower threshold
        have impossible : false = true :=
          headUpper.symm.trans contradiction
        cases impossible
      · change
          Nat.succ (lowerCostCount tail lower) ≤
            Nat.succ (lowerCostCount tail upper)
        exact Nat.succ_le_succ inductionHypothesis

theorem lower_cost_count_strict_of_appears
    (candidates : List Candidate)
    (lower upper : Candidate)
    (lowerAppears : Appears lower candidates)
    (threshold :
      natLt (totalCost lower.cost) (totalCost upper.cost) = true) :
    lowerCostCount candidates lower <
      lowerCostCount candidates upper := by
  induction lowerAppears with
  | head tail =>
      unfold lowerCostCount
      rw [natLt_is_irreflexive, threshold]
      exact Nat.lt_succ_of_le
        (lower_cost_count_monotone tail lower upper threshold)
  | tail head tail lowerAppears inductionHypothesis =>
      unfold lowerCostCount
      cases headLower :
        natLt (totalCost head.cost) (totalCost lower.cost)
      <;> cases headUpper :
        natLt (totalCost head.cost) (totalCost upper.cost)
      · change
          lowerCostCount tail lower <
            lowerCostCount tail upper
        exact inductionHypothesis
      · change
          lowerCostCount tail lower <
            Nat.succ (lowerCostCount tail upper)
        exact Nat.lt_succ_of_lt inductionHypothesis
      · change
          Nat.succ (lowerCostCount tail lower) <
            lowerCostCount tail upper
        have contradiction :
            natLt (totalCost head.cost) (totalCost upper.cost) = true :=
          natLt_true_is_transitive _ _ _ headLower threshold
        have impossible : false = true :=
          headUpper.symm.trans contradiction
        cases impossible
      · change
          Nat.succ (lowerCostCount tail lower) <
            Nat.succ (lowerCostCount tail upper)
        exact Nat.succ_lt_succ inductionHypothesis

theorem lower_cost_count_lt_length_of_appears
    (candidate : Candidate)
    (candidates : List Candidate)
    (appears : Appears candidate candidates) :
    lowerCostCount candidates candidate < candidates.length := by
  induction appears with
  | head tail =>
      unfold lowerCostCount
      rw [natLt_is_irreflexive]
      exact Nat.lt_succ_of_le
        (lower_cost_count_le_length tail candidate)
  | tail head tail appears inductionHypothesis =>
      unfold lowerCostCount
      cases comparison :
        natLt (totalCost head.cost) (totalCost candidate.cost)
      with
      | false =>
          change lowerCostCount tail candidate < Nat.succ tail.length
          exact Nat.lt_succ_of_lt inductionHypothesis
      | true =>
          change
            Nat.succ (lowerCostCount tail candidate) <
              Nat.succ tail.length
          exact Nat.succ_lt_succ inductionHypothesis

theorem list_member_implies_appears
    (candidate : Candidate)
    (candidates : List Candidate)
    (member : candidate ∈ candidates) :
    Appears candidate candidates := by
  induction member with
  | head tail =>
      exact Appears.head tail
  | tail head member inductionHypothesis =>
      exact Appears.tail head _ inductionHypothesis

def derivedCapacityRankCertificate
    (candidates : List Candidate) :
    CapacityRankCertificate candidates where
  rank := lowerCostCount candidates
  selectedWitnessDecreases := by
    intro removed witness found
    have sound :=
      findDominator_some_is_sound candidates removed witness found
    exact lower_cost_count_strict_of_appears
      candidates
      witness
      removed
      sound.1
      (strict_dominance_decreases_total_cost_bool
        witness
        removed
        sound.2)
  candidateRankBounded := by
    intro candidate member
    exact lower_cost_count_lt_length_of_appears
      candidate
      candidates
      (list_member_implies_appears candidate candidates member)

theorem finite_candidate_capacity_fuel_is_retained
    (candidates : List Candidate)
    (start : Candidate)
    (startMember : start ∈ candidates) :
    (resolveWitnessChain candidates candidates.length start).status =
      ResolutionStatus.retained :=
  capacity_fuel_is_not_exhausted
    candidates
    (derivedCapacityRankCertificate candidates)
    start
    startMember

theorem finite_candidate_capacity_fuel_reaches_kept_endpoint
    (candidates : List Candidate)
    (start : Candidate)
    (startMember : start ∈ candidates) :
    keepCandidate candidates
      (resolveWitnessChain candidates candidates.length start).endpoint = true :=
  capacity_fuel_reaches_kept_endpoint
    candidates
    (derivedCapacityRankCertificate candidates)
    start
    startMember

end ASPProof.SearchRouteDerivedCapacityRank
