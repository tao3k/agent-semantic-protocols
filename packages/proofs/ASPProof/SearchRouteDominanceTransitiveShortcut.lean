import ASPProof.SearchRouteDerivedCapacityRank

namespace ASPProof.SearchRouteDominanceTransitiveShortcut

open SearchRouteFiniteMultiobjectiveParetoMaskWitness
open SearchRouteBoundedDominanceWitnessChain
open SearchRouteDerivedCapacityRank

theorem bool_and_true_of_parts
    (left right : Bool)
    (leftTrue : left = true)
    (rightTrue : right = true) :
    (left && right) = true := by
  rw [leftTrue, rightTrue]
  rfl

theorem bool_or_true_of_left
    (left right : Bool)
    (leftTrue : left = true) :
    (left || right) = true := by
  rw [leftTrue]
  rfl

theorem bool_or_true_of_right
    (left right : Bool)
    (rightTrue : right = true) :
    (left || right) = true := by
  rw [rightTrue]
  cases left <;> rfl

theorem le_implies_natLe_true :
    ∀ left right,
      left ≤ right →
        natLe left right = true
  | 0, right, _ => by
      cases right <;> rfl
  | Nat.succ left, 0, impossible => by
      exact False.elim (Nat.not_succ_le_zero left impossible)
  | Nat.succ left, Nat.succ right, comparison => by
      change natLe left right = true
      exact le_implies_natLe_true left right
        (Nat.le_of_succ_le_succ comparison)

structure CostNoWorse (left right : CostVector) : Prop where
  graphHops : left.graphHops ≤ right.graphHops
  projectedTokens : left.projectedTokens ≤ right.projectedTokens
  interactionRounds : left.interactionRounds ≤ right.interactionRounds
  searchCacheMisses : left.searchCacheMisses ≤ right.searchCacheMisses
  modelCacheMisses : left.modelCacheMisses ≤ right.modelCacheMisses

inductive CostStrictImprovement (left right : CostVector) : Prop where
  | graphHops :
      left.graphHops < right.graphHops →
        CostStrictImprovement left right
  | projectedTokens :
      left.projectedTokens < right.projectedTokens →
        CostStrictImprovement left right
  | interactionRounds :
      left.interactionRounds < right.interactionRounds →
        CostStrictImprovement left right
  | searchCacheMisses :
      left.searchCacheMisses < right.searchCacheMisses →
        CostStrictImprovement left right
  | modelCacheMisses :
      left.modelCacheMisses < right.modelCacheMisses →
        CostStrictImprovement left right

theorem noWorse_true_to_components
    (left right : CostVector)
    (noWorse : NoWorse left right = true) :
    CostNoWorse left right := by
  unfold NoWorse at noWorse
  have parts5 := bool_and_true_parts _ _ noWorse
  have parts4 := bool_and_true_parts _ _ parts5.1
  have parts3 := bool_and_true_parts _ _ parts4.1
  have parts2 := bool_and_true_parts _ _ parts3.1
  exact {
    graphHops := natLe_true_implies_le _ _ parts2.1
    projectedTokens := natLe_true_implies_le _ _ parts2.2
    interactionRounds := natLe_true_implies_le _ _ parts3.2
    searchCacheMisses := natLe_true_implies_le _ _ parts4.2
    modelCacheMisses := natLe_true_implies_le _ _ parts5.2
  }

theorem components_to_noWorse_true
    (left right : CostVector)
    (components : CostNoWorse left right) :
    NoWorse left right = true := by
  unfold NoWorse
  apply bool_and_true_of_parts
  · apply bool_and_true_of_parts
    · apply bool_and_true_of_parts
      · apply bool_and_true_of_parts
        · exact le_implies_natLe_true _ _ components.graphHops
        · exact le_implies_natLe_true _ _ components.projectedTokens
      · exact le_implies_natLe_true _ _ components.interactionRounds
    · exact le_implies_natLe_true _ _ components.searchCacheMisses
  · exact le_implies_natLe_true _ _ components.modelCacheMisses

theorem strictlyImproves_true_to_dimension
    (left right : CostVector)
    (improves : StrictlyImproves left right = true) :
    CostStrictImprovement left right := by
  unfold StrictlyImproves at improves
  have parts5 := bool_or_true_cases _ _ improves
  cases parts5 with
  | inr modelCacheMisses =>
      exact CostStrictImprovement.modelCacheMisses
        (natLt_true_implies_lt _ _ modelCacheMisses)
  | inl first4 =>
      have parts4 := bool_or_true_cases _ _ first4
      cases parts4 with
      | inr searchCacheMisses =>
          exact CostStrictImprovement.searchCacheMisses
            (natLt_true_implies_lt _ _ searchCacheMisses)
      | inl first3 =>
          have parts3 := bool_or_true_cases _ _ first3
          cases parts3 with
          | inr interactionRounds =>
              exact CostStrictImprovement.interactionRounds
                (natLt_true_implies_lt _ _ interactionRounds)
          | inl first2 =>
              have parts2 := bool_or_true_cases _ _ first2
              cases parts2 with
              | inr projectedTokens =>
                  exact CostStrictImprovement.projectedTokens
                    (natLt_true_implies_lt _ _ projectedTokens)
              | inl graphHops =>
                  exact CostStrictImprovement.graphHops
                    (natLt_true_implies_lt _ _ graphHops)

theorem dimension_to_strictlyImproves_true
    (left right : CostVector)
    (improvement : CostStrictImprovement left right) :
    StrictlyImproves left right = true := by
  unfold StrictlyImproves
  cases improvement with
  | graphHops graphHops =>
      apply bool_or_true_of_left
      apply bool_or_true_of_left
      apply bool_or_true_of_left
      apply bool_or_true_of_left
      exact lt_implies_natLt_true _ _ graphHops
  | projectedTokens projectedTokens =>
      apply bool_or_true_of_left
      apply bool_or_true_of_left
      apply bool_or_true_of_left
      apply bool_or_true_of_right
      exact lt_implies_natLt_true _ _ projectedTokens
  | interactionRounds interactionRounds =>
      apply bool_or_true_of_left
      apply bool_or_true_of_left
      apply bool_or_true_of_right
      exact lt_implies_natLt_true _ _ interactionRounds
  | searchCacheMisses searchCacheMisses =>
      apply bool_or_true_of_left
      apply bool_or_true_of_right
      exact lt_implies_natLt_true _ _ searchCacheMisses
  | modelCacheMisses modelCacheMisses =>
      apply bool_or_true_of_right
      exact lt_implies_natLt_true _ _ modelCacheMisses

theorem costNoWorse_is_transitive
    (first second third : CostVector)
    (firstSecond : CostNoWorse first second)
    (secondThird : CostNoWorse second third) :
    CostNoWorse first third := {
  graphHops := Nat.le_trans
    firstSecond.graphHops secondThird.graphHops
  projectedTokens := Nat.le_trans
    firstSecond.projectedTokens secondThird.projectedTokens
  interactionRounds := Nat.le_trans
    firstSecond.interactionRounds secondThird.interactionRounds
  searchCacheMisses := Nat.le_trans
    firstSecond.searchCacheMisses secondThird.searchCacheMisses
  modelCacheMisses := Nat.le_trans
    firstSecond.modelCacheMisses secondThird.modelCacheMisses
}

theorem strictImprovement_followed_by_noWorse
    (first second third : CostVector)
    (improvement : CostStrictImprovement first second)
    (secondThird : CostNoWorse second third) :
    CostStrictImprovement first third := by
  cases improvement with
  | graphHops graphHops =>
      exact CostStrictImprovement.graphHops
        (Nat.lt_of_lt_of_le graphHops secondThird.graphHops)
  | projectedTokens projectedTokens =>
      exact CostStrictImprovement.projectedTokens
        (Nat.lt_of_lt_of_le
          projectedTokens secondThird.projectedTokens)
  | interactionRounds interactionRounds =>
      exact CostStrictImprovement.interactionRounds
        (Nat.lt_of_lt_of_le
          interactionRounds secondThird.interactionRounds)
  | searchCacheMisses searchCacheMisses =>
      exact CostStrictImprovement.searchCacheMisses
        (Nat.lt_of_lt_of_le
          searchCacheMisses secondThird.searchCacheMisses)
  | modelCacheMisses modelCacheMisses =>
      exact CostStrictImprovement.modelCacheMisses
        (Nat.lt_of_lt_of_le
          modelCacheMisses secondThird.modelCacheMisses)

theorem noWorse_is_transitive
    (first second third : CostVector)
    (firstSecond : NoWorse first second = true)
    (secondThird : NoWorse second third = true) :
    NoWorse first third = true :=
  components_to_noWorse_true first third
    (costNoWorse_is_transitive first second third
      (noWorse_true_to_components first second firstSecond)
      (noWorse_true_to_components second third secondThird))

theorem strict_dominance_is_transitive
    (first second third : Candidate)
    (firstSecond : StrictDominates first second = true)
    (secondThird : StrictDominates second third = true) :
    StrictDominates first third = true := by
  unfold StrictDominates at firstSecond secondThird ⊢
  have firstSecondParts :=
    bool_and_true_parts _ _ firstSecond
  have secondThirdParts :=
    bool_and_true_parts _ _ secondThird
  have firstSecondNoWorse :=
    noWorse_true_to_components
      first.cost second.cost firstSecondParts.1
  have secondThirdNoWorse :=
    noWorse_true_to_components
      second.cost third.cost secondThirdParts.1
  apply bool_and_true_of_parts
  · exact components_to_noWorse_true first.cost third.cost
      (costNoWorse_is_transitive
        first.cost second.cost third.cost
        firstSecondNoWorse
        secondThirdNoWorse)
  · exact dimension_to_strictlyImproves_true first.cost third.cost
      (strictImprovement_followed_by_noWorse
        first.cost second.cost third.cost
        (strictlyImproves_true_to_dimension
          first.cost second.cost firstSecondParts.2)
        secondThirdNoWorse)

theorem chain_endpoint_eq_or_strictly_dominates
    {start endpoint : Candidate}
    {length : Nat}
    (chain : DescendingWitnessChain start endpoint length) :
    endpoint = start ∨ StrictDominates endpoint start = true := by
  induction chain with
  | endpoint candidate =>
      exact Or.inl rfl
  | step edge tail inductionHypothesis =>
      cases inductionHypothesis with
      | inl endpointIsWitness =>
          apply Or.inr
          rw [endpointIsWitness]
          exact edge.dominates
      | inr endpointDominatesWitness =>
          exact Or.inr
            (strict_dominance_is_transitive
              _ _ _
              endpointDominatesWitness
              edge.dominates)

theorem nonempty_chain_endpoint_strictly_dominates_start
    (start endpoint : Candidate)
    (length : Nat)
    (chain :
      DescendingWitnessChain start endpoint (Nat.succ length)) :
    StrictDominates endpoint start = true := by
  have relation :=
    chain_endpoint_eq_or_strictly_dominates chain
  cases relation with
  | inr dominates =>
      exact dominates
  | inl endpointIsStart =>
      rw [endpointIsStart] at chain
      exact False.elim
        (nonempty_descending_chain_cannot_cycle
          start length chain)

structure TransitiveShortcut
    (start endpoint : Candidate) where
  chainLength : Nat
  chain :
    DescendingWitnessChain
      start endpoint (Nat.succ chainLength)
  endpointDominatesStart :
    StrictDominates endpoint start = true

def buildTransitiveShortcut
    (start endpoint : Candidate)
    (length : Nat)
    (chain :
      DescendingWitnessChain start endpoint (Nat.succ length)) :
    TransitiveShortcut start endpoint := {
  chainLength := length
  chain := chain
  endpointDominatesStart :=
    nonempty_chain_endpoint_strictly_dominates_start
      start endpoint length chain
}

theorem shortcut_preserves_removal_soundness
    (start endpoint : Candidate)
    (shortcut : TransitiveShortcut start endpoint) :
    StrictDominates endpoint start = true :=
  shortcut.endpointDominatesStart

def explicitChainReceiptTokens (length : Nat) : Nat :=
  6 + length

def shortcutReceiptTokens : Nat :=
  6

theorem shortcut_receipt_is_length_independent :
    shortcutReceiptTokens = 6 := by
  rfl

theorem shortcut_receipt_beats_nonempty_explicit_chain
    (length : Nat) :
    shortcutReceiptTokens <
      explicitChainReceiptTokens (Nat.succ length) := by
  unfold shortcutReceiptTokens
  unfold explicitChainReceiptTokens
  exact Nat.add_lt_add_left (Nat.zero_lt_succ length) 6

end ASPProof.SearchRouteDominanceTransitiveShortcut
