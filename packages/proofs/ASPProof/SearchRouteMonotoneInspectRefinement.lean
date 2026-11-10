-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteMonotoneInspectRefinement

structure InspectState where
  ambiguity : Nat
  roundsRemaining : Nat
  transitionsRemaining : Nat
  deriving DecidableEq, Repr

def DecisionResolved (state : InspectState) : Prop :=
  state.ambiguity = 0

def InspectStep
    (current next : InspectState) : Prop :=
  next.ambiguity < current.ambiguity ∧
  next.roundsRemaining < current.roundsRemaining ∧
  next.transitionsRemaining < current.transitionsRemaining

theorem inspect_self_loop_is_rejected (state : InspectState) :
    ¬ InspectStep state state := by
  intro step
  exact (Nat.lt_irrefl state.ambiguity) step.1

theorem unchanged_ambiguity_is_rejected
    (current next : InspectState)
    (unchanged : next.ambiguity = current.ambiguity) :
    ¬ InspectStep current next := by
  intro step
  exact (Nat.ne_of_lt step.1) unchanged

theorem resolved_state_has_no_refinement
    (state next : InspectState)
    (resolved : DecisionResolved state) :
    ¬ InspectStep state next := by
  intro step
  unfold InspectStep at step
  unfold DecisionResolved at resolved
  rw [resolved] at step
  exact (Nat.not_lt_zero next.ambiguity) step.1

inductive RefinementRun :
    InspectState → InspectState → Nat → Prop where
  | done (state : InspectState) :
      RefinementRun state state 0
  | step
      {current next final : InspectState}
      {steps : Nat}
      (progress : InspectStep current next)
      (rest : RefinementRun next final steps) :
      RefinementRun current final (Nat.succ steps)

theorem run_length_le_initial_ambiguity
    {initial final : InspectState}
    {steps : Nat}
    (run : RefinementRun initial final steps) :
    steps ≤ initial.ambiguity := by
  induction run with
  | done state =>
      exact Nat.zero_le state.ambiguity
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.1)

theorem run_length_le_initial_rounds
    {initial final : InspectState}
    {steps : Nat}
    (run : RefinementRun initial final steps) :
    steps ≤ initial.roundsRemaining := by
  induction run with
  | done state =>
      exact Nat.zero_le state.roundsRemaining
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.1)

theorem run_length_le_initial_transitions
    {initial final : InspectState}
    {steps : Nat}
    (run : RefinementRun initial final steps) :
    steps ≤ initial.transitionsRemaining := by
  induction run with
  | done state =>
      exact Nat.zero_le state.transitionsRemaining
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.2)

def initialExample : InspectState :=
  {
    ambiguity := 3
    roundsRemaining := 4
    transitionsRemaining := 4
  }

def refinedExample : InspectState :=
  {
    ambiguity := 2
    roundsRemaining := 3
    transitionsRemaining := 3
  }

def stalledExample : InspectState :=
  {
    ambiguity := 3
    roundsRemaining := 3
    transitionsRemaining := 3
  }

theorem example_refinement_is_valid :
    InspectStep initialExample refinedExample := by
  unfold InspectStep initialExample refinedExample
  decide

theorem budget_consumption_without_information_is_invalid :
    ¬ InspectStep initialExample stalledExample := by
  unfold InspectStep initialExample stalledExample
  decide

def resolvedExample : InspectState :=
  {
    ambiguity := 0
    roundsRemaining := 1
    transitionsRemaining := 1
  }

theorem resolved_example_has_no_next_step (next : InspectState) :
    ¬ InspectStep resolvedExample next :=
    resolved_state_has_no_refinement
    resolvedExample
    next
    (by
      unfold DecisionResolved resolvedExample
      decide)

end ASPProof.SearchRouteMonotoneInspectRefinement
