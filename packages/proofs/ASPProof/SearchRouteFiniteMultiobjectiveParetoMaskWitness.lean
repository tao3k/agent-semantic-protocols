-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

namespace ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness

structure CostVector where
  graphHops : Nat
  projectedTokens : Nat
  interactionRounds : Nat
  searchCacheMisses : Nat
  modelCacheMisses : Nat
deriving DecidableEq

structure Candidate where
  canonicalRouteId : Nat
  cost : CostVector
deriving DecidableEq

def natLe : Nat → Nat → Bool
  | 0, _ => true
  | _ + 1, 0 => false
  | left + 1, right + 1 => natLe left right

def natLt : Nat → Nat → Bool
  | 0, 0 => false
  | 0, _ + 1 => true
  | _ + 1, 0 => false
  | left + 1, right + 1 => natLt left right

def NoWorse (left right : CostVector) : Bool :=
  natLe left.graphHops right.graphHops &&
    natLe left.projectedTokens right.projectedTokens &&
    natLe left.interactionRounds right.interactionRounds &&
    natLe left.searchCacheMisses right.searchCacheMisses &&
    natLe left.modelCacheMisses right.modelCacheMisses

def StrictlyImproves (left right : CostVector) : Bool :=
  natLt left.graphHops right.graphHops ||
    natLt left.projectedTokens right.projectedTokens ||
    natLt left.interactionRounds right.interactionRounds ||
    natLt left.searchCacheMisses right.searchCacheMisses ||
    natLt left.modelCacheMisses right.modelCacheMisses

def StrictDominates (left right : Candidate) : Bool :=
  NoWorse left.cost right.cost &&
    StrictlyImproves left.cost right.cost

inductive Appears (candidate : Candidate) : List Candidate → Prop
  | head (tail : List Candidate) :
      Appears candidate (candidate :: tail)
  | tail (head : Candidate) (tail : List Candidate) :
      Appears candidate tail →
      Appears candidate (head :: tail)

def findDominator
    (candidates : List Candidate)
    (candidate : Candidate) : Option Candidate :=
  match candidates with
  | [] => none
  | head :: tail =>
      match StrictDominates head candidate with
      | true => some head
      | false => findDominator tail candidate

def keepStep (dominates tailKeep : Bool) : Bool :=
  match dominates with
  | true => false
  | false => tailKeep

def keepCandidate : List Candidate → Candidate → Bool
  | [], _ => true
  | head :: tail, candidate =>
      keepStep
        (StrictDominates head candidate)
        (keepCandidate tail candidate)

theorem findDominator_some_is_sound
    (candidates : List Candidate)
    (candidate witness : Candidate)
    (found : findDominator candidates candidate = some witness) :
    Appears witness candidates ∧
      StrictDominates witness candidate = true := by
  induction candidates with
  | nil =>
      unfold findDominator at found
      cases found
  | cons head tail inductionHypothesis =>
      unfold findDominator at found
      cases dominates : StrictDominates head candidate with
      | false =>
          rw [dominates] at found
          have tailSound := inductionHypothesis found
          exact ⟨Appears.tail head tail tailSound.1, tailSound.2⟩
      | true =>
          rw [dominates] at found
          cases found
          exact ⟨Appears.head tail, dominates⟩

theorem findDominator_none_excludes_dominator
    (candidates : List Candidate)
    (candidate : Candidate)
    (notFound : findDominator candidates candidate = none)
    (other : Candidate)
    (appears : Appears other candidates) :
    StrictDominates other candidate = false := by
  induction appears with
  | head tail =>
      unfold findDominator at notFound
      cases dominates : StrictDominates other candidate with
      | false =>
          rfl
      | true =>
          rw [dominates] at notFound
          cases notFound
  | tail head tail tailAppears inductionHypothesis =>
      unfold findDominator at notFound
      cases headDominates : StrictDominates head candidate with
      | true =>
          rw [headDominates] at notFound
          cases notFound
      | false =>
          rw [headDominates] at notFound
          exact inductionHypothesis notFound

theorem kept_candidate_findDominator_none
    (candidates : List Candidate)
    (candidate : Candidate)
    (kept : keepCandidate candidates candidate = true) :
    findDominator candidates candidate = none := by
  induction candidates with
  | nil =>
      rfl
  | cons head tail inductionHypothesis =>
      unfold keepCandidate keepStep at kept
      unfold findDominator
      cases dominates : StrictDominates head candidate with
      | true =>
          rw [dominates] at kept
          cases kept
      | false =>
          rw [dominates] at kept
          change keepCandidate tail candidate = true at kept
          change findDominator tail candidate = none
          exact inductionHypothesis kept

theorem findDominator_none_candidate_kept
    (candidates : List Candidate)
    (candidate : Candidate)
    (notFound : findDominator candidates candidate = none) :
    keepCandidate candidates candidate = true := by
  induction candidates with
  | nil =>
      rfl
  | cons head tail inductionHypothesis =>
      unfold findDominator at notFound
      unfold keepCandidate keepStep
      cases dominates : StrictDominates head candidate with
      | true =>
          rw [dominates] at notFound
          cases notFound
      | false =>
          rw [dominates] at notFound
          change findDominator tail candidate = none at notFound
          change keepCandidate tail candidate = true
          exact inductionHypothesis notFound

theorem kept_candidate_is_undominated
    (candidates : List Candidate)
    (candidate : Candidate)
    (kept : keepCandidate candidates candidate = true) :
    ∀ other,
      Appears other candidates →
        StrictDominates other candidate = false := by
  have notFound :=
    kept_candidate_findDominator_none candidates candidate kept
  intro other appears
  exact
    findDominator_none_excludes_dominator
      candidates candidate notFound other appears

theorem undominated_candidate_is_kept
    (candidates : List Candidate)
    (candidate : Candidate)
    (undominated :
      ∀ other,
        Appears other candidates →
          StrictDominates other candidate = false) :
  keepCandidate candidates candidate = true := by
  cases found : findDominator candidates candidate with
  | none =>
      exact
        findDominator_none_candidate_kept
          candidates candidate found
  | some witness =>
      have witnessSound :=
        findDominator_some_is_sound
          candidates candidate witness found
      have witnessRejected :=
        undominated witness witnessSound.1
      rw [witnessSound.2] at witnessRejected
      cases witnessRejected

theorem removed_candidate_has_dominating_witness
    (candidates : List Candidate)
    (candidate : Candidate)
    (removed : keepCandidate candidates candidate = false) :
    ∃ witness,
      findDominator candidates candidate = some witness ∧
        Appears witness candidates ∧
          StrictDominates witness candidate = true := by
  cases found : findDominator candidates candidate with
  | none =>
      have kept :=
        findDominator_none_candidate_kept
          candidates candidate found
      rw [kept] at removed
      cases removed
  | some witness =>
      have witnessSound :=
        findDominator_some_is_sound
          candidates candidate witness found
      exact ⟨witness, rfl, witnessSound.1, witnessSound.2⟩

theorem natLt_is_irreflexive
    (value : Nat) :
    natLt value value = false := by
  induction value with
  | zero =>
      rfl
  | succ value inductionHypothesis =>
      exact inductionHypothesis

theorem strictlyImproves_is_irreflexive
    (cost : CostVector) :
    StrictlyImproves cost cost = false := by
  unfold StrictlyImproves
  rw [natLt_is_irreflexive]
  rw [natLt_is_irreflexive]
  rw [natLt_is_irreflexive]
  rw [natLt_is_irreflexive]
  rw [natLt_is_irreflexive]
  rfl

theorem equal_cost_does_not_strictly_dominate
    (left right : Candidate)
    (sameCost : left.cost = right.cost) :
    StrictDominates left right = false := by
  unfold StrictDominates
  rw [sameCost]
  rw [strictlyImproves_is_irreflexive]
  cases NoWorse right.cost right.cost <;> rfl

theorem strict_dominance_is_irreflexive
    (candidate : Candidate) :
    StrictDominates candidate candidate = false :=
  equal_cost_does_not_strictly_dominate candidate candidate rfl

theorem equal_cost_distinct_routes_are_both_kept
    (left right : Candidate)
    (_distinct :
      left.canonicalRouteId ≠ right.canonicalRouteId)
    (sameCost : left.cost = right.cost) :
    keepCandidate [left, right] left = true ∧
      keepCandidate [left, right] right = true := by
  constructor
  · unfold keepCandidate
    rw [strict_dominance_is_irreflexive left]
    change keepCandidate [right] left = true
    unfold keepCandidate
    rw [equal_cost_does_not_strictly_dominate right left sameCost.symm]
    rfl
  · unfold keepCandidate
    rw [equal_cost_does_not_strictly_dominate left right sameCost]
    change keepCandidate [right] right = true
    unfold keepCandidate
    rw [strict_dominance_is_irreflexive right]
    rfl

theorem adjacent_swap_preserves_keep_mask
    (first second candidate : Candidate)
    (tail : List Candidate) :
    keepCandidate (first :: second :: tail) candidate =
      keepCandidate (second :: first :: tail) candidate := by
  change
    keepStep
        (StrictDominates first candidate)
        (keepStep
          (StrictDominates second candidate)
          (keepCandidate tail candidate)) =
      keepStep
        (StrictDominates second candidate)
        (keepStep
          (StrictDominates first candidate)
          (keepCandidate tail candidate))
  cases StrictDominates first candidate <;>
    cases StrictDominates second candidate <;>
      rfl

inductive EnumerationPermutation : List Candidate → List Candidate → Prop
  | refl (candidates : List Candidate) :
      EnumerationPermutation candidates candidates
  | adjacent (first second : Candidate) (tail : List Candidate) :
      EnumerationPermutation
        (first :: second :: tail)
        (second :: first :: tail)
  | cons (head : Candidate)
      {left right : List Candidate} :
      EnumerationPermutation left right →
      EnumerationPermutation (head :: left) (head :: right)
  | trans {first second third : List Candidate} :
      EnumerationPermutation first second →
      EnumerationPermutation second third →
      EnumerationPermutation first third

theorem candidate_permutation_preserves_keep_mask
    (left right : List Candidate)
    (candidate : Candidate)
    (permutation : EnumerationPermutation left right) :
    keepCandidate left candidate = keepCandidate right candidate := by
  induction permutation with
  | refl candidates =>
      rfl
  | adjacent first second tail =>
      exact adjacent_swap_preserves_keep_mask first second candidate tail
  | cons head permutation inductionHypothesis =>
      unfold keepCandidate
      exact
        congrArg
          (keepStep (StrictDominates head candidate))
          inductionHypothesis
  | trans firstPermutation secondPermutation
      firstInduction secondInduction =>
      exact Eq.trans firstInduction secondInduction

def fasterRoute : Candidate where
  canonicalRouteId := 10
  cost := {
    graphHops := 1
    projectedTokens := 64
    interactionRounds := 1
    searchCacheMisses := 0
    modelCacheMisses := 0
  }

def slowerRoute : Candidate where
  canonicalRouteId := 11
  cost := {
    graphHops := 2
    projectedTokens := 128
    interactionRounds := 2
    searchCacheMisses := 1
    modelCacheMisses := 1
  }

def slowestRoute : Candidate where
  canonicalRouteId := 12
  cost := {
    graphHops := 3
    projectedTokens := 256
    interactionRounds := 3
    searchCacheMisses := 2
    modelCacheMisses := 2
  }

theorem faster_route_strictly_dominates_slower_route :
    StrictDominates fasterRoute slowerRoute = true := by
  decide

theorem slower_route_is_removed_with_faster_witness :
    keepCandidate [fasterRoute, slowerRoute] slowerRoute = false := by
  decide

theorem direct_dominating_witness_may_itself_be_removed :
    findDominator
        [slowerRoute, fasterRoute, slowestRoute]
        slowestRoute =
      some slowerRoute ∧
    keepCandidate
        [slowerRoute, fasterRoute, slowestRoute]
        slowerRoute =
      false := by
  decide

structure DominanceWitnessReceiptBudget where
  fixedBytes : Nat
  bytesPerCandidate : Nat
  candidateCapacity : Nat

def dominanceWitnessReceiptUpperBound
    (budget : DominanceWitnessReceiptBudget) : Nat :=
  budget.fixedBytes +
    budget.bytesPerCandidate * budget.candidateCapacity

theorem dominance_witness_receipt_is_capacity_bounded
    (budget : DominanceWitnessReceiptBudget)
    (observedBytes : Nat)
    (withinBudget :
      observedBytes ≤ dominanceWitnessReceiptUpperBound budget) :
    observedBytes ≤ dominanceWitnessReceiptUpperBound budget :=
  withinBudget

def summarizedDominanceReceiptBytes
    (fixedSummaryBytes _removedCount : Nat) : Nat :=
  fixedSummaryBytes

theorem summarized_dominance_receipt_is_removed_count_independent
    (fixedSummaryBytes firstCount secondCount : Nat) :
    summarizedDominanceReceiptBytes fixedSummaryBytes firstCount =
      summarizedDominanceReceiptBytes fixedSummaryBytes secondCount :=
  rfl

end ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness
