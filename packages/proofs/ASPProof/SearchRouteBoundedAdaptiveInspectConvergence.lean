-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteProofCarryingSelectiveDisclosure

namespace ASPProof.SearchRouteBoundedAdaptiveInspectConvergence

open ASPProof.SearchRouteProofCarryingSelectiveDisclosure

structure InspectState where
  request : DisclosureRequest
  disclosedEdges : List Nat
  remainingEdges : List Nat
  disclosedTokenUnits : Nat
  llmRounds : Nat
deriving DecidableEq, Repr

def Partitioned (state : InspectState) : Prop :=
  state.disclosedEdges ++ state.remainingEdges =
    state.request.requiredEdges

def Complete (state : InspectState) : Prop :=
  state.remainingEdges = []

def DecisionReady (chainVerified : Prop) (state : InspectState) : Prop :=
  chainVerified ∧ Partitioned state ∧ Complete state

def MissingEdgePotential (state : InspectState) : Nat :=
  state.remainingEdges.length

structure InspectStep (before after : InspectState) where
  sameRequest : after.request = before.request
  newlyDisclosed : List Nat
  nonempty : newlyDisclosed ≠ []
  splitRemaining :
    before.remainingEdges = newlyDisclosed ++ after.remainingEdges
  extendDisclosed :
    after.disclosedEdges = before.disclosedEdges ++ newlyDisclosed
  tokenAdvance :
    after.disclosedTokenUnits =
      before.disclosedTokenUnits + newlyDisclosed.length
  roundAdvance : after.llmRounds = before.llmRounds + 1

inductive InspectRun : InspectState → InspectState → Nat → Prop
  | nil (state : InspectState) :
      InspectRun state state 0
  | cons
      {startState nextState finalState : InspectState}
      {remainingSteps : Nat}
      (step : InspectStep startState nextState)
      (tail : InspectRun nextState finalState remainingSteps) :
      InspectRun startState finalState (remainingSteps + 1)

theorem inspect_step_preserves_request
    {before after : InspectState}
    (step : InspectStep before after) :
    after.request = before.request :=
  step.sameRequest

theorem inspect_step_strictly_decreases_missing_potential
    {before after : InspectState}
    (step : InspectStep before after) :
    MissingEdgePotential after < MissingEdgePotential before := by
  have positive : 0 < step.newlyDisclosed.length := by
    cases newEdges : step.newlyDisclosed with
    | nil =>
        exact False.elim (step.nonempty newEdges)
    | cons head tail =>
        simp
  unfold MissingEdgePotential
  rw [step.splitRemaining]
  simp only [List.length_append]
  exact Nat.lt_add_of_pos_left positive

theorem inspect_step_conserves_token_plus_missing_units
    {before after : InspectState}
    (step : InspectStep before after) :
    after.disclosedTokenUnits + after.remainingEdges.length =
      before.disclosedTokenUnits + before.remainingEdges.length := by
  rw [step.tokenAdvance, step.splitRemaining]
  simp only [List.length_append]
  omega

theorem inspect_step_preserves_partition
    {before after : InspectState}
    (beforePartitioned : Partitioned before)
    (step : InspectStep before after) :
    Partitioned after := by
  unfold Partitioned at beforePartitioned ⊢
  rw [step.extendDisclosed, step.sameRequest]
  rw [List.append_assoc, ← step.splitRemaining]
  exact beforePartitioned

theorem inspect_step_is_not_reflexive
    (state : InspectState) :
    InspectStep state state → False := by
  intro step
  exact (Nat.lt_irrefl (MissingEdgePotential state))
    (inspect_step_strictly_decreases_missing_potential step)

theorem batched_step_reduces_potential_by_at_least_two
    {before after : InspectState}
    (step : InspectStep before after)
    (batch : 1 < step.newlyDisclosed.length) :
    MissingEdgePotential after + 2 ≤ MissingEdgePotential before := by
  unfold MissingEdgePotential
  rw [step.splitRemaining]
  simp only [List.length_append]
  omega

theorem inspect_run_rounds_are_bounded_by_initial_missing_edges
    {startState finalState : InspectState}
    {steps : Nat}
    (run : InspectRun startState finalState steps) :
    steps ≤ MissingEdgePotential startState := by
  induction run with
  | nil =>
      exact Nat.zero_le _
  | cons step tail inductionHypothesis =>
      have decreases :=
        inspect_step_strictly_decreases_missing_potential step
      omega

theorem inspect_run_conserves_token_plus_missing_units
    {startState finalState : InspectState}
    {steps : Nat}
    (run : InspectRun startState finalState steps) :
    finalState.disclosedTokenUnits + finalState.remainingEdges.length =
      startState.disclosedTokenUnits + startState.remainingEdges.length := by
  induction run with
  | nil =>
      rfl
  | cons step tail inductionHypothesis =>
      exact inductionHypothesis.trans
        (inspect_step_conserves_token_plus_missing_units step)

theorem inspect_run_preserves_partition
    {startState finalState : InspectState}
    {steps : Nat}
    (startPartitioned : Partitioned startState)
    (run : InspectRun startState finalState steps) :
    Partitioned finalState := by
  induction run with
  | nil =>
      exact startPartitioned
  | cons step tail inductionHypothesis =>
      exact inductionHypothesis
        (inspect_step_preserves_partition startPartitioned step)

theorem inspect_run_accounts_for_every_llm_round
    {startState finalState : InspectState}
    {steps : Nat}
    (run : InspectRun startState finalState steps) :
    finalState.llmRounds = startState.llmRounds + steps := by
  induction run with
  | nil =>
      simp
  | cons step tail inductionHypothesis =>
      rw [inductionHypothesis, step.roundAdvance]
      omega

theorem complete_partition_discloses_exact_required_edges
    {state : InspectState}
    (partitioned : Partitioned state)
    (complete : Complete state) :
    state.disclosedEdges = state.request.requiredEdges := by
  unfold Partitioned at partitioned
  unfold Complete at complete
  rw [complete] at partitioned
  simpa using partitioned

theorem decision_ready_requires_verified_chain
    {chainVerified : Prop}
    {state : InspectState}
    (ready : DecisionReady chainVerified state) :
    chainVerified :=
  ready.1

theorem decision_ready_discloses_exact_required_edges
    {chainVerified : Prop}
    {state : InspectState}
    (ready : DecisionReady chainVerified state) :
    state.disclosedEdges = state.request.requiredEdges :=
  complete_partition_discloses_exact_required_edges
    ready.2.1 ready.2.2

end ASPProof.SearchRouteBoundedAdaptiveInspectConvergence
