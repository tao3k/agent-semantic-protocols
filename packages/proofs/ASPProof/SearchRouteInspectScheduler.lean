-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteBranchBound

namespace SearchRouteInspectScheduler

open SearchRouteCost
open SearchRouteDAG
open SearchRouteDAGEnumeration
open SearchRouteBranchBound

structure PartialRoute where
  graphHopsLowerBound : Nat
  steps : List RouteStep
  deriving DecidableEq, Repr

def partialUncachedTokens (prefixRoute : PartialRoute) :
    Nat :=
  prefixRoute.steps.foldl
    (fun total step =>
      total + step.evidenceTokens +
        (step.promptTokens - step.cachedPromptTokens))
    0

def partialRounds (prefixRoute : PartialRoute) :
    Nat :=
  prefixRoute.steps.foldl
    (fun total step => total + step.roundCost)
    0

def partialLowerBound (prefixRoute : PartialRoute) :
    GraphLowerBound :=
  { graphHops := prefixRoute.graphHopsLowerBound
    uncachedTokens := partialUncachedTokens prefixRoute
    rounds := partialRounds prefixRoute
    transitions := prefixRoute.steps.length }

def ExtendsByOne
    (current next : PartialRoute)
    (step : RouteStep) :
    Prop :=
  current.graphHopsLowerBound ≤ next.graphHopsLowerBound ∧
    next.steps = current.steps ++ [step]

theorem partial_uncached_tokens_extend
    {current next : PartialRoute}
    {step : RouteStep}
    (extensionReceipt : ExtendsByOne current next step) :
    partialUncachedTokens next =
      partialUncachedTokens current +
        step.evidenceTokens +
        (step.promptTokens - step.cachedPromptTokens) := by
  simp only [partialUncachedTokens]
  rw [extensionReceipt.2]
  simp only [
    List.foldl_append,
    List.foldl_cons,
    List.foldl_nil
  ]

theorem partial_rounds_extend
    {current next : PartialRoute}
    {step : RouteStep}
    (extensionReceipt : ExtendsByOne current next step) :
    partialRounds next =
      partialRounds current + step.roundCost := by
  simp only [partialRounds]
  rw [extensionReceipt.2]
  simp only [
    List.foldl_append,
    List.foldl_cons,
    List.foldl_nil
  ]

theorem partial_lower_bound_is_monotone
    {current next : PartialRoute}
    {step : RouteStep}
    (extensionReceipt : ExtendsByOne current next step) :
    keyLexNoWorse
      (partialLowerBound current)
      (partialLowerBound next) := by
  change
    current.graphHopsLowerBound < next.graphHopsLowerBound ∨
      (current.graphHopsLowerBound =
          next.graphHopsLowerBound ∧
        (partialUncachedTokens current <
            partialUncachedTokens next ∨
          (partialUncachedTokens current =
              partialUncachedTokens next ∧
            (partialRounds current < partialRounds next ∨
              (partialRounds current = partialRounds next ∧
                current.steps.length ≤ next.steps.length)))))
  have tokenProgress :=
    partial_uncached_tokens_extend extensionReceipt
  have roundProgress :=
    partial_rounds_extend extensionReceipt
  have graphProgress := extensionReceipt.1
  have transitionProgress :
      current.steps.length + 1 = next.steps.length := by
    rw [extensionReceipt.2]
    simp
  omega

structure CertifiedPartialFrontier where
  prefixRoute : PartialRoute
  candidates : List GraphCandidate
  boundValid :
    ∀ candidate ∈ candidates,
      keyLexNoWorse
        (partialLowerBound prefixRoute)
        (candidateKey candidate)

def CertifiedPartialFrontier.toCertifiedFrontier
    (frontier : CertifiedPartialFrontier) :
    CertifiedFrontier :=
  { candidates := frontier.candidates
    lowerBound := partialLowerBound frontier.prefixRoute
    valid := frontier.boundValid }

def keyLexNoWorseB
    (left right : GraphLowerBound) :
    Bool :=
  left.graphHops < right.graphHops ||
    (left.graphHops == right.graphHops &&
      (left.uncachedTokens < right.uncachedTokens ||
        (left.uncachedTokens == right.uncachedTokens &&
          (left.rounds < right.rounds ||
            (left.rounds == right.rounds &&
              left.transitions ≤ right.transitions)))))

theorem keyLexNoWorseB_true_iff
    (left right : GraphLowerBound) :
    keyLexNoWorseB left right = true ↔
      keyLexNoWorse left right := by
  simp [keyLexNoWorseB, keyLexNoWorse]

theorem keyLexNoWorseB_false_iff
    (left right : GraphLowerBound) :
    keyLexNoWorseB left right = false ↔
      ¬keyLexNoWorse left right := by
  rw [Bool.eq_false_iff]
  exact not_congr (keyLexNoWorseB_true_iff left right)

def shouldPruneB
    (selected : GraphCandidate)
    (frontier : CertifiedPartialFrontier) :
    Bool :=
  keyLexNoWorseB
    (candidateKey selected)
    (partialLowerBound frontier.prefixRoute)

structure InspectSchedule where
  pruned : List CertifiedPartialFrontier
  inspect : List CertifiedPartialFrontier

def scheduleInspect
    (selected : GraphCandidate) :
    List CertifiedPartialFrontier → InspectSchedule
  | [] =>
      { pruned := []
        inspect := [] }
  | frontier :: rest =>
      let scheduledRest := scheduleInspect selected rest
      if shouldPruneB selected frontier then
        { scheduledRest with
          pruned := frontier :: scheduledRest.pruned }
      else
        { scheduledRest with
          inspect := frontier :: scheduledRest.inspect }

theorem scheduleInspect_covers
    (selected : GraphCandidate)
    (frontiers : List CertifiedPartialFrontier) :
    ∀ frontier ∈ frontiers,
      frontier ∈ (scheduleInspect selected frontiers).pruned ∨
        frontier ∈ (scheduleInspect selected frontiers).inspect := by
  induction frontiers with
  | nil =>
      simp
  | cons head tail inductionHypothesis =>
      intro frontier frontierMember
      simp at frontierMember
      rcases frontierMember with frontierIsHead | frontierInTail
      · subst frontier
        cases pruningDecision : shouldPruneB selected head <;>
          simp [scheduleInspect, pruningDecision]
      · have coveredByTail :=
          inductionHypothesis frontier frontierInTail
        cases pruningDecision : shouldPruneB selected head with
        | false =>
            rcases coveredByTail with prunedByTail | inspectByTail
            · exact Or.inl (by
                simpa [scheduleInspect, pruningDecision] using prunedByTail)
            · exact Or.inr (by
                simp [scheduleInspect, pruningDecision, inspectByTail])
        | true =>
            rcases coveredByTail with prunedByTail | inspectByTail
            · exact Or.inl (by
                simp [scheduleInspect, pruningDecision, prunedByTail])
            · exact Or.inr (by
                simpa [scheduleInspect, pruningDecision] using inspectByTail)

theorem scheduled_pruning_is_safe
    {selected : GraphCandidate}
    {frontiers : List CertifiedPartialFrontier}
    {frontier : CertifiedPartialFrontier}
    (scheduled :
      frontier ∈ (scheduleInspect selected frontiers).pruned) :
    ∀ candidate ∈ frontier.candidates,
      graphLexNoWorse selected candidate := by
  induction frontiers with
  | nil =>
      simp [scheduleInspect] at scheduled
  | cons head tail inductionHypothesis =>
      cases pruningDecision : shouldPruneB selected head with
      | true =>
        simp [scheduleInspect, pruningDecision] at scheduled
        rcases scheduled with frontierIsHead | scheduledInTail
        · subst frontier
          apply
            valid_lower_bound_pruning_is_safe
              (frontier := head.toCertifiedFrontier)
          exact
            (keyLexNoWorseB_true_iff
              (candidateKey selected)
              (partialLowerBound head.prefixRoute)).1 pruningDecision
        · exact inductionHypothesis scheduledInTail
      | false =>
        simp [scheduleInspect, pruningDecision] at scheduled
        exact inductionHypothesis scheduled

theorem scheduled_pruned_is_prunable
    {selected : GraphCandidate}
    {frontiers : List CertifiedPartialFrontier}
    {frontier : CertifiedPartialFrontier}
    (scheduled :
      frontier ∈ (scheduleInspect selected frontiers).pruned) :
    Prunable
      selected
      (partialLowerBound frontier.prefixRoute) := by
  induction frontiers with
  | nil =>
      simp [scheduleInspect] at scheduled
  | cons head tail inductionHypothesis =>
      cases pruningDecision : shouldPruneB selected head with
      | true =>
        simp [scheduleInspect, pruningDecision] at scheduled
        rcases scheduled with frontierIsHead | scheduledInTail
        · subst frontier
          exact
            (keyLexNoWorseB_true_iff
              (candidateKey selected)
              (partialLowerBound head.prefixRoute)).1 pruningDecision
        · exact inductionHypothesis scheduledInTail
      | false =>
        simp [scheduleInspect, pruningDecision] at scheduled
        exact inductionHypothesis scheduled

theorem scheduled_inspection_is_not_prunable
    {selected : GraphCandidate}
    {frontiers : List CertifiedPartialFrontier}
    {frontier : CertifiedPartialFrontier}
    (scheduled :
      frontier ∈ (scheduleInspect selected frontiers).inspect) :
    ¬Prunable
      selected
      (partialLowerBound frontier.prefixRoute) := by
  induction frontiers with
  | nil =>
      simp [scheduleInspect] at scheduled
  | cons head tail inductionHypothesis =>
      cases pruningDecision : shouldPruneB selected head with
      | true =>
        simp [scheduleInspect, pruningDecision] at scheduled
        exact inductionHypothesis scheduled
      | false =>
        simp [scheduleInspect, pruningDecision] at scheduled
        rcases scheduled with frontierIsHead | scheduledInTail
        · subst frontier
          exact
            (keyLexNoWorseB_false_iff
              (candidateKey selected)
              (partialLowerBound head.prefixRoute)).1 pruningDecision
        · exact inductionHypothesis scheduledInTail

def ScheduleComplete
    (selected : GraphCandidate)
    (frontiers : List CertifiedPartialFrontier) :
    Prop :=
  (scheduleInspect selected frontiers).inspect = []

theorem completed_schedule_prunes_every_frontier
    {selected : GraphCandidate}
    {frontiers : List CertifiedPartialFrontier}
    (complete : ScheduleComplete selected frontiers) :
    ∀ frontier ∈ frontiers,
      frontier ∈ (scheduleInspect selected frontiers).pruned := by
  intro frontier frontierMember
  rcases scheduleInspect_covers selected frontiers frontier frontierMember with
    pruned | inspect
  · exact pruned
  · have noInspection := complete
    unfold ScheduleComplete at noInspection
    rw [noInspection] at inspect
    simp at inspect

theorem completed_inspect_schedule_is_globally_optimal
    {selected : GraphCandidate}
    {candidateUniverse : CandidateUniverse}
    {visible : List GraphCandidate}
    {frontiers : List CertifiedPartialFrontier}
    (visibleOptimal : VisibleOptimal selected visible)
    (receipt : CoverageReceipt candidateUniverse visible)
    (receiptFrontiers :
      receipt.deferred =
        frontiers.map
          CertifiedPartialFrontier.toCertifiedFrontier)
    (complete : ScheduleComplete selected frontiers) :
    ∀ candidate ∈ candidateUniverse.candidates,
      graphLexNoWorse selected candidate := by
  apply
    certified_lazy_frontier_is_globally_optimal
      visibleOptimal
      receipt
  intro certified certifiedMember
  rw [receiptFrontiers] at certifiedMember
  rcases List.mem_map.mp certifiedMember with
    ⟨frontier, frontierMember, frontierIdentity⟩
  subst certified
  exact
    scheduled_pruned_is_prunable
      (completed_schedule_prunes_every_frontier
        complete
        frontier
        frontierMember)

def decreasingGraphPrefix : PartialRoute :=
  { graphHopsLowerBound := 5
    steps := [] }

def decreasingGraphExtension : PartialRoute :=
  { graphHopsLowerBound := 2
    steps := [zeroCostInspectStep] }

theorem decreasing_graph_bound_breaks_extension_monotonicity :
    decreasingGraphExtension.steps =
        decreasingGraphPrefix.steps ++ [zeroCostInspectStep] ∧
      ¬keyLexNoWorse
        (partialLowerBound decreasingGraphPrefix)
        (partialLowerBound decreasingGraphExtension) := by
  constructor
  · rfl
  · simp [
      keyLexNoWorse,
      partialLowerBound,
      partialUncachedTokens,
      partialRounds,
      decreasingGraphPrefix,
      decreasingGraphExtension,
      zeroCostInspectStep
    ]

end SearchRouteInspectScheduler
