-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection

open ASPProof.SearchRouteAdmissionRetryCacheRejoinReceiptBinding

structure AdmissionClaim (Digest : Type) where
  replicaId : ReplicaId
  attemptEpoch : Nat
  terminalDigest : Digest

def admissionClaimOfReadsChain {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme) :
    AdmissionClaim scheme.Digest where
  replicaId := chain.readsEnabled.content.replicaId
  attemptEpoch := chain.readsEnabled.content.attemptId
  terminalDigest := chain.readsEnabled.digest

theorem supported_chain_claim_matches_root_scope
    {scheme : DigestScheme}
    (chain : ReadsEnabledReceiptChain scheme)
    (supported : SupportsReadsEnabled chain) :
    (admissionClaimOfReadsChain chain).replicaId =
        chain.stateInstalled.content.replicaId ∧
    (admissionClaimOfReadsChain chain).attemptEpoch =
        chain.stateInstalled.content.attemptId := by
  rcases supported_reads_chain_has_single_attempt_and_replica chain supported with
    ⟨stateTokenAttempt, tokenMembershipAttempt, membershipReadsAttempt,
      stateTokenReplica, tokenMembershipReplica, membershipReadsReplica⟩
  constructor
  · exact
      ((stateTokenReplica.trans tokenMembershipReplica).trans
        membershipReadsReplica).symm
  · exact
      ((stateTokenAttempt.trans tokenMembershipAttempt).trans
        membershipReadsAttempt).symm

structure ReplicaAdmissionState (Digest : Type) where
  replicaId : ReplicaId
  currentAttemptEpoch : Nat
  active : Option (AdmissionClaim Digest)

def CanActivate {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest) : Prop :=
  claim.replicaId = state.replicaId ∧
  claim.attemptEpoch = state.currentAttemptEpoch ∧
  state.active = none

def CanObserveExisting {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest) : Prop :=
  state.active = some claim

def activate {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest) :
    ReplicaAdmissionState Digest :=
  { state with active := some claim }

def beginNewAttempt {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (nextEpoch : Nat) :
    ReplicaAdmissionState Digest :=
  { state with currentAttemptEpoch := nextEpoch, active := none }

def CanBeginNewAttempt {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (nextEpoch : Nat) : Prop :=
  state.currentAttemptEpoch < nextEpoch

theorem permitted_new_attempt_advances_epoch
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (nextEpoch : Nat)
    (permitted : CanBeginNewAttempt state nextEpoch) :
    state.currentAttemptEpoch <
      (beginNewAttempt state nextEpoch).currentAttemptEpoch :=
  permitted

theorem non_increasing_epoch_cannot_begin
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (nextEpoch : Nat)
    (nonIncreasing : nextEpoch ≤ state.currentAttemptEpoch) :
    ¬ CanBeginNewAttempt state nextEpoch := by
  intro permitted
  exact (Nat.not_lt_of_ge nonIncreasing) permitted

theorem activatable_claim_is_authority_current
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (activatable : CanActivate state claim) :
    claim.replicaId = state.replicaId ∧
    claim.attemptEpoch = state.currentAttemptEpoch :=
  ⟨activatable.1, activatable.2.1⟩

theorem cross_replica_claim_cannot_activate
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (crossReplica : claim.replicaId ≠ state.replicaId) :
    ¬ CanActivate state claim := by
  intro activatable
  exact crossReplica activatable.1

theorem future_unissued_attempt_cannot_activate
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest)
    (futureEpoch : state.currentAttemptEpoch < claim.attemptEpoch) :
    ¬ CanActivate state claim := by
  intro activatable
  exact (Nat.ne_of_lt futureEpoch) activatable.2.1.symm

theorem newer_attempt_rejects_old_chain_activation
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (oldClaim : AdmissionClaim Digest)
    (nextEpoch : Nat)
    (oldWasCurrent : oldClaim.attemptEpoch = state.currentAttemptEpoch)
    (newer : CanBeginNewAttempt state nextEpoch) :
    ¬ CanActivate (beginNewAttempt state nextEpoch) oldClaim := by
  intro activatable
  have oldEqualsNext : oldClaim.attemptEpoch = nextEpoch :=
    activatable.2.1
  have currentEqualsNext : state.currentAttemptEpoch = nextEpoch :=
    oldWasCurrent.symm.trans oldEqualsNext
  exact (Nat.ne_of_lt newer) currentEqualsNext

theorem activated_chain_cannot_activate_twice
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest) :
    ¬ CanActivate (activate state claim) claim := by
  intro activatable
  simp [CanActivate, activate] at activatable

theorem activated_chain_is_idempotently_observable
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (claim : AdmissionClaim Digest) :
    CanObserveExisting (activate state claim) claim :=
  rfl

theorem conflicting_terminal_digest_is_not_idempotent
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (accepted conflicting : AdmissionClaim Digest)
    (differentTerminal :
      accepted.terminalDigest ≠ conflicting.terminalDigest) :
    ¬ CanObserveExisting (activate state accepted) conflicting := by
  intro observable
  have sameClaim : accepted = conflicting :=
    Option.some.inj observable
  exact differentTerminal
    (congrArg AdmissionClaim.terminalDigest sameClaim)

theorem active_chain_is_unique
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (left right : AdmissionClaim Digest)
    (leftActive : state.active = some left)
    (rightActive : state.active = some right) :
    left = right :=
  Option.some.inj (leftActive.symm.trans rightActive)

theorem newer_attempt_supersedes_old_observation
    {Digest : Type}
    (state : ReplicaAdmissionState Digest)
    (oldClaim : AdmissionClaim Digest)
    (nextEpoch : Nat) :
    ¬ CanObserveExisting (beginNewAttempt state nextEpoch) oldClaim := by
  intro observable
  simp [CanObserveExisting, beginNewAttempt] at observable

end ASPProof.SearchRouteAdmissionRetryCacheRejoinReplayProtection
