-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

structure RetryState where
  roundNumber : Nat
  attemptsUsed : Nat
  maximumAttempts : Nat

def RetryAuthorized (state : RetryState) : Prop :=
  state.attemptsUsed < state.maximumAttempts

def nextRetry (state : RetryState) : RetryState := {
  roundNumber := state.roundNumber + 1
  attemptsUsed := state.attemptsUsed + 1
  maximumAttempts := state.maximumAttempts
}

def RetryRoundFence
    (expectedRound currentRound
      expectedLeaseEpoch currentLeaseEpoch
      expectedRevision currentRevision : Nat) : Prop :=
  expectedRound = currentRound ∧
  expectedLeaseEpoch = currentLeaseEpoch ∧
  expectedRevision = currentRevision

def TotalBackoff
    (delay : Nat → Nat) : Nat → Nat
  | 0 => 0
  | Nat.succ attempts =>
      TotalBackoff delay attempts + delay attempts

def BoundedBackoff
    (delay : Nat → Nat)
    (maximumDelay : Nat) : Prop :=
  ∀ round, delay round ≤ maximumDelay

structure RetryCommandKey
    (TransitionId CandidateId SnapshotId PolicyId : Type) where
  transitionId : TransitionId
  candidateId : CandidateId
  roundNumber : Nat
  attemptNumber : Nat
  maximumAttempts : Nat
  expectedLeaseEpoch : Nat
  expectedPublicationRevision : Nat
  verificationSnapshotId : SnapshotId
  backoffPolicyVersion : PolicyId

def RetryCommandCompatible
    {TransitionId CandidateId SnapshotId PolicyId : Type}
    (left right :
      RetryCommandKey
        TransitionId CandidateId SnapshotId PolicyId) : Prop :=
  left.transitionId = right.transitionId ∧
  left.candidateId = right.candidateId ∧
  left.roundNumber = right.roundNumber ∧
  left.attemptNumber = right.attemptNumber ∧
  left.maximumAttempts = right.maximumAttempts ∧
  left.expectedLeaseEpoch = right.expectedLeaseEpoch ∧
  left.expectedPublicationRevision =
      right.expectedPublicationRevision ∧
  left.verificationSnapshotId =
      right.verificationSnapshotId ∧
  left.backoffPolicyVersion =
      right.backoffPolicyVersion

theorem next_retry_increments_round_and_attempt
    (state : RetryState) :
    (nextRetry state).roundNumber =
        state.roundNumber + 1 ∧
      (nextRetry state).attemptsUsed =
        state.attemptsUsed + 1 :=
  ⟨rfl, rfl⟩

theorem authorized_retry_stays_within_attempt_budget
    (state : RetryState)
    (authorized : RetryAuthorized state) :
    (nextRetry state).attemptsUsed ≤
      state.maximumAttempts := by
  exact Nat.succ_le_of_lt authorized

theorem exhausted_attempt_budget_blocks_retry
    (state : RetryState)
    (exhausted :
      state.maximumAttempts ≤ state.attemptsUsed) :
    ¬ RetryAuthorized state := by
  intro authorized
  exact (Nat.not_lt_of_ge exhausted) authorized

theorem previous_round_is_fenced_after_retry
    (state : RetryState) :
    state.roundNumber ≠
      (nextRetry state).roundNumber := by
  change state.roundNumber ≠ state.roundNumber + 1
  rw [Nat.add_one]
  exact Nat.ne_of_lt (Nat.lt_succ_self state.roundNumber)

theorem stale_round_blocks_retry_fence
    (expectedRound currentRound
      expectedLeaseEpoch currentLeaseEpoch
      expectedRevision currentRevision : Nat)
    (roundChanged : expectedRound ≠ currentRound) :
    ¬ RetryRoundFence
        expectedRound currentRound
        expectedLeaseEpoch currentLeaseEpoch
        expectedRevision currentRevision := by
  intro fenced
  exact roundChanged fenced.1

theorem total_backoff_is_bounded
    (delay : Nat → Nat)
    (maximumDelay attempts : Nat)
    (bounded : BoundedBackoff delay maximumDelay) :
    TotalBackoff delay attempts ≤
      attempts * maximumDelay := by
  induction attempts with
  | zero =>
      simpa only [TotalBackoff, Nat.zero_mul] using
        (Nat.le_refl 0)
  | succ attempts inductionHypothesis =>
      change
        TotalBackoff delay attempts + delay attempts ≤
          Nat.succ attempts * maximumDelay
      rw [Nat.succ_mul]
      exact Nat.add_le_add
        inductionHypothesis
        (bounded attempts)

theorem adversarial_scheduler_can_starve_a_candidate :
    ∃ schedule : Nat → Bool,
      ∀ round, schedule round ≠ true := by
  let schedule := fun _ : Nat => false
  refine ⟨schedule, ?_⟩
  intro round scheduled
  cases scheduled

theorem conditional_bounded_recovery_liveness
    {Candidate : Type}
    (candidate : Candidate)
    (maximumRounds : Nat)
    (scheduled eligible selected committed :
      Candidate → Nat → Prop)
    (fairnessWitness :
      ∃ round,
        round < maximumRounds ∧
        scheduled candidate round ∧
        eligible candidate round ∧
        selected candidate round)
    (commitProgress :
      ∀ round,
        scheduled candidate round →
        eligible candidate round →
        selected candidate round →
        committed candidate round) :
    ∃ round,
      round < maximumRounds ∧
      committed candidate round := by
  rcases fairnessWitness with
    ⟨round, withinBound,
      scheduledAtRound,
      eligibleAtRound,
      selectedAtRound⟩
  exact ⟨
    round,
    withinBound,
    commitProgress
      round
      scheduledAtRound
      eligibleAtRound
      selectedAtRound
  ⟩

theorem retry_command_compatibility_binds_round
    {TransitionId CandidateId SnapshotId PolicyId : Type}
    (left right :
      RetryCommandKey
        TransitionId CandidateId SnapshotId PolicyId)
    (compatible : RetryCommandCompatible left right) :
    left.roundNumber = right.roundNumber :=
  compatible.2.2.1

theorem changed_round_rejects_retry_replay
    {TransitionId CandidateId SnapshotId PolicyId : Type}
    (left right :
      RetryCommandKey
        TransitionId CandidateId SnapshotId PolicyId)
    (roundChanged :
      left.roundNumber ≠ right.roundNumber) :
    ¬ RetryCommandCompatible left right := by
  intro compatible
  exact
    roundChanged
      (retry_command_compatibility_binds_round
        left right compatible)

theorem same_candidate_does_not_imply_retry_compatibility
    {TransitionId CandidateId SnapshotId PolicyId : Type}
    (left right :
      RetryCommandKey
        TransitionId CandidateId SnapshotId PolicyId)
    (sameCandidate :
      left.candidateId = right.candidateId)
    (roundChanged :
      left.roundNumber ≠ right.roundNumber) :
    left.candidateId = right.candidateId ∧
      ¬ RetryCommandCompatible left right :=
  ⟨sameCandidate,
    changed_round_rejects_retry_replay
      left right roundChanged⟩

theorem changed_attempt_budget_rejects_retry_replay
    {TransitionId CandidateId SnapshotId PolicyId : Type}
    (left right :
      RetryCommandKey
        TransitionId CandidateId SnapshotId PolicyId)
    (budgetChanged :
      left.maximumAttempts ≠ right.maximumAttempts) :
    ¬ RetryCommandCompatible left right := by
  intro compatible
  exact budgetChanged compatible.2.2.2.2.1

end ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness
