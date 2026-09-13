-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

def DeterministicWinner
    {Candidate : Type}
    (eligible : Candidate → Prop)
    (rank : Candidate → Nat)
    (winner : Candidate) : Prop :=
  eligible winner ∧
  ∀ candidate, eligible candidate →
    rank winner ≤ rank candidate

def InjectiveRank
    {Candidate : Type}
    (rank : Candidate → Nat) : Prop :=
  ∀ left right,
    rank left = rank right → left = right

def ContentionCommitFence
    (expectedLeaseEpoch currentLeaseEpoch
      expectedRevision currentRevision : Nat) : Prop :=
  expectedLeaseEpoch = currentLeaseEpoch ∧
  expectedRevision = currentRevision

structure ContentionRoundKey
    (TransitionId CandidateSetDigest PolicyId SnapshotId : Type) where
  transitionId : TransitionId
  previousLeaseEpoch : Nat
  previousPublicationRevision : Nat
  verificationSnapshotId : SnapshotId
  candidateSetDigest : CandidateSetDigest
  selectionPolicyVersion : PolicyId

def ContentionRoundCompatible
    {TransitionId CandidateSetDigest PolicyId SnapshotId : Type}
    (left right :
      ContentionRoundKey
        TransitionId CandidateSetDigest PolicyId SnapshotId) : Prop :=
  left.transitionId = right.transitionId ∧
  left.previousLeaseEpoch = right.previousLeaseEpoch ∧
  left.previousPublicationRevision =
      right.previousPublicationRevision ∧
  left.verificationSnapshotId =
      right.verificationSnapshotId ∧
  left.candidateSetDigest =
      right.candidateSetDigest ∧
  left.selectionPolicyVersion =
      right.selectionPolicyVersion

structure WinnerCommit (Candidate : Type) where
  winner : Candidate
  previousLeaseEpoch : Nat
  committedLeaseEpoch : Nat
  previousPublicationRevision : Nat
  committedPublicationRevision : Nat
  leaseEpochAdvanced :
    committedLeaseEpoch = previousLeaseEpoch + 1
  publicationRevisionAdvanced :
    committedPublicationRevision =
      previousPublicationRevision + 1

structure LoserRedirect
    (Candidate LeaseId RoundId : Type) where
  roundId : RoundId
  loser : Candidate
  winner : Candidate
  loserNeWinner : loser ≠ winner
  committedLeaseEpoch : Nat
  winnerLeaseId : LeaseId

theorem two_candidate_winner_exists
    {Candidate : Type}
    (rank : Candidate → Nat)
    (left right : Candidate) :
    ∃ winner,
      DeterministicWinner
        (fun candidate =>
          candidate = left ∨ candidate = right)
        rank winner := by
  cases Nat.le_total (rank left) (rank right) with
  | inl leftLeRight =>
      refine ⟨left, Or.inl rfl, ?_⟩
      intro candidate eligible
      rcases eligible with candidateIsLeft | candidateIsRight
      · rw [candidateIsLeft]
        exact Nat.le_refl (rank left)
      · rw [candidateIsRight]
        exact leftLeRight
  | inr rightLeLeft =>
      refine ⟨right, Or.inr rfl, ?_⟩
      intro candidate eligible
      rcases eligible with candidateIsLeft | candidateIsRight
      · rw [candidateIsLeft]
        exact rightLeLeft
      · rw [candidateIsRight]
        exact Nat.le_refl (rank right)

theorem injective_rank_makes_winner_unique
    {Candidate : Type}
    (eligible : Candidate → Prop)
    (rank : Candidate → Nat)
    (rankInjective : InjectiveRank rank)
    (leftWinner rightWinner : Candidate)
    (leftWins :
      DeterministicWinner eligible rank leftWinner)
    (rightWins :
      DeterministicWinner eligible rank rightWinner) :
    leftWinner = rightWinner := by
  apply rankInjective
  exact Nat.le_antisymm
    (leftWins.2 rightWinner rightWins.1)
    (rightWins.2 leftWinner leftWins.1)

theorem noninjective_rank_can_admit_two_distinct_winners :
    ∃ eligible : Bool → Prop,
      ∃ rank : Bool → Nat,
        DeterministicWinner eligible rank false ∧
        DeterministicWinner eligible rank true ∧
        false ≠ true := by
  let eligible := fun _ : Bool => True
  let rank := fun _ : Bool => 0
  refine ⟨eligible, rank, ?_, ?_, by decide⟩
  · exact ⟨True.intro, by
      intro candidate candidateEligible
      exact Nat.le_refl 0⟩
  · exact ⟨True.intro, by
      intro candidate candidateEligible
      exact Nat.le_refl 0⟩

theorem winner_commit_advances_generation
    {Candidate : Type}
    (commit : WinnerCommit Candidate) :
    commit.committedLeaseEpoch =
        commit.previousLeaseEpoch + 1 ∧
      commit.committedPublicationRevision =
        commit.previousPublicationRevision + 1 :=
  ⟨commit.leaseEpochAdvanced,
    commit.publicationRevisionAdvanced⟩

theorem previous_epoch_is_fenced_after_winner_commit
    {Candidate : Type}
    (commit : WinnerCommit Candidate) :
    commit.previousLeaseEpoch ≠
      commit.committedLeaseEpoch := by
  rw [commit.leaseEpochAdvanced, Nat.add_one]
  exact Nat.ne_of_lt
    (Nat.lt_succ_self commit.previousLeaseEpoch)

theorem loser_redirect_names_a_distinct_winner
    {Candidate LeaseId RoundId : Type}
    (redirect : LoserRedirect Candidate LeaseId RoundId) :
    redirect.loser ≠ redirect.winner :=
  redirect.loserNeWinner

theorem contention_round_compatibility_binds_candidate_set
    {TransitionId CandidateSetDigest PolicyId SnapshotId : Type}
    (left right :
      ContentionRoundKey
        TransitionId CandidateSetDigest PolicyId SnapshotId)
    (compatible : ContentionRoundCompatible left right) :
    left.candidateSetDigest =
      right.candidateSetDigest :=
  compatible.2.2.2.2.1

theorem changed_candidate_set_invalidates_contention_round
    {TransitionId CandidateSetDigest PolicyId SnapshotId : Type}
    (left right :
      ContentionRoundKey
        TransitionId CandidateSetDigest PolicyId SnapshotId)
    (candidateSetChanged :
      left.candidateSetDigest ≠
        right.candidateSetDigest) :
    ¬ ContentionRoundCompatible left right := by
  intro compatible
  exact
    candidateSetChanged
      (contention_round_compatibility_binds_candidate_set
        left right compatible)

theorem same_winner_does_not_imply_round_compatibility
    {TransitionId CandidateSetDigest PolicyId SnapshotId Candidate : Type}
    (left right :
      ContentionRoundKey
        TransitionId CandidateSetDigest PolicyId SnapshotId)
    (winner : Candidate)
    (candidateSetChanged :
      left.candidateSetDigest ≠
        right.candidateSetDigest) :
    winner = winner ∧
      ¬ ContentionRoundCompatible left right :=
  ⟨rfl,
    changed_candidate_set_invalidates_contention_round
      left right candidateSetChanged⟩

theorem stale_epoch_blocks_contention_commit
    (expectedLeaseEpoch currentLeaseEpoch
      expectedRevision currentRevision : Nat)
    (epochChanged :
      expectedLeaseEpoch ≠ currentLeaseEpoch) :
    ¬ ContentionCommitFence
        expectedLeaseEpoch currentLeaseEpoch
        expectedRevision currentRevision := by
  intro fenced
  exact epochChanged fenced.1

theorem stale_revision_blocks_contention_commit
    (expectedLeaseEpoch currentLeaseEpoch
      expectedRevision currentRevision : Nat)
    (revisionChanged :
      expectedRevision ≠ currentRevision) :
    ¬ ContentionCommitFence
        expectedLeaseEpoch currentLeaseEpoch
        expectedRevision currentRevision := by
  intro fenced
  exact revisionChanged fenced.2

end ASPProof.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention
