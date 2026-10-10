-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheFencingToken

namespace ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin

open ASPProof.SearchRouteAdmissionRetryCacheFencingToken

universe u

structure RejoinState (Replica : Type u) where
  targetCacheGeneration : Nat
  installedCacheGeneration : Nat
  fenceAuthority : FenceAuthority Replica
  servingToken : ServingToken Replica
  membershipPublished : Bool
  readsEnabled : Bool

structure RejoinMilestones where
  stateInstalledStep : Nat
  tokenIssuedStep : Nat
  membershipPublishedStep : Nat
  readsEnabledStep : Nat
deriving DecidableEq, Repr

def ProperRejoinOrder (milestones : RejoinMilestones) : Prop :=
  milestones.stateInstalledStep < milestones.tokenIssuedStep
    ∧ milestones.tokenIssuedStep < milestones.membershipPublishedStep
    ∧ milestones.membershipPublishedStep < milestones.readsEnabledStep

def SafeRejoinRead
    {Replica : Type u}
    (state : RejoinState Replica)
    (milestones : RejoinMilestones) : Prop :=
  state.targetCacheGeneration ≤ state.installedCacheGeneration
    ∧ CanServe state.fenceAuthority state.servingToken
    ∧ state.membershipPublished = true
    ∧ state.readsEnabled = true
    ∧ ProperRejoinOrder milestones

theorem safe_rejoin_read_carries_state_token_membership_and_order
    {Replica : Type u}
    {state : RejoinState Replica}
    {milestones : RejoinMilestones}
    (safe : SafeRejoinRead state milestones) :
    state.targetCacheGeneration ≤ state.installedCacheGeneration
      ∧ CanServe state.fenceAuthority state.servingToken
      ∧ state.membershipPublished = true
      ∧ state.readsEnabled = true
      ∧ ProperRejoinOrder milestones :=
  safe

def currentRejoinToken : ServingToken Bool :=
  { replica := true
    fenceGeneration := 1 }

def currentRejoinAuthority : FenceAuthority Bool :=
  { currentFenceGeneration := fun _replica => 1
    issued := fun token => token = currentRejoinToken }

def caughtUpRejoinState : RejoinState Bool :=
  { targetCacheGeneration := 1
    installedCacheGeneration := 1
    fenceAuthority := currentRejoinAuthority
    servingToken := currentRejoinToken
    membershipPublished := true
    readsEnabled := true }

def properMilestones : RejoinMilestones :=
  { stateInstalledStep := 0
    tokenIssuedStep := 1
    membershipPublishedStep := 2
    readsEnabledStep := 3 }

theorem caught_up_replica_with_ordered_receipt_may_read :
    SafeRejoinRead caughtUpRejoinState properMilestones := by
  simp
    [SafeRejoinRead,
      ProperRejoinOrder,
      caughtUpRejoinState,
      currentRejoinAuthority,
      currentRejoinToken,
      properMilestones,
      CanServe]

def tokenBeforeCatchupState : RejoinState Bool :=
  { caughtUpRejoinState with installedCacheGeneration := 0 }

theorem current_token_and_membership_before_state_catchup_are_unsafe :
    CanServe
        tokenBeforeCatchupState.fenceAuthority
        tokenBeforeCatchupState.servingToken
      ∧ tokenBeforeCatchupState.membershipPublished = true
      ∧ tokenBeforeCatchupState.readsEnabled = true
      ∧ ¬ SafeRejoinRead tokenBeforeCatchupState properMilestones := by
  simp
    [SafeRejoinRead,
      tokenBeforeCatchupState,
      caughtUpRejoinState,
      currentRejoinAuthority,
      currentRejoinToken,
      properMilestones,
      CanServe]

def tokenIssuedFirstMilestones : RejoinMilestones :=
  { stateInstalledStep := 1
    tokenIssuedStep := 0
    membershipPublishedStep := 2
    readsEnabledStep := 3 }

theorem final_current_state_without_order_receipt_does_not_prove_safe_rejoin :
    caughtUpRejoinState.targetCacheGeneration ≤
        caughtUpRejoinState.installedCacheGeneration
      ∧ CanServe
        caughtUpRejoinState.fenceAuthority
        caughtUpRejoinState.servingToken
      ∧ caughtUpRejoinState.membershipPublished = true
      ∧ caughtUpRejoinState.readsEnabled = true
      ∧ ¬ ProperRejoinOrder tokenIssuedFirstMilestones
      ∧ ¬ SafeRejoinRead
        caughtUpRejoinState
        tokenIssuedFirstMilestones := by
  simp
    [SafeRejoinRead,
      ProperRejoinOrder,
      caughtUpRejoinState,
      currentRejoinAuthority,
      currentRejoinToken,
      tokenIssuedFirstMilestones,
      CanServe]

def forgedRejoinToken : ServingToken Bool :=
  { replica := true
    fenceGeneration := 1 }

def authorityRejectingAllTokens : FenceAuthority Bool :=
  { currentFenceGeneration := fun _replica => 1
    issued := fun _token => False }

def caughtUpWithForgedTokenState : RejoinState Bool :=
  { caughtUpRejoinState with
    fenceAuthority := authorityRejectingAllTokens
    servingToken := forgedRejoinToken }

theorem state_catchup_and_order_do_not_authorize_forged_token :
    caughtUpWithForgedTokenState.targetCacheGeneration ≤
        caughtUpWithForgedTokenState.installedCacheGeneration
      ∧ ProperRejoinOrder properMilestones
      ∧ ¬ CanServe
        caughtUpWithForgedTokenState.fenceAuthority
        caughtUpWithForgedTokenState.servingToken
      ∧ ¬ SafeRejoinRead
        caughtUpWithForgedTokenState
        properMilestones := by
  simp
    [SafeRejoinRead,
      ProperRejoinOrder,
      caughtUpWithForgedTokenState,
      caughtUpRejoinState,
      authorityRejectingAllTokens,
      forgedRejoinToken,
      properMilestones,
      CanServe]

end ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin
