namespace ASPProof.SearchRouteContinuationAwareInspect

structure SplitOption where
  ambiguityAfter : Nat
  queryTokens : Nat
  residualClosureLowerBound : Nat
  deriving DecidableEq, Repr

def Informative
    (initialAmbiguity : Nat)
    (option : SplitOption) : Prop :=
  option.ambiguityAfter < initialAmbiguity

def ambiguityReduction
    (initialAmbiguity : Nat)
    (option : SplitOption) : Nat :=
  initialAmbiguity - option.ambiguityAfter

def ImmediateNoWorse
    (initialAmbiguity : Nat)
    (left right : SplitOption) : Prop :=
  ambiguityReduction initialAmbiguity left * right.queryTokens ≥
  ambiguityReduction initialAmbiguity right * left.queryTokens

def minimumTotalTokens (option : SplitOption) : Nat :=
  option.queryTokens + option.residualClosureLowerBound

def ContinuationFeasible
    (tokenBudget : Nat)
    (option : SplitOption) : Prop :=
  minimumTotalTokens option ≤ tokenBudget

def greedySplit : SplitOption :=
  {
    ambiguityAfter := 1
    queryTokens := 1
    residualClosureLowerBound := 2
  }

def strategicSplit : SplitOption :=
  {
    ambiguityAfter := 2
    queryTokens := 1
    residualClosureLowerBound := 1
  }

theorem greedy_split_is_informative :
    Informative 4 greedySplit := by
  unfold Informative greedySplit
  decide

theorem strategic_split_is_informative :
    Informative 4 strategicSplit := by
  unfold Informative strategicSplit
  decide

theorem greedy_is_immediately_no_worse :
    ImmediateNoWorse 4 greedySplit strategicSplit := by
  unfold ImmediateNoWorse ambiguityReduction greedySplit strategicSplit
  decide

theorem greedy_minimum_total_is_three :
    minimumTotalTokens greedySplit = 3 := by
  unfold minimumTotalTokens greedySplit
  decide

theorem strategic_minimum_total_is_two :
    minimumTotalTokens strategicSplit = 2 := by
  unfold minimumTotalTokens strategicSplit
  decide

theorem greedy_is_not_continuation_feasible :
    ¬ ContinuationFeasible 2 greedySplit := by
  unfold ContinuationFeasible minimumTotalTokens greedySplit
  decide

theorem strategic_is_continuation_feasible :
    ContinuationFeasible 2 strategicSplit := by
  unfold ContinuationFeasible minimumTotalTokens strategicSplit
  decide

theorem immediate_preference_does_not_imply_continuation_feasibility :
    ImmediateNoWorse 4 greedySplit strategicSplit ∧
      ¬ ContinuationFeasible 2 greedySplit ∧
      ContinuationFeasible 2 strategicSplit :=
  ⟨
    greedy_is_immediately_no_worse,
    greedy_is_not_continuation_feasible,
    strategic_is_continuation_feasible
  ⟩

end ASPProof.SearchRouteContinuationAwareInspect
