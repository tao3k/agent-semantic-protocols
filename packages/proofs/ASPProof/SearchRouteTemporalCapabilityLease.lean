-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteEvidenceGraphRouter

namespace SearchRouteTemporalCapabilityLease

open SearchRouteEvidenceGraphRouter

structure GenerationIdentity where
  evidenceRootDigest : Nat
  providerDigest : Nat
  queryDigest : Nat
  sourceIndexGeneration : Nat
  runtimeGeneration : Nat
  projectionVersion : Nat
  deriving DecidableEq, Repr

structure RuntimeObservation where
  now : Nat
  generation : GenerationIdentity
  deriving DecidableEq, Repr

structure CapabilityLease where
  issuedAt : Nat
  expiresAt : Nat
  generation : GenerationIdentity
  routeDigest : Nat
  deriving DecidableEq, Repr

def LeaseBound
    (observation : RuntimeObservation)
    (lease : CapabilityLease)
    (candidate : ExecutableGraphCandidate) : Prop :=
  lease.generation = observation.generation ∧
  lease.routeDigest = candidate.routeDigest

def LeaseFresh
    (observation : RuntimeObservation)
    (lease : CapabilityLease) : Prop :=
  lease.issuedAt ≤ observation.now ∧
  observation.now ≤ lease.expiresAt

def LeaseUsable
    (observation : RuntimeObservation)
    (lease : CapabilityLease)
    (candidate : ExecutableGraphCandidate) : Prop :=
  LeaseBound observation lease candidate ∧
  LeaseFresh observation lease ∧
  RuntimeReady candidate

theorem usable_lease_is_bound
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (usable : LeaseUsable observation lease candidate) :
    LeaseBound observation lease candidate :=
  usable.1

theorem usable_lease_is_fresh
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (usable : LeaseUsable observation lease candidate) :
    LeaseFresh observation lease :=
  usable.2.1

theorem expired_lease_is_not_usable
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (expired : lease.expiresAt < observation.now) :
    ¬ LeaseUsable observation lease candidate := by
  intro usable
  exact (Nat.not_lt_of_ge usable.2.1.2) expired

theorem future_lease_is_not_usable
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (notYetValid : observation.now < lease.issuedAt) :
    ¬ LeaseUsable observation lease candidate := by
  intro usable
  exact (Nat.not_lt_of_ge usable.2.1.1) notYetValid

theorem source_index_generation_drift_rejects_lease
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (drift :
      lease.generation.sourceIndexGeneration ≠
        observation.generation.sourceIndexGeneration) :
    ¬ LeaseUsable observation lease candidate := by
  intro usable
  exact drift (congrArg GenerationIdentity.sourceIndexGeneration usable.1.1)

theorem runtime_generation_drift_rejects_lease
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (drift :
      lease.generation.runtimeGeneration ≠
        observation.generation.runtimeGeneration) :
    ¬ LeaseUsable observation lease candidate := by
  intro usable
  exact drift (congrArg GenerationIdentity.runtimeGeneration usable.1.1)

theorem projection_version_drift_rejects_lease
    {observation : RuntimeObservation}
    {lease : CapabilityLease}
    {candidate : ExecutableGraphCandidate}
    (drift :
      lease.generation.projectionVersion ≠
        observation.generation.projectionVersion) :
    ¬ LeaseUsable observation lease candidate := by
  intro usable
  exact drift (congrArg GenerationIdentity.projectionVersion usable.1.1)

theorem runtime_ready_does_not_imply_lease_usable
    (candidate : ExecutableGraphCandidate)
    (ready : RuntimeReady candidate) :
    ∃ (observation : RuntimeObservation) (lease : CapabilityLease),
      RuntimeReady candidate ∧
      ¬ LeaseUsable observation lease candidate := by
  let generation : GenerationIdentity := {
    evidenceRootDigest := candidate.evidenceRootDigest
    providerDigest := candidate.providerDigest
    queryDigest := candidate.queryDigest
    sourceIndexGeneration := 0
    runtimeGeneration := 0
    projectionVersion := 0
  }
  let observation : RuntimeObservation := {
    now := 2
    generation := generation
  }
  let lease : CapabilityLease := {
    issuedAt := 0
    expiresAt := 1
    generation := generation
    routeDigest := candidate.routeDigest
  }
  refine ⟨observation, lease, ready, ?_⟩
  apply expired_lease_is_not_usable
  change 1 < 2
  decide

structure RecoveryState where
  attemptsRemaining : Nat
  cumulativeTokens : Nat
  cumulativeRounds : Nat
  capabilityState : CapabilityState
  deriving DecidableEq, Repr

def RecoveryProgress
    (before after : RecoveryState) : Prop :=
  after.attemptsRemaining < before.attemptsRemaining

structure ChargedRecoveryStep where
  before : RecoveryState
  after : RecoveryState
  tokenCharge : Nat
  roundCharge : Nat
  attemptsDecrease : RecoveryProgress before after
  roundChargePositive : 0 < roundCharge
  tokenAccounting :
    after.cumulativeTokens =
      before.cumulativeTokens + tokenCharge
  roundAccounting :
    after.cumulativeRounds =
      before.cumulativeRounds + roundCharge

inductive RecoveryRun : RecoveryState → RecoveryState → Nat → Prop where
  | done (state : RecoveryState) :
      RecoveryRun state state 0
  | step
      {before after terminal : RecoveryState}
      {steps : Nat}
      (transition : ChargedRecoveryStep)
      (startsAt : transition.before = before)
      (continuesAt : transition.after = after)
      (tail : RecoveryRun after terminal steps) :
      RecoveryRun before terminal (steps + 1)

theorem recovery_run_length_is_bounded
    {initial terminal : RecoveryState}
    {steps : Nat}
    (run : RecoveryRun initial terminal steps) :
    steps ≤ initial.attemptsRemaining := by
  induction run with
  | done =>
      exact Nat.zero_le _
  | step transition startsAt continuesAt tail inductionHypothesis =>
      cases startsAt
      cases continuesAt
      exact Nat.succ_le_of_lt
        (Nat.lt_of_le_of_lt
          inductionHypothesis
          transition.attemptsDecrease)

theorem zero_budget_has_no_recovery_progress
    {state next : RecoveryState}
    (empty : state.attemptsRemaining = 0) :
    ¬ RecoveryProgress state next := by
  intro progress
  simp [RecoveryProgress, empty] at progress

theorem recoverable_self_loop_has_no_progress
    {state : RecoveryState}
    (_recoverable : state.capabilityState = .recoverable) :
    ¬ RecoveryProgress state state := by
  intro progress
  exact (Nat.lt_irrefl state.attemptsRemaining) progress

end SearchRouteTemporalCapabilityLease
