-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteInspectScheduler

open SearchRouteCost
open SearchRouteDAG
open SearchRouteBranchBound
open SearchRouteInspectScheduler

namespace SearchRouteInspectLoop

/-!
The repeated inspect loop is deliberately smaller than a general search
interpreter.  It records only the information needed to prove that an agent
cannot keep materializing and rescheduling the same frontier forever.
-/

structure WorkFrontier where
  frontier : CertifiedPartialFrontier
  workCredit : Nat

structure InspectLoopState where
  selected : GraphCandidate
  pending : List WorkFrontier

def loopWork (state : InspectLoopState) : Nat :=
  (state.pending.map WorkFrontier.workCredit).sum

def LoopWellFormed (state : InspectLoopState) : Prop :=
  ∀ workFrontier ∈ state.pending, 0 < workFrontier.workCredit

def PendingContains (candidate : GraphCandidate) (pending : List WorkFrontier) : Prop :=
  ∃ workFrontier ∈ pending, candidate ∈ workFrontier.frontier.candidates

def LoopCovers (candidateUniverse : List GraphCandidate) (state : InspectLoopState) : Prop :=
  ∀ candidate ∈ candidateUniverse,
    graphLexNoWorse state.selected candidate ∨ PendingContains candidate state.pending

def RefinementCovers
    (frontier : CertifiedPartialFrontier)
    (newSelected : GraphCandidate)
    (children : List WorkFrontier) : Prop :=
  ∀ candidate ∈ frontier.candidates,
    graphLexNoWorse newSelected candidate ∨ PendingContains candidate children

inductive LoopDecision where
  | prune
  | inspect (newSelected : GraphCandidate) (children : List WorkFrontier)

def applyDecision (state : InspectLoopState) : LoopDecision → InspectLoopState
  | .prune =>
      match state.pending with
      | [] => state
      | _ :: rest => { state with pending := rest }
  | .inspect newSelected children =>
      match state.pending with
      | [] => state
      | _ :: rest => { selected := newSelected, pending := children ++ rest }

def DecisionValid (state : InspectLoopState) : LoopDecision → Prop
  | .prune =>
      match state.pending with
      | [] => False
      | head :: _ =>
          0 < head.workCredit ∧
            Prunable state.selected (partialLowerBound head.frontier.prefixRoute)
  | .inspect newSelected children =>
      match state.pending with
      | [] => False
      | head :: _ =>
          ¬Prunable state.selected (partialLowerBound head.frontier.prefixRoute) ∧
            graphLexNoWorse newSelected state.selected ∧
            (∀ child ∈ children, 0 < child.workCredit) ∧
            (children.map WorkFrontier.workCredit).sum < head.workCredit ∧
            RefinementCovers head.frontier newSelected children

theorem graphLexNoWorse_refl (candidate : GraphCandidate) :
    graphLexNoWorse candidate candidate := by
  exact (graphLexNoWorse_iff_key candidate candidate).2 (keyLexNoWorse_refl _)

theorem graphLexNoWorse_trans
    {first second third : GraphCandidate}
    (firstSecond : graphLexNoWorse first second)
    (secondThird : graphLexNoWorse second third) :
    graphLexNoWorse first third := by
  apply (graphLexNoWorse_iff_key first third).2
  exact keyLexNoWorse_trans
    ((graphLexNoWorse_iff_key first second).1 firstSecond)
    ((graphLexNoWorse_iff_key second third).1 secondThird)

theorem valid_decision_decreases_work
    {state : InspectLoopState}
    {decision : LoopDecision}
    (valid : DecisionValid state decision) :
    loopWork (applyDecision state decision) < loopWork state := by
  rcases state with ⟨selected, pending⟩
  cases pending with
  | nil =>
      cases decision <;> simp [DecisionValid] at valid
  | cons head rest =>
      cases decision with
      | prune =>
          simp [DecisionValid] at valid
          simp [loopWork, applyDecision]
          omega
      | inspect newSelected children =>
          simp [DecisionValid] at valid
          simp [loopWork, applyDecision, List.sum_append]
          omega

theorem valid_decision_preserves_well_formed
    {state : InspectLoopState}
    {decision : LoopDecision}
    (wellFormed : LoopWellFormed state)
    (valid : DecisionValid state decision) :
    LoopWellFormed (applyDecision state decision) := by
  rcases state with ⟨selected, pending⟩
  cases pending with
  | nil =>
      cases decision <;> simp [DecisionValid] at valid
  | cons head rest =>
      cases decision with
      | prune =>
          intro workFrontier workFrontierMember
          have restMember : workFrontier ∈ rest := by
            simpa [applyDecision] using workFrontierMember
          exact wellFormed workFrontier (by simp [restMember])
      | inspect newSelected children =>
          intro workFrontier workFrontierMember
          have memberSplit : workFrontier ∈ children ∨ workFrontier ∈ rest := by
            simpa [applyDecision, List.mem_append] using workFrontierMember
          cases memberSplit with
          | inl childMember =>
              exact valid.2.2.1 workFrontier childMember
          | inr restMember =>
              exact wellFormed workFrontier (by simp [restMember])

theorem valid_decision_selected_no_worse
    {state : InspectLoopState}
    {decision : LoopDecision}
    (valid : DecisionValid state decision) :
    graphLexNoWorse (applyDecision state decision).selected state.selected := by
  rcases state with ⟨selected, pending⟩
  cases pending with
  | nil =>
      cases decision <;> simp [DecisionValid] at valid
  | cons head rest =>
      cases decision with
      | prune =>
          simpa [applyDecision] using graphLexNoWorse_refl selected
      | inspect newSelected children =>
          exact valid.2.1

theorem valid_prune_is_safe
    {selected : GraphCandidate}
    {head : WorkFrontier}
    {rest : List WorkFrontier}
    (valid : DecisionValid
      { selected := selected, pending := head :: rest }
      .prune) :
    ∀ candidate ∈ head.frontier.candidates, graphLexNoWorse selected candidate := by
  apply valid_lower_bound_pruning_is_safe
    (frontier := head.frontier.toCertifiedFrontier)
  exact valid.2

theorem valid_decision_preserves_coverage
    {candidateUniverse : List GraphCandidate}
    {state : InspectLoopState}
    {decision : LoopDecision}
    (coverage : LoopCovers candidateUniverse state)
    (valid : DecisionValid state decision) :
    LoopCovers candidateUniverse (applyDecision state decision) := by
  rcases state with ⟨selected, pending⟩
  cases pending with
  | nil =>
      cases decision <;> simp [DecisionValid] at valid
  | cons head rest =>
      intro candidate candidateMember
      rcases coverage candidate candidateMember with selectedDominates | pendingContains
      · left
        exact graphLexNoWorse_trans
          (valid_decision_selected_no_worse valid)
          selectedDominates
      · rcases pendingContains with ⟨workFrontier, workFrontierMember, frontierMember⟩
        have memberSplit : workFrontier = head ∨ workFrontier ∈ rest := by
          simpa using workFrontierMember
        cases decision with
        | prune =>
            cases memberSplit with
            | inl isHead =>
                subst workFrontier
                left
                exact valid_prune_is_safe valid candidate frontierMember
            | inr restMember =>
                right
                exact ⟨workFrontier, by simpa [applyDecision] using restMember, frontierMember⟩
        | inspect newSelected children =>
            cases memberSplit with
            | inl isHead =>
                subst workFrontier
                rcases valid.2.2.2.2 candidate frontierMember with
                  newSelectedDominates | childContains
                · left
                  simpa [applyDecision] using newSelectedDominates
                · right
                  rcases childContains with
                    ⟨child, childMember, childFrontierMember⟩
                  exact ⟨child, by
                    simp [applyDecision, List.mem_append, childMember],
                    childFrontierMember⟩
            | inr restMember =>
                right
                exact ⟨workFrontier, by
                  simp [applyDecision, List.mem_append, restMember], frontierMember⟩

inductive InspectLoopTrace : InspectLoopState → InspectLoopState → Nat → Prop
  | stop (state : InspectLoopState) : InspectLoopTrace state state 0
  | step
      {current finalState : InspectLoopState}
      {steps : Nat}
      (decision : LoopDecision)
      (valid : DecisionValid current decision)
      (rest : InspectLoopTrace (applyDecision current decision) finalState steps) :
      InspectLoopTrace current finalState (steps + 1)

theorem trace_work_bound
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps) :
    loopWork finalState + steps ≤ loopWork initial := by
  induction trace with
  | stop state =>
      simp
  | step decision valid rest inductionHypothesis =>
      have decreases := valid_decision_decreases_work valid
      omega

theorem trace_length_bound
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps) :
    steps ≤ loopWork initial := by
  have workBound := trace_work_bound trace
  omega

theorem trace_preserves_well_formed
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps)
    (wellFormed : LoopWellFormed initial) :
    LoopWellFormed finalState := by
  induction trace with
  | stop state =>
      exact wellFormed
  | step decision valid rest inductionHypothesis =>
      exact inductionHypothesis (valid_decision_preserves_well_formed wellFormed valid)

theorem trace_preserves_coverage
    {candidateUniverse : List GraphCandidate}
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps)
    (coverage : LoopCovers candidateUniverse initial) :
    LoopCovers candidateUniverse finalState := by
  induction trace with
  | stop state =>
      exact coverage
  | step decision valid rest inductionHypothesis =>
      exact inductionHypothesis (valid_decision_preserves_coverage coverage valid)

def LoopClosed (state : InspectLoopState) : Prop :=
  state.pending = []

theorem zero_work_well_formed_is_closed
    {state : InspectLoopState}
    (wellFormed : LoopWellFormed state)
    (zeroWork : loopWork state = 0) :
    LoopClosed state := by
  rcases state with ⟨selected, pending⟩
  cases pending with
  | nil =>
      rfl
  | cons head rest =>
      have positiveCredit : 0 < head.workCredit :=
        wellFormed head (by simp)
      simp [loopWork] at zeroWork
      omega

theorem closed_coverage_is_globally_optimal
    {candidateUniverse : List GraphCandidate}
    {state : InspectLoopState}
    (closed : LoopClosed state)
    (coverage : LoopCovers candidateUniverse state) :
    ∀ candidate ∈ candidateUniverse, graphLexNoWorse state.selected candidate := by
  intro candidate candidateMember
  rcases coverage candidate candidateMember with selectedDominates | pendingContains
  · exact selectedDominates
  · rcases pendingContains with ⟨workFrontier, workFrontierMember, _⟩
    simp [LoopClosed] at closed
    simp [closed] at workFrontierMember

theorem zero_work_trace_closes_and_is_globally_optimal
    {candidateUniverse : List GraphCandidate}
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps)
    (wellFormed : LoopWellFormed initial)
    (coverage : LoopCovers candidateUniverse initial)
    (finalZero : loopWork finalState = 0) :
    LoopClosed finalState ∧
      ∀ candidate ∈ candidateUniverse,
        graphLexNoWorse finalState.selected candidate := by
  have finalWellFormed := trace_preserves_well_formed trace wellFormed
  have finalClosed := zero_work_well_formed_is_closed finalWellFormed finalZero
  exact ⟨
    finalClosed,
    closed_coverage_is_globally_optimal
      finalClosed
      (trace_preserves_coverage trace coverage)
  ⟩

theorem full_budget_trace_closes_and_is_globally_optimal
    {candidateUniverse : List GraphCandidate}
    {initial finalState : InspectLoopState}
    {steps : Nat}
    (trace : InspectLoopTrace initial finalState steps)
    (wellFormed : LoopWellFormed initial)
    (coverage : LoopCovers candidateUniverse initial)
    (usesFullBudget : steps = loopWork initial) :
    LoopClosed finalState ∧
      ∀ candidate ∈ candidateUniverse,
        graphLexNoWorse finalState.selected candidate := by
  have workBound := trace_work_bound trace
  have finalZero : loopWork finalState = 0 := by
    omega
  exact zero_work_trace_closes_and_is_globally_optimal
    trace wellFormed coverage finalZero

def stutterWorkFrontier (frontier : CertifiedPartialFrontier) : WorkFrontier :=
  { frontier := frontier, workCredit := 1 }

def stutterState
    (selected : GraphCandidate)
    (frontier : CertifiedPartialFrontier) : InspectLoopState :=
  { selected := selected, pending := [stutterWorkFrontier frontier] }

def stutterDecision
    (selected : GraphCandidate)
    (frontier : CertifiedPartialFrontier) : LoopDecision :=
  .inspect selected [stutterWorkFrontier frontier]

def NonStrictCreditValid (head : WorkFrontier) (children : List WorkFrontier) : Prop :=
  (children.map WorkFrontier.workCredit).sum ≤ head.workCredit

theorem non_strict_credit_allows_stutter
    (selected : GraphCandidate)
    (frontier : CertifiedPartialFrontier) :
    NonStrictCreditValid
        (stutterWorkFrontier frontier)
        [stutterWorkFrontier frontier] ∧
      applyDecision
          (stutterState selected frontier)
          (stutterDecision selected frontier) =
        stutterState selected frontier ∧
      loopWork
          (applyDecision
            (stutterState selected frontier)
            (stutterDecision selected frontier)) =
        loopWork (stutterState selected frontier) := by
  constructor
  · simp [NonStrictCreditValid, stutterWorkFrontier]
  · constructor <;> rfl

end SearchRouteInspectLoop
