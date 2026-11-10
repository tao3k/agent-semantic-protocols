-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteProofCarryingStateMachine

namespace ASPProof.SearchRouteEndToEndTraceSafety

open ASPProof.SearchRouteProofCarryingStateMachine

inductive ExploreRefinementRun :
    ExploreState → ExploreState → Prop where
  | done (state : ExploreState) :
      ExploreRefinementRun state state
  | step
      {current next final : ExploreState}
      (transition :
        LegalTransition (.exploring current) (.exploring next))
      (rest : ExploreRefinementRun next final) :
      ExploreRefinementRun current final

theorem exploration_run_preserves_context
    {initial final : ExploreState}
    (run : ExploreRefinementRun initial final) :
    initial.context = final.context := by
  induction run with
  | done state =>
      rfl
  | step transition rest inductionHypothesis =>
      cases transition with
      | refine current next sameContext lessAmbiguity
          chargedRound chargedTransition =>
          exact Eq.trans sameContext inductionHypothesis

structure ExecutionTrace where
  initial : ExploreState
  resolved : ExploreState
  decision : DecisionState
  admitted : AdmittedState
  executing : ExecutingState
  refinements : ExploreRefinementRun initial resolved
  decisionTransition :
    LegalTransition (.exploring resolved) (.decisionReady decision)
  admissionTransition :
    LegalTransition (.decisionReady decision) (.admitted admitted)
  executionTransition :
    LegalTransition (.admitted admitted) (.executing executing)

theorem decision_transition_is_resolved
    (resolved : ExploreState)
    (decision : DecisionState)
    (transition :
      LegalTransition (.exploring resolved) (.decisionReady decision)) :
    resolved.ambiguity = 0 := by
  cases transition with
  | decide current next isResolved sameContext =>
      exact isResolved

theorem decision_transition_preserves_context
    (resolved : ExploreState)
    (decision : DecisionState)
    (transition :
      LegalTransition (.exploring resolved) (.decisionReady decision)) :
    resolved.context = decision.context := by
  cases transition with
  | decide current next isResolved sameContext =>
      exact sameContext

theorem admission_transition_preserves_context
    (decision : DecisionState)
    (admitted : AdmittedState)
    (transition :
      LegalTransition (.decisionReady decision) (.admitted admitted)) :
    decision.context = admitted.context := by
  cases transition with
  | admit current next executeAction sameContext sameMode ready =>
      exact sameContext

theorem execution_trace_has_resolved_decision
    (trace : ExecutionTrace) :
    trace.resolved.ambiguity = 0 :=
  decision_transition_is_resolved
    trace.resolved
    trace.decision
    trace.decisionTransition

theorem execution_trace_has_ready_admission
    (trace : ExecutionTrace) :
    ExecutionReady trace.admitted :=
  execution_transition_requires_ready
    trace.admitted
    trace.executing
    trace.executionTransition

theorem execution_trace_preserves_context
    (trace : ExecutionTrace) :
    trace.initial.context = trace.executing.context := by
  exact Eq.trans
    (exploration_run_preserves_context trace.refinements)
    (Eq.trans
      (decision_transition_preserves_context
        trace.resolved
        trace.decision
        trace.decisionTransition)
      (Eq.trans
        (admission_transition_preserves_context
          trace.decision
          trace.admitted
          trace.admissionTransition)
        (execution_transition_preserves_context
          trace.admitted
          trace.executing
          trace.executionTransition)))

theorem execution_trace_end_to_end_safety
    (trace : ExecutionTrace) :
    trace.resolved.ambiguity = 0 ∧
      ExecutionReady trace.admitted ∧
      trace.initial.context = trace.executing.context :=
  ⟨
    execution_trace_has_resolved_decision trace,
    execution_trace_has_ready_admission trace,
    execution_trace_preserves_context trace
  ⟩

structure CompletionTrace where
  execution : ExecutionTrace
  completed : CompletedState
  completionTransition :
    LegalTransition
      (.executing execution.executing)
      (.completed completed)

theorem completion_trace_preserves_context
    (trace : CompletionTrace) :
    trace.execution.executing.context = trace.completed.context := by
  cases trace.completionTransition with
  | complete current next sameContext sameMode sameRequest =>
      exact sameContext

theorem completion_trace_preserves_mode
    (trace : CompletionTrace) :
    trace.execution.executing.completionMode =
      trace.completed.completionMode := by
  cases trace.completionTransition with
  | complete current next sameContext sameMode sameRequest =>
      exact sameMode

theorem completion_trace_preserves_request
    (trace : CompletionTrace) :
    trace.execution.executing.requestDigest =
      trace.completed.requestDigest := by
  cases trace.completionTransition with
  | complete current next sameContext sameMode sameRequest =>
      exact sameRequest

theorem cross_context_trace_is_impossible
    (trace : ExecutionTrace)
    (drift :
      trace.initial.context ≠ trace.executing.context) :
    False :=
  drift (execution_trace_preserves_context trace)

end ASPProof.SearchRouteEndToEndTraceSafety
