-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteInspectLoop

namespace SearchRouteInspectDriver

open SearchRouteDAG
open SearchRouteInspectLoop

/-!
`InspectLoopTrace` is a safety relation: its `stop` constructor permits a trace
to stop at any state.  This module adds the missing liveness premise and proves
that a total decision source produces a bounded, typed closed trace.
-/

def DecisionTotal : Prop :=
  ∀ state : InspectLoopState,
    LoopWellFormed state →
      ¬LoopClosed state →
        ∃ decision : LoopDecision, DecisionValid state decision

structure ClosedTraceReceipt (initial : InspectLoopState) where
  finalState : InspectLoopState
  steps : Nat
  trace : InspectLoopTrace initial finalState steps
  closed : LoopClosed finalState

theorem decision_total_produces_closed_trace
    (decisionTotal : DecisionTotal)
    {initial : InspectLoopState}
    (wellFormed : LoopWellFormed initial) :
    Nonempty (ClosedTraceReceipt initial) := by
  classical
  generalize workEquation : loopWork initial = work
  induction work using Nat.strongRecOn generalizing initial with
  | ind work inductionHypothesis =>
      by_cases closed : LoopClosed initial
      · exact ⟨
          {
            finalState := initial
            steps := 0
            trace := .stop initial
            closed := closed
          }
        ⟩
      · rcases decisionTotal initial wellFormed closed with ⟨decision, valid⟩
        let nextState := applyDecision initial decision
        have nextWorkLess : loopWork nextState < work := by
          rw [← workEquation]
          exact valid_decision_decreases_work valid
        have nextWellFormed : LoopWellFormed nextState :=
          valid_decision_preserves_well_formed wellFormed valid
        rcases inductionHypothesis
            (loopWork nextState)
            nextWorkLess
            nextWellFormed
            rfl with
          ⟨restReceipt⟩
        exact ⟨
          {
            finalState := restReceipt.finalState
            steps := restReceipt.steps + 1
            trace := .step decision valid restReceipt.trace
            closed := restReceipt.closed
          }
        ⟩

theorem closed_trace_is_within_initial_work
    {initial : InspectLoopState}
    (receipt : ClosedTraceReceipt initial) :
    receipt.steps ≤ loopWork initial :=
  trace_length_bound receipt.trace

theorem trace_selected_no_worse
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps) :
    graphLexNoWorse finalState.selected initial.selected := by
  induction trace with
  | stop state =>
      exact graphLexNoWorse_refl state.selected
  | step decision valid rest inductionHypothesis =>
      exact graphLexNoWorse_trans
        inductionHypothesis
        (valid_decision_selected_no_worse valid)

theorem decision_total_produces_bounded_global_optimum
    {candidateUniverse : List GraphCandidate}
    (decisionTotal : DecisionTotal)
    {initial : InspectLoopState}
    (wellFormed : LoopWellFormed initial)
    (coverage : LoopCovers candidateUniverse initial) :
    ∃ receipt : ClosedTraceReceipt initial,
      receipt.steps ≤ loopWork initial ∧
        graphLexNoWorse receipt.finalState.selected initial.selected ∧
        ∀ candidate ∈ candidateUniverse,
          graphLexNoWorse receipt.finalState.selected candidate := by
  rcases decision_total_produces_closed_trace decisionTotal wellFormed with
    ⟨receipt⟩
  have finalCoverage : LoopCovers candidateUniverse receipt.finalState :=
    trace_preserves_coverage receipt.trace coverage
  exact ⟨
    receipt,
    closed_trace_is_within_initial_work receipt,
    trace_selected_no_worse receipt.trace,
    closed_coverage_is_globally_optimal receipt.closed finalCoverage
  ⟩

theorem safety_trace_alone_allows_early_stop
    {state : InspectLoopState}
    (notClosed : ¬LoopClosed state) :
    Nonempty (InspectLoopTrace state state 0) ∧ ¬LoopClosed state :=
  ⟨⟨.stop state⟩, notClosed⟩

end SearchRouteInspectDriver
