-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDAGEnumeration

namespace SearchRouteBranchBound

open SearchRouteCost
open SearchRouteDAG

structure GraphLowerBound where
  graphHops : Nat
  uncachedTokens : Nat
  rounds : Nat
  transitions : Nat
  deriving DecidableEq, Repr

def candidateKey (candidate : GraphCandidate) :
    GraphLowerBound :=
  { graphHops := candidate.graphHops
    uncachedTokens := uncachedTokenCost candidate.route
    rounds := roundCost candidate.route
    transitions := transitionCount candidate.route }

def keyLexNoWorse
    (left right : GraphLowerBound) :
    Prop :=
  left.graphHops < right.graphHops ∨
    (left.graphHops = right.graphHops ∧
      (left.uncachedTokens < right.uncachedTokens ∨
        (left.uncachedTokens = right.uncachedTokens ∧
          (left.rounds < right.rounds ∨
            (left.rounds = right.rounds ∧
              left.transitions ≤ right.transitions)))))

theorem keyLexNoWorse_refl (key : GraphLowerBound) :
    keyLexNoWorse key key := by
  simp [keyLexNoWorse]

theorem keyLexNoWorse_trans
    {first second third : GraphLowerBound}
    (firstSecond : keyLexNoWorse first second)
    (secondThird : keyLexNoWorse second third) :
    keyLexNoWorse first third := by
  unfold keyLexNoWorse at *
  omega

theorem graphLexNoWorse_iff_key
    (left right : GraphCandidate) :
    graphLexNoWorse left right ↔
      keyLexNoWorse (candidateKey left) (candidateKey right) := by
  rfl

def BoundValid
    (candidates : List GraphCandidate)
    (lowerBound : GraphLowerBound) :
    Prop :=
  ∀ candidate ∈ candidates,
    keyLexNoWorse lowerBound (candidateKey candidate)

def Prunable
    (selected : GraphCandidate)
    (lowerBound : GraphLowerBound) :
    Prop :=
  keyLexNoWorse (candidateKey selected) lowerBound

structure CertifiedFrontier where
  candidates : List GraphCandidate
  lowerBound : GraphLowerBound
  valid : BoundValid candidates lowerBound

theorem valid_lower_bound_pruning_is_safe
    {selected : GraphCandidate}
    {frontier : CertifiedFrontier}
    (prunable : Prunable selected frontier.lowerBound) :
    ∀ candidate ∈ frontier.candidates,
      graphLexNoWorse selected candidate := by
  intro candidate candidateMember
  apply (graphLexNoWorse_iff_key selected candidate).2
  exact
    keyLexNoWorse_trans
      prunable
      (frontier.valid candidate candidateMember)

def VisibleOptimal
    (selected : GraphCandidate)
    (visible : List GraphCandidate) :
    Prop :=
  selected ∈ visible ∧
    ∀ candidate ∈ visible,
      graphLexNoWorse selected candidate

structure CandidateUniverse where
  snapshotDigest : Nat
  candidates : List GraphCandidate

structure CoverageReceipt
    (candidateUniverse : CandidateUniverse)
    (visible : List GraphCandidate) where
  claimedSnapshotDigest : Nat
  snapshotBound :
    claimedSnapshotDigest = candidateUniverse.snapshotDigest
  deferred : List CertifiedFrontier
  covers :
    ∀ candidate ∈ candidateUniverse.candidates,
      candidate ∈ visible ∨
        ∃ frontier ∈ deferred,
          candidate ∈ frontier.candidates

def DeferredPrunable
    (selected : GraphCandidate)
    {candidateUniverse : CandidateUniverse}
    {visible : List GraphCandidate}
    (receipt : CoverageReceipt candidateUniverse visible) :
    Prop :=
  ∀ frontier ∈ receipt.deferred,
    Prunable selected frontier.lowerBound

theorem certified_lazy_frontier_is_globally_optimal
    {selected : GraphCandidate}
    {candidateUniverse : CandidateUniverse}
    {visible : List GraphCandidate}
    (visibleOptimal : VisibleOptimal selected visible)
    (receipt : CoverageReceipt candidateUniverse visible)
    (deferredPrunable : DeferredPrunable selected receipt) :
    ∀ candidate ∈ candidateUniverse.candidates,
      graphLexNoWorse selected candidate := by
  intro candidate candidateMember
  rcases receipt.covers candidate candidateMember with
    visibleMember | ⟨frontier, frontierMember, candidateInFrontier⟩
  · exact visibleOptimal.2 candidate visibleMember
  · exact
      valid_lower_bound_pruning_is_safe
        (deferredPrunable frontier frontierMember)
        candidate
        candidateInFrontier

def snapshotMatchesB
    {candidateUniverse : CandidateUniverse}
    {visible : List GraphCandidate}
    (receipt : CoverageReceipt candidateUniverse visible)
    (currentSnapshotDigest : Nat) :
    Bool :=
  receipt.claimedSnapshotDigest == currentSnapshotDigest

theorem snapshotMatchesB_true_iff
    {candidateUniverse : CandidateUniverse}
    {visible : List GraphCandidate}
    (receipt : CoverageReceipt candidateUniverse visible)
    (currentSnapshotDigest : Nat) :
    snapshotMatchesB receipt currentSnapshotDigest = true ↔
      candidateUniverse.snapshotDigest = currentSnapshotDigest := by
  simp [snapshotMatchesB, receipt.snapshotBound]

theorem snapshot_accepted_lazy_frontier_is_globally_optimal
    {selected : GraphCandidate}
    {candidateUniverse : CandidateUniverse}
    {visible : List GraphCandidate}
    (receipt : CoverageReceipt candidateUniverse visible)
    (currentSnapshotDigest : Nat)
    (snapshotAccepted :
      snapshotMatchesB receipt currentSnapshotDigest = true)
    (visibleOptimal : VisibleOptimal selected visible)
    (deferredPrunable : DeferredPrunable selected receipt) :
    ∀ candidate ∈ candidateUniverse.candidates,
      graphLexNoWorse selected candidate := by
  have _snapshotIdentity :=
    (snapshotMatchesB_true_iff
      receipt
      currentSnapshotDigest).1 snapshotAccepted
  exact
    certified_lazy_frontier_is_globally_optimal
      visibleOptimal
      receipt
      deferredPrunable

def invalidOptimisticFrontier : GraphLowerBound :=
  { graphHops := 6
    uncachedTokens := 0
    rounds := 0
    transitions := 0 }

theorem invalid_lower_bound_can_prune_a_better_candidate :
    Prunable shortInteractionLongGraph invalidOptimisticFrontier ∧
      ¬BoundValid
        [longInteractionShortGraph]
        invalidOptimisticFrontier ∧
      ¬graphLexNoWorse
        shortInteractionLongGraph
        longInteractionShortGraph := by
  constructor
  · simp [
      Prunable,
      keyLexNoWorse,
      candidateKey,
      invalidOptimisticFrontier,
      shortInteractionLongGraph
    ]
  constructor
  · intro boundValid
    have falseLowerBound :=
      boundValid longInteractionShortGraph (by simp)
    simp [
      keyLexNoWorse,
      candidateKey,
      invalidOptimisticFrontier,
      longInteractionShortGraph
    ] at falseLowerBound
  · simp [
      graphLexNoWorse,
      shortInteractionLongGraph,
      longInteractionShortGraph
    ]

theorem visible_optimality_without_coverage_is_unsound :
    VisibleOptimal
        shortInteractionLongGraph
        [shortInteractionLongGraph] ∧
      ¬(∀ candidate ∈
          [shortInteractionLongGraph, longInteractionShortGraph],
        graphLexNoWorse shortInteractionLongGraph candidate) := by
  constructor
  · constructor
    · simp
    · intro candidate candidateMember
      simp at candidateMember
      subst candidate
      apply (graphLexNoWorse_iff_key _ _).2
      exact keyLexNoWorse_refl _
  · intro claimedGlobal
    have falseOptimality :=
      claimedGlobal longInteractionShortGraph (by simp)
    simp [
      graphLexNoWorse,
      shortInteractionLongGraph,
      longInteractionShortGraph
    ] at falseOptimality

def staleCandidateUniverse : CandidateUniverse :=
  { snapshotDigest := 41
    candidates := [shortInteractionLongGraph] }

def staleCoverageReceipt :
    CoverageReceipt
      staleCandidateUniverse
      [shortInteractionLongGraph] :=
  { claimedSnapshotDigest := 41
    snapshotBound := rfl
    deferred := []
    covers := by
      intro candidate candidateMember
      exact Or.inl candidateMember }

theorem stale_snapshot_coverage_receipt_is_rejected :
    snapshotMatchesB staleCoverageReceipt 42 = false := by
  decide

end SearchRouteBranchBound
