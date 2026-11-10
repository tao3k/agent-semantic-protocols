-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier

namespace ASPProof.SearchRouteAdmissionRetryCacheFencingToken

universe u

structure ServingToken (Replica : Type u) where
  replica : Replica
  fenceGeneration : Nat
deriving DecidableEq, Repr

structure FenceAuthority (Replica : Type u) where
  currentFenceGeneration : Replica → Nat
  issued : ServingToken Replica → Prop

def CanServe
    {Replica : Type u}
    (authority : FenceAuthority Replica)
    (token : ServingToken Replica) : Prop :=
  authority.issued token
    ∧ token.fenceGeneration =
      authority.currentFenceGeneration token.replica

def FenceMonotone
    {Replica : Type u}
    (before after : FenceAuthority Replica) : Prop :=
  ∀ replica,
    before.currentFenceGeneration replica ≤
      after.currentFenceGeneration replica

theorem serving_token_is_authentic_and_generation_current
    {Replica : Type u}
    {authority : FenceAuthority Replica}
    {token : ServingToken Replica}
    (serving : CanServe authority token) :
    authority.issued token
      ∧ token.fenceGeneration =
        authority.currentFenceGeneration token.replica :=
  serving

theorem monotonic_fence_advance_keeps_old_token_stale
    {Replica : Type u}
    {before after : FenceAuthority Replica}
    (monotone : FenceMonotone before after)
    {token : ServingToken Replica}
    (alreadyStale :
      token.fenceGeneration <
        before.currentFenceGeneration token.replica) :
    ¬ CanServe after token := by
  intro serving
  have laterStale :
      token.fenceGeneration <
        after.currentFenceGeneration token.replica :=
    Nat.lt_of_lt_of_le
      alreadyStale
      (monotone token.replica)
  exact (Nat.ne_of_lt laterStale) serving.2

def oldToken : ServingToken Bool :=
  { replica := true
    fenceGeneration := 0 }

def forgedCurrentToken : ServingToken Bool :=
  { replica := true
    fenceGeneration := 1 }

def rejoinFenceAuthority : FenceAuthority Bool :=
  { currentFenceGeneration := fun _replica => 1
    issued := fun token => token = oldToken }

theorem formerly_authentic_old_token_cannot_serve_after_fence_advance :
    rejoinFenceAuthority.issued oldToken
      ∧ oldToken.fenceGeneration <
        rejoinFenceAuthority.currentFenceGeneration oldToken.replica
      ∧ ¬ CanServe rejoinFenceAuthority oldToken := by
  simp [rejoinFenceAuthority, oldToken, CanServe]

theorem forged_current_generation_without_issuance_cannot_serve :
    forgedCurrentToken.fenceGeneration =
        rejoinFenceAuthority.currentFenceGeneration
          forgedCurrentToken.replica
      ∧ ¬ rejoinFenceAuthority.issued forgedCurrentToken
      ∧ ¬ CanServe rejoinFenceAuthority forgedCurrentToken := by
  simp
    [rejoinFenceAuthority,
      forgedCurrentToken,
      oldToken,
      CanServe]

def unsafeBooleanServingFlag (_replica : Bool) : Bool :=
  true

theorem boolean_serving_flag_can_reenable_replica_with_stale_token :
    unsafeBooleanServingFlag oldToken.replica = true
      ∧ ¬ CanServe rejoinFenceAuthority oldToken := by
  simp
    [unsafeBooleanServingFlag,
      rejoinFenceAuthority,
      oldToken,
      CanServe]

end ASPProof.SearchRouteAdmissionRetryCacheFencingToken
